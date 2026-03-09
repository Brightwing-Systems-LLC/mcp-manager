import { useEffect, useState, useMemo } from "react";
import { useStore } from "../store";
import * as tauri from "../lib/tauri";
import type { ProxyServer, ToolFilterEntry } from "../lib/types";

interface Props {
  server: ProxyServer;
  onBack: () => void;
}

export default function ToolFilterPanel({ server, onBack }: Props) {
  const { activeFilter, activeFilterLoading, loadToolFilter, toggleToolFilter, showToast } =
    useStore();
  const [search, setSearch] = useState("");
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    loadToolFilter(server.server_id);
  }, [server.server_id, loadToolFilter]);

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

  const handleToggle = (entry: ToolFilterEntry) => {
    toggleToolFilter(server.server_id, entry.tool_name, !entry.enabled, entry.token_estimate);
  };

  const handleEnableAll = async () => {
    for (const entry of activeFilter) {
      if (!entry.enabled) {
        await toggleToolFilter(server.server_id, entry.tool_name, true, entry.token_estimate);
      }
    }
    showToast("All tools enabled", "success");
  };

  const handleDisableAll = async () => {
    for (const entry of activeFilter) {
      if (entry.enabled) {
        await toggleToolFilter(server.server_id, entry.tool_name, false, entry.token_estimate);
      }
    }
    showToast("All tools disabled", "success");
  };

  const tokenPercent = totalTokens > 0 ? (enabledTokens / totalTokens) * 100 : 0;

  return (
    <div>
      {/* Header */}
      <div className="flex items-center gap-3 mb-6">
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
          <p className="text-sm text-brightwing-gray-400">Tool Filter</p>
        </div>
        {server.upstream_url && (
          <button
            onClick={async () => {
              setRefreshing(true);
              try {
                const tools = await tauri.discoverUpstreamTools(server.server_id);
                showToast(`Refreshed: ${tools.length} tools`, "success");
                loadToolFilter(server.server_id);
              } catch (e) {
                showToast(`Refresh failed: ${e}`, "error");
              }
              setRefreshing(false);
            }}
            disabled={refreshing}
            className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors disabled:opacity-50"
          >
            {refreshing ? "Refreshing..." : "Refresh"}
          </button>
        )}
      </div>

      {/* Token budget bar */}
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
      </div>

      {/* Search */}
      <div className="mb-4">
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Filter tools..."
          className="w-full px-3 py-2 text-sm bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md focus:outline-none focus:ring-2 focus:ring-brightwing-blue/50 placeholder-brightwing-gray-500"
        />
      </div>

      {/* Tool list */}
      {activeFilterLoading ? (
        <p className="text-brightwing-gray-500 text-sm">Loading tools...</p>
      ) : filtered.length === 0 ? (
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
          <p className="text-brightwing-gray-400">
            {activeFilter.length === 0
              ? "No tools cached yet. Tools will appear after the proxy connects to the upstream server."
              : "No tools match your search."}
          </p>
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
    </div>
  );
}
