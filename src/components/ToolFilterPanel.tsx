import { useEffect, useState, useMemo, useRef } from "react";
import { useStore } from "../store";
import * as tauri from "../lib/tauri";
import type { ProxyServer, ToolFilterEntry } from "../lib/types";

interface Props {
  server: ProxyServer;
  onBack: () => void;
}

export default function ToolFilterPanel({ server, onBack }: Props) {
  const { tools, activeFilter, activeFilterLoading, loadToolFilter, toggleToolFilter, showToast } =
    useStore();
  const [search, setSearch] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const didAutoDiscover = useRef(false);
  // Track which app tabs have had filter changes (by tool_id)
  const [changedTabs, setChangedTabs] = useState<Set<string>>(new Set());

  const detectedTools = tools.filter((t) => t.detected);

  // Active tab — the tool_id for per-app filtering (e.g. "claude_code", "cursor")
  const [activeTab, setActiveTab] = useState<string>(
    detectedTools.length > 0 ? detectedTools[0].id : "_all"
  );

  // Load filter whenever server or tab changes
  useEffect(() => {
    loadToolFilter(server.server_id, activeTab === "_all" ? undefined : activeTab);
  }, [server.server_id, activeTab, loadToolFilter]);

  // Auto-discover tools on first mount if none are cached
  useEffect(() => {
    if (didAutoDiscover.current) return;
    if (activeFilterLoading) return; // wait for initial load to finish

    // If we have tools already, no need to auto-discover
    if (activeFilter.length > 0) return;

    // Only auto-discover for HTTP servers (stdio servers discover via the proxy)
    if (!server.upstream_url) return;

    didAutoDiscover.current = true;
    setDiscovering(true);
    setDiscoveryError(null);

    tauri.discoverUpstreamTools(server.server_id)
      .then((discovered) => {
        if (discovered.length > 0) {
          const toolId = activeTab === "_all" ? undefined : activeTab;
          loadToolFilter(server.server_id, toolId);
        } else {
          setDiscoveryError("Server returned no tools.");
        }
      })
      .catch((e) => {
        setDiscoveryError(String(e));
      })
      .finally(() => {
        setDiscovering(false);
      });
  }, [activeFilterLoading, activeFilter.length, server.server_id, server.upstream_url]);

  const filtered = useMemo(() => {
    if (!search.trim()) return activeFilter;
    const q = search.toLowerCase();
    return activeFilter.filter((t) => t.tool_name.toLowerCase().includes(q));
  }, [activeFilter, search]);

  const enabledCount = activeFilter.filter((t) => t.enabled).length;
  const totalCount = activeFilter.length;
  const enabledTokens = activeFilter
    .filter((t) => t.enabled)
    .reduce((sum, t) => sum + t.token_estimate, 0);
  const totalTokens = activeFilter.reduce((sum, t) => sum + t.token_estimate, 0);

  const markTabChanged = (tab: string) => {
    setChangedTabs((prev) => {
      const next = new Set(prev);
      next.add(tab);
      return next;
    });
  };

  const handleToggle = (entry: ToolFilterEntry) => {
    const toolId = activeTab === "_all" ? undefined : activeTab;
    toggleToolFilter(server.server_id, entry.tool_name, !entry.enabled, entry.token_estimate, toolId);
    markTabChanged(activeTab);
  };

  const handleEnableAll = async () => {
    const toolId = activeTab === "_all" ? undefined : activeTab;
    const allTools = activeFilter.map((t) => t.tool_name);
    try {
      await tauri.setToolFilterBulk(server.server_id, allTools, toolId);
      loadToolFilter(server.server_id, toolId);
      markTabChanged(activeTab);
      showToast("All tools enabled", "success");
    } catch (e) {
      showToast(`Failed: ${e}`, "error");
    }
  };

  const handleDisableAll = async () => {
    const toolId = activeTab === "_all" ? undefined : activeTab;
    try {
      await tauri.setToolFilterBulk(server.server_id, [], toolId);
      loadToolFilter(server.server_id, toolId);
      markTabChanged(activeTab);
      showToast("All tools disabled", "success");
    } catch (e) {
      showToast(`Failed: ${e}`, "error");
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    setDiscoveryError(null);
    try {
      const discovered = await tauri.discoverUpstreamTools(server.server_id);
      showToast(`Refreshed: ${discovered.length} tools`, "success");
      const toolId = activeTab === "_all" ? undefined : activeTab;
      loadToolFilter(server.server_id, toolId);
    } catch (e) {
      showToast(`Refresh failed: ${e}`, "error");
    }
    setRefreshing(false);
  };

  const tokenPercent = totalTokens > 0 ? (enabledTokens / totalTokens) * 100 : 0;

  // Show full-page discovering state when we have no tools yet
  const showDiscoveringState = (discovering || activeFilterLoading) && activeFilter.length === 0;

  return (
    <div>
      {/* Header */}
      <div className="flex items-center gap-3 mb-4">
        <button
          onClick={onBack}
          className="p-1.5 rounded-md hover:bg-brightwing-gray-700 transition-colors"
        >
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M10.5 19.5 3 12m0 0 7.5-7.5M3 12h18" />
          </svg>
        </button>
        <div className="flex-1">
          <h1 className="text-xl font-semibold">{server.display_name}</h1>
          <p className="text-sm text-brightwing-gray-400">Filter Tools</p>
        </div>
        {!showDiscoveringState && (
          <button
            onClick={handleRefresh}
            disabled={refreshing}
            className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors disabled:opacity-50 inline-flex items-center gap-1.5"
          >
            <svg className={`w-3.5 h-3.5 ${refreshing ? "animate-spin" : ""}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182" />
            </svg>
            {refreshing ? "Refreshing..." : "Refresh Tools List"}
          </button>
        )}
      </div>

      {/* Full-page discovering state */}
      {showDiscoveringState ? (
        <div className="flex flex-col items-center justify-center py-20">
          <svg className="w-8 h-8 text-brightwing-blue animate-spin mb-4" viewBox="0 0 24 24" fill="none">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
          <p className="text-brightwing-gray-300 font-medium mb-1">Discovering tools...</p>
          <p className="text-xs text-brightwing-gray-500 text-center max-w-xs">
            Connecting to the upstream server to fetch the list of available tools. This may take a moment.
          </p>
        </div>
      ) : (
        <>
          {/* Per-app tabs */}
          {detectedTools.length > 0 && (
            <div className="flex gap-1 mb-4 overflow-x-auto border-b border-brightwing-gray-700 pb-px">
              {detectedTools.map((tool) => (
                <button
                  key={tool.id}
                  onClick={() => setActiveTab(tool.id)}
                  className={`px-3 py-2 text-xs font-medium rounded-t-md transition-colors whitespace-nowrap ${
                    activeTab === tool.id
                      ? "bg-brightwing-gray-800 text-white border border-brightwing-gray-700 border-b-brightwing-gray-800 -mb-px"
                      : "text-brightwing-gray-500 hover:text-brightwing-gray-300 hover:bg-brightwing-gray-800/50"
                  }`}
                >
                  {tool.short_name}
                </button>
              ))}
            </div>
          )}

          {/* Token budget bar */}
          {totalCount > 0 && (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4 mb-4">
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm text-brightwing-gray-300">
                  {enabledCount} of {totalCount} tools enabled
                </span>
                <span className="text-sm font-mono text-brightwing-gray-400">
                  ~{enabledTokens.toLocaleString()} / {totalTokens.toLocaleString()} tokens
                </span>
              </div>
              <div className="w-full bg-brightwing-gray-700 rounded-full h-2.5">
                <div
                  className="h-2.5 rounded-full transition-all duration-300 bg-brightwing-blue"
                  style={{ width: `${tokenPercent}%` }}
                />
              </div>
              <div className="flex gap-2 mt-3">
                <button
                  onClick={handleEnableAll}
                  className="px-3 py-1 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded transition-colors"
                >
                  Enable All
                </button>
                <button
                  onClick={handleDisableAll}
                  className="px-3 py-1 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded transition-colors"
                >
                  Disable All
                </button>
              </div>
              {changedTabs.size > 0 && (
                <p className="mt-3 text-xs text-yellow-400">
                  Restart {changedTabs.size === 1
                    ? (detectedTools.find((t) => t.id === [...changedTabs][0])?.short_name ?? "the affected app")
                    : "affected apps"
                  } to apply the updated tool list and reclaim token savings.
                </p>
              )}
            </div>
          )}

          {/* Search */}
          {totalCount > 0 && (
            <div className="mb-4">
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Filter tools..."
                className="w-full px-3 py-2 text-sm bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md focus:outline-none focus:ring-2 focus:ring-brightwing-blue/50 placeholder-brightwing-gray-500"
              />
            </div>
          )}

          {/* Tool list */}
          {filtered.length === 0 ? (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
              {discoveryError ? (
                <>
                  <p className="text-red-400 text-sm font-medium mb-1">Discovery failed</p>
                  <p className="text-xs text-brightwing-gray-500 mb-3">{discoveryError}</p>
                  {server.upstream_url && (
                    <button
                      onClick={handleRefresh}
                      disabled={refreshing}
                      className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors disabled:opacity-50"
                    >
                      {refreshing ? "Retrying..." : "Retry"}
                    </button>
                  )}
                </>
              ) : activeFilter.length === 0 ? (
                <>
                  <p className="text-brightwing-gray-400 mb-2">No tools cached yet.</p>
                  {server.upstream_url ? (
                    <button
                      onClick={handleRefresh}
                      disabled={refreshing}
                      className="px-3 py-1.5 text-xs bg-brightwing-blue/20 text-brightwing-blue hover:bg-brightwing-blue/30 border border-brightwing-blue/30 rounded-md transition-colors disabled:opacity-50"
                    >
                      {refreshing ? "Discovering..." : "Discover Tools"}
                    </button>
                  ) : (
                    <p className="text-xs text-brightwing-gray-500">
                      Tools will appear after the proxy connects to the upstream server.
                    </p>
                  )}
                </>
              ) : (
                <p className="text-brightwing-gray-400">No tools match your search.</p>
              )}
            </div>
          ) : (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg divide-y divide-brightwing-gray-700/50">
              {filtered.map((entry) => (
                <div
                  key={entry.tool_name}
                  className="flex items-center gap-3 px-4 py-3 hover:bg-brightwing-gray-700/30 transition-colors"
                >
                  <button
                    onClick={() => handleToggle(entry)}
                    className={`w-5 h-5 rounded border-2 shrink-0 inline-flex items-center justify-center transition-all ${
                      entry.enabled
                        ? "bg-green-500 border-green-500"
                        : "bg-transparent border-brightwing-gray-600 hover:border-brightwing-gray-400"
                    }`}
                  >
                    {entry.enabled && (
                      <svg className="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                      </svg>
                    )}
                  </button>
                  <div className="flex-1 min-w-0">
                    <span className="text-sm font-mono">{entry.tool_name}</span>
                  </div>
                  <span className="text-xs text-brightwing-gray-500 font-mono shrink-0">
                    ~{entry.token_estimate} tok
                  </span>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
