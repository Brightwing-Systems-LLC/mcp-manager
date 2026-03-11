import { useEffect, useState, useMemo, useCallback } from "react";
import { useStore } from "../store";
import { fetchCliServerConfig, deleteServer, getOAuthStatus, installProxyToTool, registerProxyServer, daemonStatus, startDaemon, addFavorite, removeFavorite } from "../lib/tauri";
import type { OAuthStatus } from "../lib/types";

type CellInfo = {
  configJson: string | null;
  isCliOnly: boolean;
  actualName: string; // the real name in this tool's config (may differ from normalized row key)
};

// Mirror Rust sanitize_server_name — used to group servers with different names across tools
function normalizeServerName(name: string): string {
  let sanitized = name
    .split("")
    .map((c) => (/[a-zA-Z0-9_-]/.test(c) ? c : "_"))
    .join("");
  sanitized = sanitized.replace(/_+/g, "_");
  sanitized = sanitized.replace(/^_+|_+$/g, "");
  return sanitized.toLowerCase();
}

type PendingChange = {
  serverName: string;
  toolId: string;
  action: "enable" | "disable" | "add" | "remove";
  cellInfo: CellInfo;
};

// Tools where remote connectors are cloud-managed with no local API
const LOCKED_TOOLS = new Set<string>();

// Tools with known MCP bugs — config is written correctly but the app may not use it
const TOOL_WARNINGS: Record<string, { short: string; detail: string; link: string }> = {
  codex: {
    short: "Desktop app MCP tools may not work",
    detail:
      "Codex Desktop has a known bug where MCP tools are configured but not surfaced to the model. The CLI works correctly. Both share the same config file.",
    link: "https://github.com/openai/codex/issues/11264",
  },
};

export default function Dashboard() {
  const {
    tools,
    toolsLoading,
    refreshTools,
    configuredServers,
    configuredServersLoading,
    refreshConfiguredServers,
    disabledServers,
    refreshDisabledServers,
    disableServer,
    enableServer,
    addPendingRestart,
    showToast,
    proxyServers,
    refreshProxyServers,
    favorites,
    refreshFavorites,
    setView,
    setServerDetailId,
  } = useStore();

  const [showFavoritesOnly, setShowFavoritesOnly] = useState(false);

  const [pendingChanges, setPendingChanges] = useState<Map<string, PendingChange>>(new Map());
  const [saving, setSaving] = useState(false);
  const [saveProgress, setSaveProgress] = useState({ current: 0, total: 0, currentName: "" });
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [oauthStatuses, setOauthStatuses] = useState<Record<string, OAuthStatus>>({});

  useEffect(() => {
    refreshConfiguredServers();
    refreshDisabledServers();
    refreshProxyServers();
    refreshFavorites();
  }, [refreshConfiguredServers, refreshDisabledServers, refreshProxyServers, refreshFavorites]);

  // Build a set of favorited normalized names for quick lookup
  const favoriteNames = useMemo(() => {
    const set = new Set<string>();
    for (const f of favorites) {
      set.add(normalizeServerName(f.server_name));
      if (f.server_uuid) set.add(normalizeServerName(f.server_uuid));
    }
    return set;
  }, [favorites]);

  const isFavorited = (serverName: string) => favoriteNames.has(serverName);

  const handleToggleFavorite = async (serverName: string) => {
    const displayName = displayNameMap.get(serverName) || serverName;
    const ps = proxyServerByNormalized.get(serverName);
    const serverId = ps?.server_id || displayName;

    // Check if already favorited
    const existing = favorites.find(
      (f) => normalizeServerName(f.server_name) === serverName || normalizeServerName(f.server_uuid) === serverName
    );

    if (existing) {
      await removeFavorite(existing.server_uuid);
    } else {
      await addFavorite({
        serverUuid: serverId,
        serverName: displayName,
        displayName,
      });
    }
    await refreshFavorites();
  };

  // Load OAuth statuses for proxy servers
  useEffect(() => {
    for (const ps of proxyServers) {
      if (ps.auth_type === "oauth") {
        getOAuthStatus(ps.server_id).then((status) => {
          setOauthStatuses((prev) => ({ ...prev, [ps.server_id]: status }));
        });
      }
    }
  }, [proxyServers]);

  // Build a map of normalizedName -> proxyServer for status dots
  const proxyServerByNormalized = useMemo(() => {
    const map = new Map<string, typeof proxyServers[0]>();
    for (const ps of proxyServers) {
      map.set(normalizeServerName(ps.server_id), ps);
      map.set(normalizeServerName(ps.display_name), ps);
    }
    return map;
  }, [proxyServers]);

  const detectedTools = tools.filter((t) => t.detected);
  const notDetected = tools.filter((t) => !t.detected);

  // Separate locked vs manageable detected tools
  const manageableTools = detectedTools.filter((t) => !LOCKED_TOOLS.has(t.id));
  const lockedDetected = detectedTools.filter((t) => LOCKED_TOOLS.has(t.id));

  // Build the grid data model — normalize server names to group across tools
  const { originalState, cellInfoMap, allServerNames, serverConfigMap, displayNameMap } = useMemo(() => {
    const state = new Map<string, boolean>();
    const info = new Map<string, CellInfo>();
    const normalizedNames = new Set<string>();
    // Map normalizedName -> configJson (from any source, for cross-tool installs)
    const configByServer = new Map<string, string>();
    // Map normalizedName -> prettiest display name (prefer unsanitized originals)
    const displayNames = new Map<string, string>();

    const trackDisplayName = (normalized: string, original: string) => {
      const existing = displayNames.get(normalized);
      // Prefer the longer/more readable name (the unsanitized original)
      if (!existing || original.length > existing.length) {
        displayNames.set(normalized, original);
      }
    };

    for (const cs of configuredServers) {
      // Skip locked tools from grid data
      if (LOCKED_TOOLS.has(cs.tool_id)) continue;
      const normalized = normalizeServerName(cs.server_name);
      const key = `${normalized}:${cs.tool_id}`;
      state.set(key, true);
      info.set(key, { configJson: cs.config_json, isCliOnly: cs.is_cli_only, actualName: cs.server_name });
      normalizedNames.add(normalized);
      trackDisplayName(normalized, cs.server_name);
      if (cs.config_json && !configByServer.has(normalized)) {
        configByServer.set(normalized, cs.config_json);
      }
    }

    for (const ds of disabledServers) {
      if (LOCKED_TOOLS.has(ds.tool_id)) continue;
      const normalized = normalizeServerName(ds.server_name);
      const key = `${normalized}:${ds.tool_id}`;
      state.set(key, false);
      info.set(key, { configJson: ds.config_json, isCliOnly: false, actualName: ds.server_name });
      normalizedNames.add(normalized);
      trackDisplayName(normalized, ds.server_name);
      if (ds.config_json && !configByServer.has(normalized)) {
        configByServer.set(normalized, ds.config_json);
      }
    }

    // Include proxy servers that aren't already in any tool config
    for (const ps of proxyServers) {
      const normalized = normalizeServerName(ps.server_id);
      if (!normalizedNames.has(normalized)) {
        normalizedNames.add(normalized);
        trackDisplayName(normalized, ps.display_name);
      } else {
        // Still track the display name (proxy may have a nicer name)
        trackDisplayName(normalized, ps.display_name);
      }
    }

    return {
      originalState: state,
      cellInfoMap: info,
      allServerNames: Array.from(normalizedNames).sort(),
      serverConfigMap: configByServer,
      displayNameMap: displayNames,
    };
  }, [configuredServers, disabledServers, proxyServers]);

  const cellKey = (serverName: string, toolId: string) => `${serverName}:${toolId}`;

  // Returns: true = enabled, false = disabled, null = not present in tool
  const getOriginalState = useCallback(
    (serverName: string, toolId: string): boolean | null => {
      const key = cellKey(serverName, toolId);
      const original = originalState.get(key);
      return original !== undefined ? original : null;
    },
    [originalState]
  );

  const getEffectiveState = useCallback(
    (serverName: string, toolId: string): boolean => {
      const key = cellKey(serverName, toolId);
      const pending = pendingChanges.get(key);
      if (pending) {
        return pending.action === "enable" || pending.action === "add";
      }
      const original = originalState.get(key);
      return original === true;
    },
    [originalState, pendingChanges]
  );

  const isChanged = useCallback(
    (serverName: string, toolId: string): boolean => {
      return pendingChanges.has(cellKey(serverName, toolId));
    },
    [pendingChanges]
  );

  const handleCellToggle = (serverName: string, toolId: string) => {
    const key = cellKey(serverName, toolId);
    const original = getOriginalState(serverName, toolId);
    const currentEffective = getEffectiveState(serverName, toolId);
    const targetChecked = !currentEffective;

    setPendingChanges((prev) => {
      const next = new Map(prev);

      if (original === null) {
        // Cell is empty — toggling adds or removes
        if (targetChecked) {
          const configJson = serverConfigMap.get(serverName) || null;
          const displayName = displayNameMap.get(serverName) || serverName;
          next.set(key, {
            serverName,
            toolId,
            action: "add",
            cellInfo: { configJson, isCliOnly: false, actualName: displayName },
          });
        } else {
          // Unchecking a pending add — remove the change
          next.delete(key);
        }
      } else if (targetChecked === original) {
        // Toggling back to original — remove pending change
        next.delete(key);
      } else {
        const ci = cellInfoMap.get(key) || { configJson: null, isCliOnly: false, actualName: serverName };
        next.set(key, {
          serverName,
          toolId,
          action: targetChecked ? "enable" : "disable",
          cellInfo: ci,
        });
      }

      return next;
    });
  };

  const handleSave = async () => {
    const changes = Array.from(pendingChanges.values());
    if (changes.length === 0) return;

    setSaving(true);
    setSaveProgress({ current: 0, total: changes.length, currentName: "" });

    let successCount = 0;
    for (let i = 0; i < changes.length; i++) {
      const change = changes[i];
      setSaveProgress({ current: i + 1, total: changes.length, currentName: change.serverName });

      try {
        // Use the actual name stored in the tool's config (not the normalized grid key)
        const actualName = change.cellInfo.actualName || change.serverName;
        if (change.action === "enable") {
          await enableServer(change.toolId, actualName);
          successCount++;
        } else if (change.action === "disable") {
          let configJson = change.cellInfo.configJson;
          if (!configJson && change.cellInfo.isCliOnly) {
            try {
              configJson = await fetchCliServerConfig(change.toolId, actualName);
            } catch {
              showToast(`Failed to fetch config for ${change.serverName}`, "error");
              continue;
            }
          }
          if (!configJson) {
            showToast(`No config available for ${change.serverName}`, "error");
            continue;
          }
          await disableServer(change.toolId, actualName, configJson);
          successCount++;
        } else if (change.action === "add") {
          // Always use proxy install path
          let ps = proxyServerByNormalized.get(change.serverName);
          if (!ps) {
            // No proxy server yet — create one from existing config
            try {
              const status = await daemonStatus();
              if (!status.running) await startDaemon();

              const displayName = displayNameMap.get(change.serverName) || change.serverName;
              let configJson = change.cellInfo.configJson;
              if (!configJson) {
                configJson = serverConfigMap.get(change.serverName) || null;
              }

              // Parse config to determine upstream details
              let upstreamCommand: string | undefined;
              let upstreamArgs: string | undefined;
              let upstreamUrl: string | undefined;
              if (configJson) {
                try {
                  const parsed = JSON.parse(configJson);
                  if (parsed.command) {
                    upstreamCommand = parsed.command;
                    upstreamArgs = (parsed.args || []).join(" ");
                  } else if (parsed.url) {
                    upstreamUrl = parsed.url;
                  }
                } catch { /* ignore parse errors */ }
              }

              const serverId = change.serverName;
              await registerProxyServer({
                serverId,
                displayName,
                authType: "none",
                upstreamUrl,
                upstreamCommand,
                upstreamArgs,
              });

              // Use the newly created proxy
              const result = await installProxyToTool(change.toolId, serverId, serverId);
              if (result.success) {
                successCount++;
                if (result.needs_restart) addPendingRestart(change.toolId);
              } else {
                showToast(result.message, "error");
              }
            } catch (e) {
              showToast(`Failed to create proxy for ${change.serverName}: ${e}`, "error");
            }
          } else {
            const result = await installProxyToTool(change.toolId, ps.server_id, ps.server_id);
            if (result.success) {
              successCount++;
              if (result.needs_restart) addPendingRestart(change.toolId);
            } else {
              showToast(result.message, "error");
            }
          }
        }
      } catch (e) {
        showToast(`Failed: ${change.serverName} — ${e}`, "error");
      }
    }

    await refreshConfiguredServers();
    await refreshDisabledServers();
    await refreshProxyServers();
    setPendingChanges(new Map());
    setSaving(false);

    if (successCount === changes.length) {
      showToast(`Saved ${successCount} change${successCount > 1 ? "s" : ""} successfully`, "success");
    } else {
      showToast(`Saved ${successCount}/${changes.length} changes`, "error");
    }
  };

  // Compute which tools a server is installed in (for delete confirmation)
  const getServerToolNames = useCallback(
    (serverName: string): string[] => {
      const toolIds = new Set<string>();
      for (const cs of configuredServers) {
        if (normalizeServerName(cs.server_name) === serverName) {
          toolIds.add(cs.tool_id);
        }
      }
      for (const ds of disabledServers) {
        if (normalizeServerName(ds.server_name) === serverName) {
          toolIds.add(ds.tool_id);
        }
      }
      return Array.from(toolIds).map((tid) => {
        const tool = tools.find((t) => t.id === tid);
        return tool ? tool.display_name : tid;
      });
    },
    [configuredServers, disabledServers, tools]
  );

  // Collect all actual (non-normalized) name variants for a normalized server key
  const getServerNameVariants = useCallback(
    (normalizedName: string): string[] => {
      const variants = new Set<string>();
      for (const cs of configuredServers) {
        if (normalizeServerName(cs.server_name) === normalizedName) {
          variants.add(cs.server_name);
        }
      }
      for (const ds of disabledServers) {
        if (normalizeServerName(ds.server_name) === normalizedName) {
          variants.add(ds.server_name);
        }
      }
      // Also include the normalized name itself and the display name
      variants.add(normalizedName);
      const display = displayNameMap.get(normalizedName);
      if (display) variants.add(display);
      // Include proxy server_id so proxy records get cleaned up
      const ps = proxyServerByNormalized.get(normalizedName);
      if (ps) {
        variants.add(ps.server_id);
        variants.add(ps.display_name);
      }
      return Array.from(variants);
    },
    [configuredServers, disabledServers, displayNameMap]
  );

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      const displayName = displayNameMap.get(deleteTarget) || deleteTarget;
      const nameVariants = getServerNameVariants(deleteTarget);
      const removedFrom = await deleteServer(nameVariants);
      await refreshConfiguredServers();
      await refreshDisabledServers();
      await refreshProxyServers();
      await refreshFavorites();
      // Clear any pending changes for this server
      setPendingChanges((prev) => {
        const next = new Map(prev);
        for (const key of next.keys()) {
          if (key.startsWith(deleteTarget + ":")) next.delete(key);
        }
        return next;
      });
      showToast(
        removedFrom.length > 0
          ? `Deleted "${displayName}" from ${removedFrom.length} app${removedFrom.length > 1 ? "s" : ""}`
          : `Deleted "${displayName}"`,
        "success"
      );
    } catch (e) {
      showToast(`Failed to delete: ${e}`, "error");
    }
    setDeleting(false);
    setDeleteTarget(null);
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Dashboard</h1>
        <div className="flex gap-2">
          <button
            onClick={() => {
              refreshConfiguredServers();
              refreshDisabledServers();
            }}
            disabled={configuredServersLoading}
            className="px-3 py-1.5 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors disabled:opacity-50"
          >
            {configuredServersLoading ? "Scanning..." : "Rescan Configs"}
          </button>
          <button
            onClick={refreshTools}
            disabled={toolsLoading}
            className="px-3 py-1.5 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors disabled:opacity-50"
          >
            {toolsLoading ? "Scanning..." : "Rescan Tools"}
          </button>
        </div>
      </div>

      {/* Detected Tools */}
      <section className="mb-8">
        <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
          Detected Tools
        </h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          {manageableTools.map((tool) => (
            <div
              key={tool.id}
              className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg px-3 py-2 flex items-center gap-2"
            >
              <div className="w-2 h-2 rounded-full bg-green-400 shrink-0" />
              <span className="text-sm font-medium flex-1">{tool.display_name}</span>
              <span className="text-xs text-brightwing-gray-500 font-mono">{tool.short_name}</span>
            </div>
          ))}
          {notDetected.filter((t) => !LOCKED_TOOLS.has(t.id)).map((tool) => (
            <div
              key={tool.id}
              className="bg-brightwing-gray-800/50 border border-brightwing-gray-700/50 rounded-lg px-3 py-2 flex items-center gap-2 opacity-50"
            >
              <div className="w-2 h-2 rounded-full bg-brightwing-gray-600 shrink-0" />
              <span className="text-sm font-medium text-brightwing-gray-500 flex-1">{tool.display_name}</span>
              <span className="text-xs text-brightwing-gray-600 font-mono">N/A</span>
            </div>
          ))}
        </div>
        {lockedDetected.length > 0 && (
          <p className="text-xs text-brightwing-gray-500 mt-2">
            {lockedDetected.map((t) => t.display_name).join(", ")}{" "}
            {lockedDetected.length === 1 ? "is" : "are"} installed but{" "}
            {lockedDetected.length === 1 ? "its" : "their"} MCP servers cannot be managed externally.
          </p>
        )}
      </section>

      {/* MCP Servers Grid */}
      <section>
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-4">
            <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider">
              MCP Servers ({allServerNames.length})
            </h2>
            <button
              onClick={() => setShowFavoritesOnly(!showFavoritesOnly)}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs transition-colors ${
                showFavoritesOnly
                  ? "bg-orange-500/15 text-orange-400 border border-orange-500/30"
                  : "bg-brightwing-gray-700/50 text-brightwing-gray-500 hover:text-brightwing-gray-300 border border-transparent"
              }`}
            >
              <svg className="w-3.5 h-3.5" fill={showFavoritesOnly ? "currentColor" : "none"} viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M11.48 3.499a.562.562 0 0 1 1.04 0l2.125 5.111a.563.563 0 0 0 .475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 0 0-.182.557l1.285 5.385a.562.562 0 0 1-.84.61l-4.725-2.885a.562.562 0 0 0-.586 0L6.982 20.54a.562.562 0 0 1-.84-.61l1.285-5.386a.562.562 0 0 0-.182-.557l-4.204-3.602a.562.562 0 0 1 .321-.988l5.518-.442a.563.563 0 0 0 .475-.345L11.48 3.5Z" />
              </svg>
              Favorites
            </button>
          </div>
          {pendingChanges.size > 0 && (
            <button
              onClick={handleSave}
              className="px-4 py-1.5 text-sm font-medium bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors"
            >
              Save Changes ({pendingChanges.size})
            </button>
          )}
        </div>

        {/* Tool-specific warnings */}
        {(() => {
          const affected = manageableTools.filter((t) => TOOL_WARNINGS[t.id]);
          if (affected.length === 0) return null;
          return (
            <div className="mb-3 space-y-2">
              {affected.map((tool) => {
                const w = TOOL_WARNINGS[tool.id];
                return (
                  <div
                    key={tool.id}
                    className="flex items-start gap-2.5 bg-amber-500/5 border border-amber-500/20 rounded-lg px-4 py-3"
                  >
                    <svg className="w-4 h-4 text-amber-400 mt-0.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
                    </svg>
                    <div className="flex-1 min-w-0">
                      <p className="text-xs text-amber-300 font-medium">
                        {tool.display_name}: {w.short}
                      </p>
                      <p className="text-xs text-brightwing-gray-400 mt-0.5">
                        {w.detail}{" "}
                        <a
                          href={w.link}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-amber-400/70 hover:text-amber-300 underline underline-offset-2"
                        >
                          Track issue &rarr;
                        </a>
                      </p>
                    </div>
                  </div>
                );
              })}
            </div>
          );
        })()}

        {(() => {
          const filteredServerNames = showFavoritesOnly
            ? allServerNames.filter((n) => isFavorited(n))
            : allServerNames;

          return configuredServersLoading ? (
          <p className="text-brightwing-gray-500 text-sm">Scanning config files...</p>
        ) : allServerNames.length === 0 ? (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
            <p className="text-brightwing-gray-400">
              No MCP servers configured in any tool.
            </p>
            <p className="text-brightwing-gray-500 text-sm mt-1">
              Use Search to find and install MCP servers.
            </p>
          </div>
        ) : filteredServerNames.length === 0 && showFavoritesOnly ? (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
            <p className="text-brightwing-gray-400">
              No favorite servers yet.
            </p>
            <p className="text-brightwing-gray-500 text-sm mt-1">
              Click the star next to a server to add it to your favorites.
            </p>
          </div>
        ) : (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-brightwing-gray-700">
                    <th className="w-8 px-2 py-3" />
                    <th className="text-left px-4 py-3 text-brightwing-gray-400 font-medium text-xs uppercase tracking-wider sticky left-0 bg-brightwing-gray-800 z-10">
                      Server
                    </th>
                    {manageableTools.map((tool) => {
                      const warning = TOOL_WARNINGS[tool.id];
                      return (
                        <th
                          key={tool.id}
                          className={`px-3 py-3 text-center font-mono font-medium text-xs uppercase tracking-wider min-w-[60px] ${
                            warning ? "text-amber-400/80" : "text-brightwing-gray-400"
                          }`}
                          title={warning ? `${tool.display_name} — ${warning.short}` : tool.display_name}
                        >
                          <span className="inline-flex items-center gap-1 justify-center">
                            {tool.short_name}
                            {warning && (
                              <svg className="w-3 h-3 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                                <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
                              </svg>
                            )}
                          </span>
                        </th>
                      );
                    })}
                    <th className="px-3 py-3 w-10" />
                  </tr>
                </thead>
                <tbody>
                  {filteredServerNames.map((serverName, idx) => (
                    <tr
                      key={serverName}
                      className={`border-b border-brightwing-gray-700/50 ${
                        idx % 2 === 0 ? "" : "bg-brightwing-gray-800/50"
                      }`}
                    >
                      <td className="w-8 px-2 py-2.5 text-center">
                        <button
                          onClick={() => handleToggleFavorite(serverName)}
                          className="p-0.5 transition-colors"
                          title={isFavorited(serverName) ? "Remove from favorites" : "Add to favorites"}
                        >
                          <svg
                            className={`w-4 h-4 ${isFavorited(serverName) ? "text-orange-400" : "text-brightwing-gray-600 hover:text-brightwing-gray-400"}`}
                            fill={isFavorited(serverName) ? "currentColor" : "none"}
                            viewBox="0 0 24 24"
                            stroke="currentColor"
                            strokeWidth={1.5}
                          >
                            <path strokeLinecap="round" strokeLinejoin="round" d="M11.48 3.499a.562.562 0 0 1 1.04 0l2.125 5.111a.563.563 0 0 0 .475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 0 0-.182.557l1.285 5.385a.562.562 0 0 1-.84.61l-4.725-2.885a.562.562 0 0 0-.586 0L6.982 20.54a.562.562 0 0 1-.84-.61l1.285-5.386a.562.562 0 0 0-.182-.557l-4.204-3.602a.562.562 0 0 1 .321-.988l5.518-.442a.563.563 0 0 0 .475-.345L11.48 3.5Z" />
                          </svg>
                        </button>
                      </td>
                      <td className="px-4 py-2.5 font-mono text-xs sticky left-0 bg-inherit z-10 max-w-[200px]">
                        {(() => {
                          const ps = proxyServerByNormalized.get(serverName);
                          const oauthStatus = ps ? oauthStatuses[ps.server_id] : null;
                          const dotColor = ps
                            ? ps.auth_type === "oauth"
                              ? oauthStatus?.status === "connected"
                                ? "bg-green-400"
                                : oauthStatus?.status === "expired"
                                ? "bg-amber-400"
                                : "bg-red-400"
                              : ps.auth_type === "api_key"
                              ? "bg-green-400"
                              : "bg-green-400"
                            : "bg-red-400";
                          return (
                            <button
                              onClick={() => {
                                setServerDetailId(serverName);
                                setView("server-detail");
                              }}
                              className="flex items-center gap-1.5 hover:text-brightwing-blue transition-colors truncate text-left"
                              title={displayNameMap.get(serverName) || serverName}
                            >
                              <span className={`w-1.5 h-1.5 rounded-full ${dotColor} shrink-0`} />
                              <span className="truncate">{displayNameMap.get(serverName) || serverName}</span>
                            </button>
                          );
                        })()}
                      </td>
                      {manageableTools.map((tool) => {
                        const effective = getEffectiveState(serverName, tool.id);
                        const changed = isChanged(serverName, tool.id);
                        const hasConfig = serverConfigMap.has(serverName);
                        const isProxy = proxyServerByNormalized.has(serverName);
                        const original = getOriginalState(serverName, tool.id);
                        // Can interact if: exists in this tool, OR we have config to install it, OR it's a proxy server
                        const canInteract = original !== null || hasConfig || isProxy;

                        return (
                          <td key={tool.id} className="px-3 py-2.5 text-center">
                            {canInteract ? (
                              <button
                                onClick={() => handleCellToggle(serverName, tool.id)}
                                className={`w-5 h-5 rounded border-2 inline-flex items-center justify-center transition-all ${
                                  changed
                                    ? "ring-2 ring-amber-400/50 ring-offset-1 ring-offset-brightwing-gray-800"
                                    : ""
                                } ${
                                  effective
                                    ? "bg-green-500 border-green-500"
                                    : "bg-transparent border-brightwing-gray-600 hover:border-brightwing-gray-400"
                                }`}
                              >
                                {effective && (
                                  <svg className="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                                    <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                                  </svg>
                                )}
                              </button>
                            ) : (
                              <span className="text-brightwing-gray-700">&mdash;</span>
                            )}
                          </td>
                        );
                      })}
                      <td className="px-3 py-2.5 text-center">
                        <button
                          onClick={() => setDeleteTarget(serverName)}
                          className="p-1 text-brightwing-gray-600 hover:text-red-400 transition-colors"
                          title="Delete server"
                        >
                          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                            <path strokeLinecap="round" strokeLinejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
                          </svg>
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        );
        })()}
        <p className="text-xs text-brightwing-gray-500 mt-4">
          Check/uncheck to enable/disable MCP servers, then click Save Changes to apply.
        </p>
      </section>

      {/* Delete confirmation modal */}
      {deleteTarget && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-xl p-6 max-w-md mx-4 shadow-2xl">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-10 h-10 rounded-full bg-red-500/10 flex items-center justify-center shrink-0">
                <svg className="w-5 h-5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
                </svg>
              </div>
              <h3 className="text-lg font-semibold">Delete Server</h3>
            </div>

            <p className="text-sm text-brightwing-gray-300 mb-4">
              This will permanently remove <strong className="text-white">{displayNameMap.get(deleteTarget) || deleteTarget}</strong> and
              cannot be undone. You would need to add it from scratch again.
            </p>

            {(() => {
              const affectedTools = getServerToolNames(deleteTarget);
              return affectedTools.length > 0 ? (
                <div className="bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-lg p-3 mb-5">
                  <p className="text-xs text-brightwing-gray-400 mb-2">Will be removed from:</p>
                  <div className="flex flex-wrap gap-1.5">
                    {affectedTools.map((name) => (
                      <span key={name} className="text-xs bg-red-500/10 text-red-300 px-2 py-0.5 rounded-full">
                        {name}
                      </span>
                    ))}
                  </div>
                </div>
              ) : null;
            })()}

            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setDeleteTarget(null)}
                disabled={deleting}
                className="px-4 py-2 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleDeleteConfirm}
                disabled={deleting}
                className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 text-white rounded-md transition-colors disabled:opacity-50"
              >
                {deleting ? "Deleting..." : "Yes, Delete"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Saving modal */}
      {saving && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-xl p-6 max-w-sm mx-4 shadow-2xl">
            <h3 className="text-lg font-semibold mb-4">Saving Changes</h3>
            <div className="mb-3">
              <div className="flex justify-between text-sm text-brightwing-gray-400 mb-1">
                <span>{saveProgress.currentName}</span>
                <span>{saveProgress.current}/{saveProgress.total}</span>
              </div>
              <div className="w-full bg-brightwing-gray-700 rounded-full h-2">
                <div
                  className="bg-brightwing-blue h-2 rounded-full transition-all duration-300"
                  style={{ width: `${(saveProgress.current / saveProgress.total) * 100}%` }}
                />
              </div>
            </div>
            <p className="text-xs text-brightwing-gray-500">
              Applying changes to tool configurations...
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
