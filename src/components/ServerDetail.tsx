import { useEffect, useState } from "react";
import { useStore } from "../store";
import type { ProxyServer, OAuthStatus } from "../lib/types";
import * as tauri from "../lib/tauri";
import OAuthConnect from "./OAuthConnect";
import ToolFilterPanel from "./ToolFilterPanel";
import ProxyLogViewer from "./ProxyLogViewer";

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

type SubView = "detail" | "filter" | "logs";

export default function ServerDetail() {
  const {
    serverDetailId,
    setView,
    setServerDetailId,
    tools,
    configuredServers,
    proxyServers,
    showToast,
    refreshConfiguredServers,
    refreshProxyServers,
    refreshFavorites,
    refreshDisabledServers,
    installTarget,
    installConfig,
  } = useStore();

  const [proxyInstalls, setProxyInstalls] = useState<string[]>([]);
  const [deleting, setDeleting] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [oauthStatus, setOauthStatus] = useState<OAuthStatus | null>(null);
  const [subView, setSubView] = useState<SubView>("detail");

  // Tool filter summary for the title card
  const [toolTokens, setToolTokens] = useState<{ total: number; enabled: number; toolCount: number } | null>(null);
  const [filteringApps, setFilteringApps] = useState<{ filtered: number; total: number }>({ filtered: 0, total: 0 });

  const normalizedName = serverDetailId || "";

  // Find matching proxy server
  const proxyServer: ProxyServer | undefined = proxyServers.find(
    (ps) =>
      normalizeServerName(ps.server_id) === normalizedName ||
      normalizeServerName(ps.display_name) === normalizedName
  );

  // Find display name — prefer proxy display name, then install target, then config
  const displayName = (() => {
    if (proxyServer) return proxyServer.display_name;
    if (installTarget && normalizeServerName(installTarget.name) === normalizedName) return installTarget.name;
    for (const cs of configuredServers) {
      if (normalizeServerName(cs.server_name) === normalizedName) return cs.server_name;
    }
    return normalizedName;
  })();

  // Get scoreboard info if available (from search/deep-link)
  const scoreboardServer = installTarget && normalizeServerName(installTarget.name) === normalizedName
    ? installTarget
    : null;

  // Get install config if available
  const config = installConfig;

  // Find which tools this server is installed in (from config files)
  const installedToolIds = new Set(
    configuredServers
      .filter((cs) => normalizeServerName(cs.server_name) === normalizedName)
      .map((cs) => cs.tool_id)
  );

  // Merge with proxy installs
  for (const toolId of proxyInstalls) {
    installedToolIds.add(toolId);
  }

  const detectedTools = tools.filter((t) => t.detected);

  // Load proxy installs
  useEffect(() => {
    if (proxyServer) {
      tauri.getProxyInstalls(proxyServer.server_id).then(setProxyInstalls);
    }
  }, [proxyServer?.server_id]);

  // Load OAuth status
  useEffect(() => {
    if (proxyServer?.auth_type === "oauth") {
      tauri.getOAuthStatus(proxyServer.server_id).then(setOauthStatus);
    }
  }, [proxyServer?.server_id, proxyServer?.auth_type]);

  // Load tool filter summary (total tokens + per-app filtering status)
  useEffect(() => {
    if (!proxyServer) return;
    const sid = proxyServer.server_id;

    // Load the global (_all) filter for total token count
    tauri.getToolFilter(sid).then((entries) => {
      if (entries.length > 0) {
        const total = entries.reduce((s, e) => s + e.token_estimate, 0);
        const enabled = entries.filter((e) => e.enabled).reduce((s, e) => s + e.token_estimate, 0);
        setToolTokens({ total, enabled, toolCount: entries.length });
      }
    }).catch(() => {});

    // Check per-app filtering: for each detected+installed tool, check if any tools are disabled
    const installed = detectedTools.filter((t) => installedToolIds.has(t.id));
    if (installed.length === 0) return;
    let filteredCount = 0;
    let completed = 0;
    for (const tool of installed) {
      tauri.getToolFilter(sid, tool.id).then((entries) => {
        if (entries.some((e) => !e.enabled)) filteredCount++;
        completed++;
        if (completed === installed.length) {
          setFilteringApps({ filtered: filteredCount, total: installed.length });
        }
      }).catch(() => {
        completed++;
        if (completed === installed.length) {
          setFilteringApps({ filtered: filteredCount, total: installed.length });
        }
      });
    }
  }, [proxyServer?.server_id, subView]); // re-check when returning from filter sub-view

  // Copy to clipboard helper
  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    showToast("Copied to clipboard", "success");
  };

  const handleDelete = async () => {
    setDeleting(true);
    try {
      const variants = new Set<string>();
      variants.add(normalizedName);
      if (proxyServer) {
        variants.add(proxyServer.server_id);
        variants.add(proxyServer.display_name);
      }
      for (const cs of configuredServers) {
        if (normalizeServerName(cs.server_name) === normalizedName) {
          variants.add(cs.server_name);
        }
      }

      const removedFrom = await tauri.deleteServer(Array.from(variants));

      // Also delete any API keys associated with this server
      if (proxyServer) {
        await tauri.deleteApiKey(proxyServer.server_id).catch(() => {});
      }

      await Promise.all([
        refreshConfiguredServers(),
        refreshProxyServers(),
        refreshFavorites(),
        refreshDisabledServers(),
      ]);
      showToast(
        removedFrom.length > 0
          ? `Deleted "${displayName}" from ${removedFrom.length} app${removedFrom.length > 1 ? "s" : ""}`
          : `Deleted "${displayName}"`,
        "success"
      );
      setServerDetailId(null);
      setView("dashboard");
    } catch (e) {
      showToast(`Failed to delete: ${e}`, "error");
    }
    setDeleting(false);
    setDeleteConfirm(false);
  };

  const handleBack = () => {
    setServerDetailId(null);
    setView("dashboard");
  };

  if (!serverDetailId) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <p className="text-brightwing-gray-400">No server selected.</p>
        <button
          onClick={() => setView("dashboard")}
          className="mt-3 px-4 py-2 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md"
        >
          Back to Dashboard
        </button>
      </div>
    );
  }

  // Sub-views
  if (subView === "filter" && proxyServer) {
    return (
      <ToolFilterPanel
        server={proxyServer}
        onBack={() => setSubView("detail")}
      />
    );
  }

  if (subView === "logs" && proxyServer) {
    return (
      <ProxyLogViewer
        server={proxyServer}
        onBack={() => setSubView("detail")}
      />
    );
  }

  // Auth type display
  const authType = proxyServer?.auth_type || "none";

  // Status dot color
  const dotColor = proxyServer
    ? authType === "oauth"
      ? oauthStatus?.status === "connected"
        ? "bg-green-400"
        : oauthStatus?.status === "expired"
        ? "bg-amber-400"
        : "bg-red-400"
      : authType === "api_key"
      ? "bg-green-400"
      : "bg-green-400"
    : "bg-red-400";

  return (
    <div>
      {/* Header */}
      <button
        onClick={handleBack}
        className="flex items-center gap-1 text-sm text-brightwing-gray-400 hover:text-brightwing-gray-200 mb-4"
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
        </svg>
        Back to Dashboard
      </button>

      {/* Server identity card */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-6 mb-6">
        <div className="flex items-start gap-4">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className={`w-2 h-2 rounded-full ${dotColor} shrink-0`} />
              <h1 className="text-xl font-mono font-semibold">{displayName}</h1>
            </div>
            {scoreboardServer?.description && (
              <p className="text-sm text-brightwing-gray-400 mt-1">
                {scoreboardServer.description}
              </p>
            )}
            <div className="flex items-center gap-2 mt-2 flex-wrap">
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
              {proxyServer?.upstream_url && (
                <span className="text-xs font-mono text-brightwing-gray-500 truncate">
                  {proxyServer.upstream_url}
                </span>
              )}
              {scoreboardServer?.current_grade && (
                <span className="text-xs font-semibold text-green-400">
                  {scoreboardServer.current_grade}
                  {scoreboardServer.current_score != null && ` (${scoreboardServer.current_score})`}
                </span>
              )}
              {scoreboardServer?.language && (
                <span className="text-xs text-brightwing-gray-500">{scoreboardServer.language}</span>
              )}
              {scoreboardServer?.stars_count != null && scoreboardServer.stars_count > 0 && (
                <span className="text-xs text-brightwing-gray-500">{scoreboardServer.stars_count} stars</span>
              )}
            </div>
          </div>
          <div className="flex flex-col items-end gap-2 shrink-0">
            {config?.verified && (
              <span className="px-2 py-1 text-xs bg-green-500/10 text-green-400 rounded-md">
                Verified
              </span>
            )}
            {proxyServer && toolTokens && (
              <div className="bg-brightwing-gray-900 border border-brightwing-gray-600 rounded-lg px-4 py-3 text-right min-w-[160px]">
                <div className="text-lg font-mono font-semibold text-white">
                  ~{toolTokens.enabled.toLocaleString()}
                  <span className="text-xs font-normal text-brightwing-gray-500 ml-1">tok</span>
                </div>
                {toolTokens.enabled < toolTokens.total && (
                  <div className="text-[10px] text-brightwing-gray-500 font-mono">
                    of {toolTokens.total.toLocaleString()} total
                  </div>
                )}
                <div className="text-[11px] text-brightwing-gray-400 mt-1">
                  {filteringApps.filtered > 0 ? (
                    <span className="text-brightwing-blue">
                      Filtering: {filteringApps.filtered}/{filteringApps.total} apps
                    </span>
                  ) : (
                    <span className="text-brightwing-gray-500">
                      Filtering: 0/{filteringApps.total} apps
                    </span>
                  )}
                </div>
                <button
                  onClick={() => setSubView("filter")}
                  className="mt-2 w-full px-3 py-1.5 text-xs font-medium bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors"
                >
                  Filter Tools
                </button>
              </div>
            )}
          </div>
        </div>
        {config?.install_notes && (
          <p className="text-xs text-brightwing-gray-500 mt-3 border-t border-brightwing-gray-700 pt-3">
            {config.install_notes}
          </p>
        )}
      </div>

      <div className="space-y-6 max-w-xl">
        {/* Authentication section */}
        {proxyServer && authType === "oauth" && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
              Authentication
            </h2>
            <OAuthConnect
              server={proxyServer}
              onStatusChange={() => {
                tauri.getOAuthStatus(proxyServer.server_id).then(setOauthStatus);
              }}
            />
          </div>
        )}

        {proxyServer && authType === "api_key" && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
              Authentication
            </h2>
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
          </div>
        )}

        {proxyServer && authType === "none" && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-green-400" />
              <span className="text-xs text-brightwing-gray-400">No auth required</span>
            </div>
          </div>
        )}

        {!proxyServer && (
          <div className="bg-brightwing-gray-800 border border-red-500/30 rounded-lg p-4">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-red-400" />
              <span className="text-xs text-red-300">No proxy registered</span>
            </div>
            <p className="text-xs text-brightwing-gray-500 mt-1">
              This server was installed directly. Re-install from Search to set up proxy authentication.
            </p>
          </div>
        )}

        {/* Installed In — read-only display */}
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
                {proxyServer?.server_id || normalizedName}
              </span>
            </div>
            {proxyServer?.upstream_url && (
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Upstream URL</span>
                <span className="font-mono text-brightwing-gray-300 truncate ml-4">{proxyServer.upstream_url}</span>
              </div>
            )}
            {proxyServer?.upstream_command && (
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Upstream Command</span>
                <span className="font-mono text-brightwing-gray-300 truncate ml-4">
                  {proxyServer.upstream_command} {proxyServer.upstream_args || ""}
                </span>
              </div>
            )}
            <div className="flex items-center justify-between">
              <span className="text-brightwing-gray-500">Transport</span>
              <span className="font-mono text-brightwing-gray-300">
                {proxyServer ? "proxy (stdio)" : "direct"}
              </span>
            </div>
            {proxyServer && (
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Proxy Command</span>
                <span className="font-mono text-brightwing-gray-300">
                  brightwing-proxy --server {proxyServer.server_id}
                </span>
              </div>
            )}
          </div>
        </div>

        {/* Actions — Logs */}
        {proxyServer && (
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => setSubView("logs")}
              className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors inline-flex items-center gap-1.5"
            >
              <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M3.75 12h16.5m-16.5 3.75h16.5M3.75 19.5h16.5M5.625 4.5h12.75a1.875 1.875 0 0 1 0 3.75H5.625a1.875 1.875 0 0 1 0-3.75Z" />
              </svg>
              Logs
            </button>
          </div>
        )}

        {/* CLI Usage */}
        {proxyServer && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
              CLI Usage
            </h2>
            <div className="space-y-2">
              {(() => {
                const qId = proxyServer.server_id.includes(' ') ? `"${proxyServer.server_id}"` : proxyServer.server_id;
                return [
                  { label: "List tools", cmd: `bw ${qId}` },
                  { label: "Tool help", cmd: `bw ${qId} <tool> --help` },
                  { label: "Call a tool", cmd: `bw ${qId} <tool> --key value` },
                  { label: "Raw JSON output", cmd: `bw ${qId} <tool> --key value --json` },
                ];
              })().map(({ label, cmd }) => (
                <div key={label}>
                  <div className="text-xs text-brightwing-gray-500 mb-0.5">{label}</div>
                  <div className="flex items-center gap-2">
                    <code className="flex-1 text-xs font-mono text-brightwing-gray-200 bg-brightwing-gray-900 px-2 py-1 rounded block">
                      {cmd}
                    </code>
                    <button
                      onClick={() => copyToClipboard(cmd)}
                      className="p-1 text-brightwing-gray-500 hover:text-brightwing-gray-300 transition-colors shrink-0"
                      title="Copy to clipboard"
                    >
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M15.666 3.888A2.25 2.25 0 0 0 13.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 0 1-.75.75H9.75a.75.75 0 0 1-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 0 1-2.25 2.25H6.75A2.25 2.25 0 0 1 4.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 0 1 1.927-.184" />
                      </svg>
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Delete */}
        <div className="border-t border-brightwing-gray-700 pt-6">
          {!deleteConfirm ? (
            <button
              onClick={() => setDeleteConfirm(true)}
              className="px-4 py-2 text-sm text-red-400 hover:bg-red-500/10 border border-red-500/30 rounded-md transition-colors"
            >
              Delete Server
            </button>
          ) : (
            <div className="bg-red-500/5 border border-red-500/20 rounded-lg p-4">
              <p className="text-sm text-red-300 font-medium mb-2">
                Delete "{displayName}"?
              </p>
              <p className="text-xs text-brightwing-gray-400 mb-3">
                This will remove the server from all tools, delete proxy registration, credentials, and all associated data. This cannot be undone.
              </p>
              <div className="flex gap-2">
                <button
                  onClick={handleDelete}
                  disabled={deleting}
                  className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 text-white rounded-md transition-colors disabled:opacity-50"
                >
                  {deleting ? "Deleting..." : "Yes, Delete"}
                </button>
                <button
                  onClick={() => setDeleteConfirm(false)}
                  disabled={deleting}
                  className="px-4 py-2 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
          <p className="text-xs text-brightwing-gray-500 mt-2">
            Removes from all tools and unregisters the proxy server.
          </p>
        </div>
      </div>
    </div>
  );
}
