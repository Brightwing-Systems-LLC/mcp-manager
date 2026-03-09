import { useEffect, useState, useMemo, useCallback } from "react";
import { useStore } from "../store";
import { fetchCliServerConfig, addServerToTool, getToolFilter } from "../lib/tauri";
import type { ToolFilterEntry } from "../lib/types";

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
const LOCKED_TOOLS = new Set(["claude_desktop"]);

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
    setView,
  } = useStore();

  const [pendingChanges, setPendingChanges] = useState<Map<string, PendingChange>>(new Map());
  const [saving, setSaving] = useState(false);
  const [saveProgress, setSaveProgress] = useState({ current: 0, total: 0, currentName: "" });
  const [proxyFilterSummaries, setProxyFilterSummaries] = useState<
    Record<string, { enabled: number; total: number; enabledTokens: number; totalTokens: number }>
  >({});

  useEffect(() => {
    refreshConfiguredServers();
    refreshDisabledServers();
  }, [refreshConfiguredServers, refreshDisabledServers]);

  // Load token budget summaries for proxy servers
  useEffect(() => {
    for (const ps of proxyServers) {
      getToolFilter(ps.server_id).then((entries: ToolFilterEntry[]) => {
        const enabled = entries.filter((e) => e.enabled).length;
        const enabledTokens = entries.filter((e) => e.enabled).reduce((s, e) => s + e.token_estimate, 0);
        const totalTokens = entries.reduce((s, e) => s + e.token_estimate, 0);
        setProxyFilterSummaries((prev) => ({
          ...prev,
          [ps.server_id]: { enabled, total: entries.length, enabledTokens, totalTokens },
        }));
      });
    }
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

    return {
      originalState: state,
      cellInfoMap: info,
      allServerNames: Array.from(normalizedNames).sort(),
      serverConfigMap: configByServer,
      displayNameMap: displayNames,
    };
  }, [configuredServers, disabledServers]);

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
          let configJson = change.cellInfo.configJson;
          if (!configJson) {
            // Try to get config from another tool
            configJson = serverConfigMap.get(change.serverName) || null;
          }
          if (!configJson) {
            showToast(`No config available to install ${change.serverName}`, "error");
            continue;
          }
          const result = await addServerToTool(change.toolId, change.serverName, configJson);
          if (result.success) {
            successCount++;
            if (result.needs_restart) addPendingRestart(change.toolId);
          } else {
            showToast(result.message, "error");
          }
        }
      } catch (e) {
        showToast(`Failed: ${change.serverName} — ${e}`, "error");
      }
    }

    await refreshConfiguredServers();
    await refreshDisabledServers();
    setPendingChanges(new Map());
    setSaving(false);

    if (successCount === changes.length) {
      showToast(`Saved ${successCount} change${successCount > 1 ? "s" : ""} successfully`, "success");
    } else {
      showToast(`Saved ${successCount}/${changes.length} changes`, "error");
    }
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

      {/* Proxy Servers Summary */}
      {proxyServers.length > 0 && (
        <section className="mb-8">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider">
              Proxy Servers ({proxyServers.length})
            </h2>
            <button
              onClick={() => setView("proxy")}
              className="text-xs text-brightwing-blue hover:text-brightwing-blue/80 transition-colors"
            >
              Manage &rarr;
            </button>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {proxyServers.map((ps) => {
              const summary = proxyFilterSummaries[ps.server_id];
              const tokenPercent = summary && summary.totalTokens > 0
                ? (summary.enabledTokens / summary.totalTokens) * 100
                : 0;
              return (
                <div
                  key={ps.server_id}
                  className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg px-4 py-3"
                >
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2 min-w-0">
                      <span className="font-medium text-sm truncate">{ps.display_name}</span>
                      <span
                        className={`text-xs px-1.5 py-0.5 rounded-full shrink-0 ${
                          ps.auth_type === "oauth"
                            ? "bg-purple-500/20 text-purple-300"
                            : ps.auth_type === "api_key"
                            ? "bg-amber-500/20 text-amber-300"
                            : "bg-brightwing-gray-700 text-brightwing-gray-400"
                        }`}
                      >
                        {ps.auth_type === "api_key" ? "API Key" : ps.auth_type === "oauth" ? "OAuth" : "None"}
                      </span>
                    </div>
                  </div>
                  {summary && summary.total > 0 && (
                    <div>
                      <div className="flex items-center justify-between text-xs text-brightwing-gray-500 mb-1">
                        <span>{summary.enabled}/{summary.total} tools</span>
                        <span className="font-mono">~{summary.enabledTokens.toLocaleString()} tok</span>
                      </div>
                      <div className="w-full bg-brightwing-gray-700 rounded-full h-1.5">
                        <div
                          className="h-1.5 rounded-full transition-all duration-300 bg-brightwing-blue"
                          style={{ width: `${tokenPercent}%` }}
                        />
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </section>
      )}

      {/* MCP Servers Grid */}
      <section>
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider">
            MCP Servers ({allServerNames.length})
          </h2>
          {pendingChanges.size > 0 && (
            <button
              onClick={handleSave}
              className="px-4 py-1.5 text-sm font-medium bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors"
            >
              Save Changes ({pendingChanges.size})
            </button>
          )}
        </div>

        {configuredServersLoading ? (
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
        ) : (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-brightwing-gray-700">
                    <th className="text-left px-4 py-3 text-brightwing-gray-400 font-medium text-xs uppercase tracking-wider sticky left-0 bg-brightwing-gray-800 z-10">
                      Server
                    </th>
                    {manageableTools.map((tool) => (
                      <th
                        key={tool.id}
                        className="px-3 py-3 text-center text-brightwing-gray-400 font-mono font-medium text-xs uppercase tracking-wider min-w-[60px]"
                        title={tool.display_name}
                      >
                        {tool.short_name}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {allServerNames.map((serverName, idx) => (
                    <tr
                      key={serverName}
                      className={`border-b border-brightwing-gray-700/50 ${
                        idx % 2 === 0 ? "" : "bg-brightwing-gray-800/50"
                      }`}
                    >
                      <td className="px-4 py-2.5 font-mono text-xs sticky left-0 bg-inherit z-10 max-w-[200px] truncate" title={displayNameMap.get(serverName) || serverName}>
                        {displayNameMap.get(serverName) || serverName}
                      </td>
                      {manageableTools.map((tool) => {
                        const effective = getEffectiveState(serverName, tool.id);
                        const changed = isChanged(serverName, tool.id);
                        const hasConfig = serverConfigMap.has(serverName);
                        const original = getOriginalState(serverName, tool.id);
                        // Can interact if: exists in this tool, OR we have config to install it
                        const canInteract = original !== null || hasConfig;

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
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
        <p className="text-xs text-brightwing-gray-500 mt-4">
          Check/uncheck to enable/disable MCP servers, then click Save Changes to apply.
        </p>
      </section>

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
