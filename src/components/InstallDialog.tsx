import { useState, useEffect } from "react";
import { useStore } from "../store";
import {
  apiGetServer,
  registerProxyServer,
  storeApiKey,
  installProxyToTool,
  uninstallProxyFromTool,
  daemonStatus,
  startDaemon,
  getProxyServer,
  probeServerAuth,
} from "../lib/tauri";
import type { ScoreboardServer, AuthProbeResult, ProxyServer } from "../lib/types";
import OAuthConnect from "./OAuthConnect";

// Mirror Rust sanitize_server_name
function normalizeServerName(name: string): string {
  let sanitized = name
    .split("")
    .map((c) => (/[a-zA-Z0-9_-]/.test(c) ? c : "_"))
    .join("");
  sanitized = sanitized.replace(/_+/g, "_");
  sanitized = sanitized.replace(/^_+|_+$/g, "");
  return sanitized.toLowerCase();
}

// Tools with known MCP bugs — config is written correctly but the app may not use it
const TOOL_WARNINGS: Record<string, { short: string; detail: string; link: string }> = {
  codex: {
    short: "Desktop app MCP tools may not work",
    detail:
      "Codex Desktop has a known bug where MCP tools are configured but not surfaced to the model. The CLI works correctly. Both share the same config file.",
    link: "https://github.com/openai/codex/issues/11264",
  },
};

/** Returns true if the install config has any sensitive env vars (API keys, tokens, etc.) */
function hasSensitiveEnv(
  envSchema: Record<string, { sensitive: boolean }>
): boolean {
  return Object.values(envSchema).some((s) => s.sensitive);
}

export default function InstallDialog() {
  const {
    installTarget,
    installConfig,
    installConfigLoading,
    pendingDeepLink,
    tools,
    installations,
    setView,
    setInstallTarget,
    setPendingDeepLink,
    setServerDetailId,
    refreshInstallations,
    refreshProxyServers,
    refreshConfiguredServers,
    showToast,
    addPendingRestart,
  } = useStore();

  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [saving, setSaving] = useState(false);
  const [envValues, setEnvValues] = useState<Record<string, string>>({});
  const [deepLinkLoading, setDeepLinkLoading] = useState(false);
  const [deepLinkError, setDeepLinkError] = useState<string | null>(null);
  // Proxy mode is always on — all servers go through the proxy
  const proxyMode = true;

  // Auto-probe state for HTTP servers
  const [probeResult, setProbeResult] = useState<AuthProbeResult | null>(null);
  const [probing, setProbing] = useState(false);
  const [proxyServer, setProxyServer] = useState<ProxyServer | null>(null);
  const [oauthConnected, setOauthConnected] = useState(false);

  const server = installTarget;
  const deepLink = pendingDeepLink;
  const config = installConfig;

  const isHttpServer = config?.remote_url && config.transport === "http";

  // Auto-probe HTTP servers when config loads
  useEffect(() => {
    if (!config?.remote_url || config.transport !== "http") return;
    if (probeResult || probing) return;

    let cancelled = false;
    setProbing(true);

    (async () => {
      try {
        const result = await probeServerAuth(config.remote_url!);
        if (cancelled) return;
        setProbeResult(result);

        // Auto-register proxy server invisibly
        const daemonOk = await ensureDaemon();
        if (!daemonOk || cancelled) return;

        const serverId = config.config_key;
        const authType = result.auth_type === "oauth" ? "oauth"
          : result.auth_type === "api_key" ? "api_key"
          : "none";

        await registerProxyServer({
          serverId,
          displayName: server?.name || serverId,
          authType,
          upstreamUrl: config.remote_url || undefined,
        });

        const ps = await getProxyServer(serverId);
        if (!cancelled && ps) {
          setProxyServer(ps);
        }
      } catch (e) {
        if (!cancelled) {
          setProbeResult({
            auth_type: "unknown",
            server_reachable: false,
            error_message: String(e),
            has_oauth_metadata: false,
          });
        }
      } finally {
        if (!cancelled) setProbing(false);
      }
    })();

    return () => { cancelled = true; };
  }, [config?.remote_url, config?.transport, config?.config_key, server?.name]);

  // Auto-register proxy server for stdio servers when config loads
  const [stdioProxyRegistered, setStdioProxyRegistered] = useState(false);
  useEffect(() => {
    if (!config || isHttpServer || stdioProxyRegistered || !proxyMode) return;
    if (!server) return;

    let cancelled = false;
    (async () => {
      try {
        const daemonOk = await ensureDaemon();
        if (!daemonOk || cancelled) return;

        const serverId = config.config_key;
        const authType = hasSensitiveEnv(config.env_schema) ? "api_key" : "none";
        await registerProxyServer({
          serverId,
          displayName: server.name,
          authType,
          upstreamCommand: config.command,
          upstreamArgs: config.args.join(" "),
        });
        await refreshProxyServers();
        if (!cancelled) setStdioProxyRegistered(true);
      } catch {
        // Non-fatal — proxy will be registered on save
      }
    })();

    return () => { cancelled = true; };
  }, [config, isHttpServer, stdioProxyRegistered, proxyMode, server?.name]);

  // When we have a deep link but no server, fetch the server details
  useEffect(() => {
    if (deepLink && !server && !deepLinkLoading) {
      setDeepLinkLoading(true);
      setDeepLinkError(null);
      apiGetServer(deepLink.server_uuid)
        .then((serverData: ScoreboardServer) => {
          setInstallTarget(serverData);
          setPendingDeepLink(null);
        })
        .catch((e) => {
          setDeepLinkError(`Failed to fetch server: ${e}`);
        })
        .finally(() => {
          setDeepLinkLoading(false);
        });
    }
  }, [deepLink, server, deepLinkLoading, setInstallTarget, setPendingDeepLink]);

  // Refresh installations on mount to avoid stale data (e.g. deep link while app is running)
  useEffect(() => {
    refreshInstallations();
  }, [refreshInstallations]);

  // Initialize checkbox state from current installations when server loads
  const detectedTools = tools.filter((t) => t.detected);
  const installedToolIds = server
    ? new Set(
        installations
          .filter(
            (i) =>
              i.server_name === server.name ||
              i.server_uuid === String(server.id)
          )
          .map((i) => i.tool_id)
      )
    : new Set<string>();

  useEffect(() => {
    if (server) {
      setSelectedTools(new Set(installedToolIds));
    }
  }, [server?.id, installations]);

  if (!server && !deepLink) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <p className="text-brightwing-gray-400">
          No server selected for installation.
        </p>
        <button
          onClick={() => setView("search")}
          className="mt-3 px-4 py-2 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md"
        >
          Search Servers
        </button>
      </div>
    );
  }

  if (deepLink && !server) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center max-w-md">
          {deepLinkError ? (
            <>
              <h2 className="text-lg font-semibold mb-2 text-red-400">Error</h2>
              <p className="text-brightwing-gray-400 text-sm">{deepLinkError}</p>
            </>
          ) : (
            <>
              <h2 className="text-lg font-semibold mb-2">Loading Server</h2>
              <p className="text-brightwing-gray-500 text-sm">
                Fetching server details from MCP Scoreboard...
              </p>
            </>
          )}
          <button
            onClick={() => {
              setPendingDeepLink(null);
              setDeepLinkError(null);
              setView("dashboard");
            }}
            className="mt-4 px-4 py-2 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md"
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }

  if (!server) return null;

  const hasConfig = config !== null;

  const toggleTool = (toolId: string) => {
    const next = new Set(selectedTools);
    if (next.has(toolId)) {
      next.delete(toolId);
    } else {
      next.add(toolId);
    }
    setSelectedTools(next);
  };

  const handleEnvChange = (key: string, value: string) => {
    setEnvValues((prev) => ({ ...prev, [key]: value }));
  };

  // Compute what changed vs. current installations
  const toInstall = [...selectedTools].filter((id) => !installedToolIds.has(id));
  const toRemove = [...installedToolIds].filter((id) => !selectedTools.has(id));
  const hasChanges = toInstall.length > 0 || toRemove.length > 0;

  // For HTTP+OAuth servers, block install until OAuth is connected
  const needsOauthFirst = !!(isHttpServer && probeResult?.auth_type === "oauth" && !oauthConnected);

  /** Ensure the daemon is running, starting it if needed. */
  const ensureDaemon = async (): Promise<boolean> => {
    try {
      const status = await daemonStatus();
      if (status.running) return true;
      const started = await startDaemon();
      return started.running;
    } catch {
      showToast("Failed to start auth daemon", "error");
      return false;
    }
  };

  /** Proxy install: register server with daemon, store credentials, install proxy configs. */
  const handleProxySave = async () => {
    if (!config) return;

    // For HTTP servers with auto-probe, proxy registration already happened
    const serverId = config.config_key;

    // Validate required env vars (for non-OAuth HTTP servers or stdio proxy)
    if (toInstall.length > 0 && !isHttpServer) {
      const missingRequired = Object.entries(config.env_schema)
        .filter(([, schema]) => schema.required)
        .filter(([key]) => !envValues[key]?.trim());

      if (missingRequired.length > 0) {
        showToast(
          `Missing required: ${missingRequired.map(([k]) => k).join(", ")}`,
          "error"
        );
        return;
      }
    }

    // For API key HTTP servers, validate env vars
    if (isHttpServer && probeResult?.auth_type === "api_key" && toInstall.length > 0) {
      const missingRequired = Object.entries(config.env_schema)
        .filter(([, schema]) => schema.required)
        .filter(([key]) => !envValues[key]?.trim());

      if (missingRequired.length > 0) {
        showToast(
          `Missing required: ${missingRequired.map(([k]) => k).join(", ")}`,
          "error"
        );
        return;
      }
    }

    setSaving(true);

    try {
      // Ensure daemon is running
      const daemonOk = await ensureDaemon();
      if (!daemonOk) {
        setSaving(false);
        return;
      }

      // If not an auto-probed HTTP server, register proxy now
      if (!isHttpServer) {
        const authType = hasSensitiveEnv(config.env_schema) ? "api_key" : "none";
        await registerProxyServer({
          serverId,
          displayName: server.name,
          authType,
          upstreamUrl: config.remote_url || undefined,
          upstreamCommand: config.transport === "stdio" ? config.command : undefined,
          upstreamArgs:
            config.transport === "stdio" ? config.args.join(" ") : undefined,
        });
      }

      // Store credentials if there are sensitive env vars (not OAuth)
      if (probeResult?.auth_type !== "oauth" && hasSensitiveEnv(config.env_schema)) {
        const env: Record<string, string> = {};
        for (const [key, schema] of Object.entries(config.env_schema)) {
          const value = envValues[key]?.trim();
          if (value) {
            env[key] = value;
          } else if (schema.default) {
            env[key] = schema.default;
          }
        }
        await storeApiKey(serverId, env);
      }

      // Install/uninstall proxy configs
      let successCount = 0;
      let failCount = 0;

      for (const toolId of toInstall) {
        try {
          const result = await installProxyToTool(toolId, serverId, config.config_key);
          if (result.success) {
            successCount++;
            if (result.needs_restart) addPendingRestart(toolId);
          } else {
            failCount++;
            showToast(result.message, "error");
          }
        } catch (e) {
          failCount++;
          showToast(`Proxy install failed: ${e}`, "error");
        }
      }

      for (const toolId of toRemove) {
        try {
          const result = await uninstallProxyFromTool(toolId, serverId, config.config_key);
          if (result.success) {
            successCount++;
            if (result.needs_restart) addPendingRestart(toolId);
          } else {
            failCount++;
            showToast(result.message, "error");
          }
        } catch (e) {
          failCount++;
          showToast(`Proxy uninstall failed: ${e}`, "error");
        }
      }

      await Promise.all([refreshInstallations(), refreshProxyServers(), refreshConfiguredServers()]);

      if (failCount === 0) {
        const parts = [];
        if (toInstall.length > 0)
          parts.push(`installed into ${toInstall.length}`);
        if (toRemove.length > 0)
          parts.push(`removed from ${toRemove.length}`);
        showToast(
          `${server.name}: ${parts.join(", ")} tool${successCount !== 1 ? "s" : ""} (via proxy)`,
          "success"
        );
        // Navigate to unified server detail page
        setServerDetailId(normalizeServerName(config.config_key));
        setView("server-detail");
      }
    } catch (e) {
      showToast(`Proxy setup failed: ${e}`, "error");
    } finally {
      setSaving(false);
    }
  };

  const handleSave = handleProxySave;

  const handleBack = () => {
    setInstallTarget(null);
    setPendingDeepLink(null);
    setProbeResult(null);
    setProxyServer(null);
    setOauthConnected(false);
    setStdioProxyRegistered(false);
    setView("dashboard");
  };


  return (
    <div>
      <button
        onClick={handleBack}
        className="flex items-center gap-1 text-sm text-brightwing-gray-400 hover:text-brightwing-gray-200 mb-4"
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
        </svg>
        Back
      </button>

      {/* Server info */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-6 mb-6">
        <div className="flex items-start gap-4">
          <div className="flex-1">
            <h1 className="text-xl font-mono font-semibold">{server.name}</h1>
            <p className="text-sm text-brightwing-gray-400 mt-1">
              {server.description}
            </p>
            <div className="flex gap-3 mt-2 text-xs text-brightwing-gray-500">
              {server.current_grade && (
                <span className="font-semibold text-green-400">
                  {server.current_grade}
                  {server.current_score != null && ` (${server.current_score})`}
                </span>
              )}
              {server.language && <span>{server.language}</span>}
              {server.stars_count > 0 && <span>{server.stars_count} stars</span>}
            </div>
          </div>
          {config?.verified && (
            <span className="px-2 py-1 text-xs bg-green-500/10 text-green-400 rounded-md">
              Verified
            </span>
          )}
        </div>
        {config?.install_notes && (
          <p className="text-xs text-brightwing-gray-500 mt-3 border-t border-brightwing-gray-700 pt-3">
            {config.install_notes}
          </p>
        )}
      </div>

      {/* Install config status */}
      {installConfigLoading ? (
        <p className="text-brightwing-gray-500 text-sm mb-6">
          Fetching install configuration...
        </p>
      ) : !hasConfig ? (
        <div className="bg-brightwing-gray-800 border border-yellow-500/30 rounded-lg p-5 mb-6">
          <p className="text-yellow-400 text-sm font-medium mb-1">
            No install configuration available
          </p>
          <p className="text-brightwing-gray-500 text-xs">
            This server doesn't have a verified install config on MCP Scoreboard
            yet. You can install it manually by editing your tool's config file
            directly.
          </p>
        </div>
      ) : (
        <div className="space-y-5 max-w-xl">
          {/* Auth probe status for HTTP servers */}
          {isHttpServer && (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
              <div className="flex items-center gap-2 mb-1">
                <p className="text-sm font-medium">Server Authentication</p>
                {probing && (
                  <svg className="w-4 h-4 animate-spin text-brightwing-blue" viewBox="0 0 24 24" fill="none">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                  </svg>
                )}
              </div>
              {probing ? (
                <p className="text-xs text-brightwing-gray-500">Checking server auth requirements...</p>
              ) : probeResult ? (
                <div className="flex items-center gap-2">
                  <span
                    className={`w-2 h-2 rounded-full ${
                      !probeResult.server_reachable
                        ? "bg-red-400"
                        : probeResult.auth_type === "oauth"
                        ? "bg-purple-400"
                        : probeResult.auth_type === "api_key"
                        ? "bg-amber-400"
                        : probeResult.auth_type === "none"
                        ? "bg-green-400"
                        : "bg-brightwing-gray-500"
                    }`}
                  />
                  <span className="text-xs text-brightwing-gray-300">
                    {!probeResult.server_reachable
                      ? "Server unreachable"
                      : probeResult.auth_type === "oauth"
                      ? "OAuth detected"
                      : probeResult.auth_type === "api_key"
                      ? "API key required"
                      : probeResult.auth_type === "none"
                      ? "No auth needed"
                      : "Unknown auth"}
                  </span>
                  {probeResult.error_message && (
                    <span className="text-xs text-brightwing-gray-500">
                      ({probeResult.error_message})
                    </span>
                  )}
                </div>
              ) : null}
            </div>
          )}

          {/* Inline OAuth connect for HTTP+OAuth servers */}
          {isHttpServer && probeResult?.auth_type === "oauth" && proxyServer && (
            <div className="bg-brightwing-gray-800 border border-purple-500/30 rounded-lg p-4">
              <h3 className="text-sm font-medium mb-3">Connect with OAuth</h3>
              <OAuthConnect
                server={proxyServer}
                onStatusChange={(status) => {
                  setOauthConnected(status === "connected");
                }}
              />
            </div>
          )}

          {/* Unreachable fallback warning */}
          {isHttpServer && probeResult && !probeResult.server_reachable && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4">
              <p className="text-sm text-red-400 font-medium mb-1">Server Unreachable</p>
              <p className="text-xs text-brightwing-gray-400">
                Could not connect to the server. You can still install it and configure auth later from the Proxy tab.
              </p>
            </div>
          )}

          {/* Proxy command preview */}
          {(
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
              <p className="text-xs text-brightwing-gray-500 mb-1">
                Proxy Command (written to tool configs)
              </p>
              <code className="text-sm text-brightwing-gray-200 font-mono">
                brightwing-proxy --server {config.config_key}
              </code>
            </div>
          )}

          {/* Env var form — hidden for HTTP+OAuth (OAuth handles auth), shown for API key / none / stdio */}
          {!(isHttpServer && probeResult?.auth_type === "oauth") &&
            Object.keys(config.env_schema).length > 0 && (
            <div>
              <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
                {proxyMode ? "Credentials (stored securely)" : "Configuration"}
              </h2>
              <div className="space-y-3">
                {Object.entries(config.env_schema).map(([key, schema]) => (
                  <div key={key}>
                    <label className="block text-xs text-brightwing-gray-400 mb-1">
                      {key}
                      {schema.required && (
                        <span className="text-red-400 ml-1">*</span>
                      )}
                      {schema.description && (
                        <span className="text-brightwing-gray-600 ml-2">
                          — {schema.description}
                        </span>
                      )}
                    </label>
                    <input
                      type={schema.sensitive ? "password" : "text"}
                      placeholder={schema.default || ""}
                      value={envValues[key] || ""}
                      onChange={(e) => handleEnvChange(key, e.target.value)}
                      className="w-full px-3 py-2 bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
                    />
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Tool selection with checkboxes */}
          <div>
            <label className="block text-xs text-brightwing-gray-400 uppercase tracking-wider mb-2">
              Install Into
            </label>
            <div className="space-y-2">
              {detectedTools.map((tool) => {
                const isSelected = selectedTools.has(tool.id);
                return (
                  <button
                    key={tool.id}
                    onClick={() => toggleTool(tool.id)}
                    disabled={saving}
                    className={`w-full flex items-center gap-3 p-3 rounded-lg border transition-colors text-left ${
                      isSelected
                        ? "bg-brightwing-blue/10 border-brightwing-blue"
                        : "bg-brightwing-gray-800 border-brightwing-gray-700 hover:border-brightwing-gray-600"
                    } disabled:opacity-50`}
                  >
                    <div
                      className={`w-4 h-4 rounded border-2 flex items-center justify-center flex-shrink-0 ${
                        isSelected
                          ? "border-brightwing-blue bg-brightwing-blue"
                          : "border-brightwing-gray-600"
                      }`}
                    >
                      {isSelected && (
                        <svg
                          className="w-3 h-3 text-white"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                          strokeWidth={3}
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="m4.5 12.75 6 6 9-13.5"
                          />
                        </svg>
                      )}
                    </div>
                    <span className="text-sm font-medium">
                      {tool.display_name}
                    </span>
                    <span className="text-xs text-brightwing-gray-500">
                      ({tool.short_name})
                    </span>
                    {TOOL_WARNINGS[tool.id] && (
                      <span
                        className="text-xs text-amber-400 flex items-center gap-1"
                        title={TOOL_WARNINGS[tool.id].detail}
                      >
                        <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                          <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
                        </svg>
                        Known issue
                      </span>
                    )}
                    {installedToolIds.has(tool.id) && (
                      <span className="ml-auto text-xs text-green-400">
                        installed
                      </span>
                    )}
                  </button>
                );
              })}
              {detectedTools.length === 0 && (
                <p className="text-brightwing-gray-500 text-sm">
                  No AI tools detected on your machine.
                </p>
              )}
            </div>
          </div>

          {/* Save button */}
          <button
            onClick={handleSave}
            disabled={saving || !hasChanges || needsOauthFirst}
            className="w-full py-2.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {saving
              ? proxyMode
                ? "Setting up proxy..."
                : "Saving..."
              : needsOauthFirst
                ? "Connect OAuth First"
                : !hasChanges
                  ? "No Changes"
                  : proxyMode
                    ? "Save Changes (via Proxy)"
                    : "Save Changes"}
          </button>

          <p className="text-xs text-brightwing-gray-500">
            {proxyMode
              ? "Credentials are stored in Brightwing's secure vault. Tool configs will reference the local proxy."
              : "Changes will appear in the restart banner above when tools need restarting."}
          </p>
        </div>
      )}
    </div>
  );
}
