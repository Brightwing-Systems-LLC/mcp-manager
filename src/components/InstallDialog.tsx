import { useState, useEffect } from "react";
import { useStore } from "../store";
import {
  apiGetServer,
  registerProxyServer,
  storeApiKey,
  getApiKey,
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
    showToast,
  } = useStore();

  const [envValues, setEnvValues] = useState<Record<string, string>>({});
  const [deepLinkLoading, setDeepLinkLoading] = useState(false);
  const [deepLinkError, setDeepLinkError] = useState<string | null>(null);
  const [savingKey, setSavingKey] = useState(false);
  const [apiKeySaved, setApiKeySaved] = useState(false);

  // Auto-probe state for HTTP servers
  const [probeResult, setProbeResult] = useState<AuthProbeResult | null>(null);
  const [probing, setProbing] = useState(false);
  const [proxyServer, setProxyServer] = useState<ProxyServer | null>(null);
  const [oauthConnected, setOauthConnected] = useState(false);

  const server = installTarget;
  const deepLink = pendingDeepLink;
  const config = installConfig;

  const isHttpServer = config?.remote_url && config.transport === "http";

  // Determine auth type: for HTTP servers use probe result, for stdio use env schema
  const needsApiKey = isHttpServer
    ? probeResult?.auth_type === "api_key"
    : config ? hasSensitiveEnv(config.env_schema) : false;

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
    if (!config || isHttpServer || stdioProxyRegistered) return;
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
  }, [config, isHttpServer, stdioProxyRegistered, server?.name]);

  // Check if API key already exists in vault on mount
  useEffect(() => {
    if (!config) return;
    const serverId = config.config_key;
    getApiKey(serverId).then((key) => {
      if (key && Object.keys(key.env).length > 0) {
        setApiKeySaved(true);
      }
    }).catch(() => {});
  }, [config?.config_key]);

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

  // Refresh installations on mount
  useEffect(() => {
    refreshInstallations();
  }, [refreshInstallations]);

  // Track which tools already have this server installed
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

  const handleEnvChange = (key: string, value: string) => {
    setEnvValues((prev) => ({ ...prev, [key]: value }));
  };

  /** Save API key to vault */
  const handleSaveApiKey = async () => {
    if (!config) return;

    const serverId = config.config_key;
    const env: Record<string, string> = {};

    for (const [key, schema] of Object.entries(config.env_schema)) {
      const value = envValues[key]?.trim();
      if (value) {
        env[key] = value;
      } else if (schema.default) {
        env[key] = schema.default;
      }
    }

    // Validate that at least one key has a value
    const hasValue = Object.values(env).some((v) => v.length > 0);
    if (!hasValue) {
      showToast("Please enter an API key", "error");
      return;
    }

    setSavingKey(true);
    try {
      await storeApiKey(serverId, env);
      setApiKeySaved(true);
      setEnvValues({});
      showToast("API key saved to secure vault", "success");
      await refreshProxyServers();
    } catch (e) {
      showToast(`Failed to save API key: ${e}`, "error");
    } finally {
      setSavingKey(false);
    }
  };

  const handleBack = () => {
    setInstallTarget(null);
    setPendingDeepLink(null);
    setProbeResult(null);
    setProxyServer(null);
    setOauthConnected(false);
    setStdioProxyRegistered(false);
    setView("dashboard");
  };

  const handleViewDetail = () => {
    if (config) {
      setServerDetailId(normalizeServerName(config.config_key));
      setView("server-detail");
    }
  };

  const handleGoToDashboard = () => {
    setInstallTarget(null);
    setPendingDeepLink(null);
    setView("dashboard");
  };

  // Auth type for status display
  const authType = isHttpServer
    ? probeResult?.auth_type === "oauth" ? "oauth"
      : probeResult?.auth_type === "api_key" ? "api_key"
      : probeResult?.auth_type === "none" ? "none"
      : null
    : needsApiKey ? "api_key" : "none";

  // Status dot color for the identity card
  const dotColor = (() => {
    if (!hasConfig) return "bg-brightwing-gray-500";
    if (authType === "oauth") {
      return oauthConnected ? "bg-green-400" : "bg-purple-400";
    }
    if (authType === "api_key") {
      return apiKeySaved ? "bg-green-400" : "bg-amber-400";
    }
    if (authType === "none") return "bg-green-400";
    return "bg-brightwing-gray-500";
  })();

  return (
    <div>
      <button
        onClick={handleBack}
        className="flex items-center gap-1 text-sm text-brightwing-gray-400 hover:text-brightwing-gray-200 mb-4"
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
        </svg>
        Back to Dashboard
      </button>

      {/* Server identity card — matches ServerDetail style */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-6 mb-6">
        <div className="flex items-start gap-4">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className={`w-2 h-2 rounded-full ${dotColor} shrink-0`} />
              <h1 className="text-xl font-mono font-semibold">{server.name}</h1>
            </div>
            <p className="text-sm text-brightwing-gray-400 mt-1">
              {server.description}
            </p>
            <div className="flex items-center gap-2 mt-2 flex-wrap">
              {authType && (
                <span
                  className={`text-xs px-2 py-0.5 rounded-full ${
                    authType === "oauth"
                      ? "bg-purple-500/20 text-purple-300"
                      : authType === "api_key"
                      ? "bg-amber-500/20 text-amber-300"
                      : "bg-brightwing-gray-700 text-brightwing-gray-400"
                  }`}
                >
                  {authType === "api_key" ? "API Key" : authType === "oauth" ? "OAuth" : "No Auth"}
                </span>
              )}
              {isHttpServer && config?.remote_url && (
                <span className="text-xs font-mono text-brightwing-gray-500 truncate">
                  {config.remote_url}
                </span>
              )}
              {server.current_grade && (
                <span className="text-xs font-semibold text-green-400">
                  {server.current_grade}
                  {server.current_score != null && ` (${server.current_score})`}
                </span>
              )}
              {server.language && <span className="text-xs text-brightwing-gray-500">{server.language}</span>}
              {server.stars_count > 0 && <span className="text-xs text-brightwing-gray-500">{server.stars_count} stars</span>}
            </div>
          </div>
          {config?.verified && (
            <span className="px-2 py-1 text-xs bg-green-500/10 text-green-400 rounded-md shrink-0">
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
          {/* Authentication section — matches ServerDetail pattern */}

          {/* HTTP server auth probe loading */}
          {isHttpServer && probing && (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
              <div className="flex items-center gap-2">
                <svg className="w-4 h-4 animate-spin text-brightwing-blue" viewBox="0 0 24 24" fill="none">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
                <span className="text-xs text-brightwing-gray-500">Checking server auth requirements...</span>
              </div>
            </div>
          )}

          {/* OAuth auth card */}
          {authType === "oauth" && proxyServer && (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
              <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
                Authentication
              </h2>
              <OAuthConnect
                server={proxyServer}
                onStatusChange={(status) => {
                  setOauthConnected(status === "connected");
                }}
              />
            </div>
          )}

          {/* API Key auth card — shows inline form OR configured status */}
          {authType === "api_key" && (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
              <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
                Authentication
              </h2>

              {apiKeySaved ? (
                /* Key is saved — show green status matching ServerDetail */
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-full bg-green-400" />
                  <span className="text-xs text-brightwing-gray-300">API Key configured</span>
                  <button
                    onClick={() => setView("api-keys")}
                    className="ml-auto text-xs text-brightwing-blue hover:text-brightwing-blue/80"
                  >
                    Manage Keys
                  </button>
                </div>
              ) : (
                /* Key not saved — show inline entry form */
                <div className="space-y-3">
                  {config && Object.entries(config.env_schema).map(([key, schema]) => (
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

                  {/* Save button appears when any field has a value */}
                  {Object.values(envValues).some((v) => v.trim().length > 0) && (
                    <button
                      onClick={handleSaveApiKey}
                      disabled={savingKey}
                      className="w-full py-2 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50"
                    >
                      {savingKey ? "Saving to vault..." : "Save API Key"}
                    </button>
                  )}

                  <p className="text-xs text-brightwing-gray-500">
                    Credentials are stored in Brightwing's encrypted vault.
                  </p>
                </div>
              )}
            </div>
          )}

          {/* No auth needed card */}
          {authType === "none" && !probing && (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
              <div className="flex items-center gap-2">
                <span className="w-2 h-2 rounded-full bg-green-400" />
                <span className="text-xs text-brightwing-gray-400">No auth required</span>
              </div>
            </div>
          )}

          {/* Unreachable warning */}
          {isHttpServer && probeResult && !probeResult.server_reachable && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4">
              <p className="text-sm text-red-400 font-medium mb-1">Server Unreachable</p>
              <p className="text-xs text-brightwing-gray-400">
                Could not connect to the server. You can still configure auth later from the server detail page.
              </p>
            </div>
          )}

          {/* Installed In — read-only display (matches ServerDetail) */}
          <div>
            <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
              Installed In
            </h2>
            <div className="flex flex-wrap gap-2">
              {detectedTools.map((tool) => {
                const isInstalled = installedToolIds.has(tool.id);
                if (!isInstalled) return (
                  <span
                    key={tool.id}
                    className="px-3 py-1.5 text-xs rounded-md bg-brightwing-gray-700/50 border border-brightwing-gray-600 text-brightwing-gray-500"
                  >
                    {tool.short_name}
                  </span>
                );
                return (
                  <span
                    key={tool.id}
                    className="px-3 py-1.5 text-xs rounded-md bg-green-500/10 border border-green-500/30 text-green-300"
                  >
                    {tool.short_name} {"\u2713"}
                  </span>
                );
              })}
            </div>
            <p className="text-xs text-brightwing-gray-500 mt-2">
              Use the Dashboard to add or remove this server from tools.
            </p>
          </div>

          {/* Server Info */}
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
              Server Info
            </h2>
            <div className="space-y-2 text-xs">
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Config Key</span>
                <span className="font-mono text-brightwing-gray-300">
                  {config.config_key}
                </span>
              </div>
              {config.remote_url && (
                <div className="flex items-center justify-between">
                  <span className="text-brightwing-gray-500">Upstream URL</span>
                  <span className="font-mono text-brightwing-gray-300 truncate ml-4">{config.remote_url}</span>
                </div>
              )}
              {config.transport === "stdio" && (
                <div className="flex items-center justify-between">
                  <span className="text-brightwing-gray-500">Upstream Command</span>
                  <span className="font-mono text-brightwing-gray-300 truncate ml-4">
                    {config.command} {config.args.join(" ")}
                  </span>
                </div>
              )}
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Transport</span>
                <span className="font-mono text-brightwing-gray-300">proxy (stdio)</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Proxy Command</span>
                <span className="font-mono text-brightwing-gray-300">
                  brightwing-proxy --server {config.config_key}
                </span>
              </div>
            </div>
          </div>

          {/* Action buttons */}
          <div className="flex gap-3">
            <button
              onClick={handleGoToDashboard}
              className="flex-1 py-2.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors"
            >
              Go to Dashboard to Install
            </button>
            {(stdioProxyRegistered || proxyServer) && (
              <button
                onClick={handleViewDetail}
                className="px-4 py-2.5 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 text-white rounded-md transition-colors"
              >
                View Details
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
