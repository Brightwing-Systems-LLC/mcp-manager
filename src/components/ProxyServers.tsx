import { useEffect, useState } from "react";
import { useStore } from "../store";
import type { ProxyServer } from "../lib/types";
import * as tauri from "../lib/tauri";
import ToolFilterPanel from "./ToolFilterPanel";
import OAuthConnect from "./OAuthConnect";
import DaemonStatus from "./DaemonStatus";

type SubView = "list" | "filter" | "add";

export default function ProxyServers() {
  const { proxyServers, proxyServersLoading, refreshProxyServers, tools, showToast, addPendingRestart } =
    useStore();

  const [subView, setSubView] = useState<SubView>("list");
  const [selectedServer, setSelectedServer] = useState<ProxyServer | null>(null);
  const [expandedServer, setExpandedServer] = useState<string | null>(null);
  const [proxyInstalls, setProxyInstalls] = useState<Record<string, string[]>>({});

  // Add form state
  const [newServerId, setNewServerId] = useState("");
  const [newDisplayName, setNewDisplayName] = useState("");
  const [newAuthType, setNewAuthType] = useState("api_key");
  const [newUpstreamUrl, setNewUpstreamUrl] = useState("");
  const [addSaving, setAddSaving] = useState(false);

  useEffect(() => {
    refreshProxyServers();
  }, [refreshProxyServers]);

  // Load proxy installs for expanded server
  useEffect(() => {
    if (expandedServer) {
      tauri.getProxyInstalls(expandedServer).then((installs) => {
        setProxyInstalls((prev) => ({ ...prev, [expandedServer]: installs }));
      });
    }
  }, [expandedServer]);

  const detectedTools = tools.filter((t) => t.detected);

  const handleAddServer = async () => {
    if (!newServerId.trim() || !newDisplayName.trim()) {
      showToast("Server ID and display name are required", "error");
      return;
    }
    setAddSaving(true);
    try {
      await tauri.registerProxyServer({
        serverId: newServerId.trim(),
        displayName: newDisplayName.trim(),
        authType: newAuthType,
        upstreamUrl: newUpstreamUrl.trim() || undefined,
      });
      showToast(`Registered ${newDisplayName}`, "success");
      setNewServerId("");
      setNewDisplayName("");
      setNewUpstreamUrl("");
      setSubView("list");
      await refreshProxyServers();
    } catch (e) {
      showToast(`Failed to register: ${e}`, "error");
    }
    setAddSaving(false);
  };

  const handleDelete = async (serverId: string) => {
    try {
      await tauri.unregisterProxyServer(serverId);
      showToast(`Removed ${serverId}`, "success");
      await refreshProxyServers();
    } catch (e) {
      showToast(`Failed to remove: ${e}`, "error");
    }
  };

  const handleInstallToTool = async (serverId: string, toolId: string, configKey: string) => {
    try {
      const result = await tauri.installProxyToTool(toolId, serverId, configKey);
      if (result.success) {
        showToast(result.message, "success");
        if (result.needs_restart) addPendingRestart(toolId);
        // Refresh installs
        const installs = await tauri.getProxyInstalls(serverId);
        setProxyInstalls((prev) => ({ ...prev, [serverId]: installs }));
      } else {
        showToast(result.message, "error");
      }
    } catch (e) {
      showToast(`Install failed: ${e}`, "error");
    }
  };

  const handleUninstallFromTool = async (serverId: string, toolId: string, configKey: string) => {
    try {
      const result = await tauri.uninstallProxyFromTool(toolId, serverId, configKey);
      if (result.success) {
        showToast(result.message, "success");
        if (result.needs_restart) addPendingRestart(toolId);
        const installs = await tauri.getProxyInstalls(serverId);
        setProxyInstalls((prev) => ({ ...prev, [serverId]: installs }));
      } else {
        showToast(result.message, "error");
      }
    } catch (e) {
      showToast(`Uninstall failed: ${e}`, "error");
    }
  };

  if (subView === "filter" && selectedServer) {
    return (
      <ToolFilterPanel
        server={selectedServer}
        onBack={() => {
          setSubView("list");
          setSelectedServer(null);
        }}
      />
    );
  }

  if (subView === "add") {
    return (
      <div>
        <div className="flex items-center gap-3 mb-6">
          <button
            onClick={() => setSubView("list")}
            className="p-1.5 rounded-md hover:bg-brightwing-gray-700 transition-colors"
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M10.5 19.5 3 12m0 0 7.5-7.5M3 12h18" />
            </svg>
          </button>
          <h1 className="text-xl font-semibold">Register Proxy Server</h1>
        </div>

        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-6 max-w-lg space-y-4">
          <div>
            <label className="block text-sm font-medium text-brightwing-gray-300 mb-1">
              Server ID <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={newServerId}
              onChange={(e) => setNewServerId(e.target.value)}
              placeholder="e.g., github-mcp"
              className="w-full px-3 py-2 text-sm bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-md focus:outline-none focus:ring-2 focus:ring-brightwing-blue/50 placeholder-brightwing-gray-500 font-mono"
            />
            <p className="text-xs text-brightwing-gray-500 mt-1">
              Unique identifier used in configs and IPC. Use lowercase with hyphens.
            </p>
          </div>

          <div>
            <label className="block text-sm font-medium text-brightwing-gray-300 mb-1">
              Display Name <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={newDisplayName}
              onChange={(e) => setNewDisplayName(e.target.value)}
              placeholder="e.g., GitHub MCP Server"
              className="w-full px-3 py-2 text-sm bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-md focus:outline-none focus:ring-2 focus:ring-brightwing-blue/50 placeholder-brightwing-gray-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-brightwing-gray-300 mb-1">
              Auth Type
            </label>
            <select
              value={newAuthType}
              onChange={(e) => setNewAuthType(e.target.value)}
              className="w-full px-3 py-2 text-sm bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-md focus:outline-none focus:ring-2 focus:ring-brightwing-blue/50"
            >
              <option value="api_key">API Key</option>
              <option value="oauth">OAuth</option>
              <option value="none">None</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-brightwing-gray-300 mb-1">
              Upstream URL
            </label>
            <input
              type="text"
              value={newUpstreamUrl}
              onChange={(e) => setNewUpstreamUrl(e.target.value)}
              placeholder="https://mcp-server.example.com"
              className="w-full px-3 py-2 text-sm bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-md focus:outline-none focus:ring-2 focus:ring-brightwing-blue/50 placeholder-brightwing-gray-500 font-mono"
            />
            <p className="text-xs text-brightwing-gray-500 mt-1">
              For HTTP/SSE upstream servers. Leave blank for stdio servers.
            </p>
          </div>

          <button
            onClick={handleAddServer}
            disabled={addSaving || !newServerId.trim() || !newDisplayName.trim()}
            className="px-4 py-2 text-sm font-medium bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors disabled:opacity-50"
          >
            {addSaving ? "Registering..." : "Register Server"}
          </button>
        </div>
      </div>
    );
  }

  // List view
  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Proxy Servers</h1>
        <button
          onClick={() => setSubView("add")}
          className="px-3 py-1.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors"
        >
          + Register Server
        </button>
      </div>

      <div className="mb-6">
        <DaemonStatus />
      </div>

      {proxyServersLoading ? (
        <p className="text-brightwing-gray-500 text-sm">Loading...</p>
      ) : proxyServers.length === 0 ? (
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
          <p className="text-brightwing-gray-400">No proxy servers registered.</p>
          <p className="text-brightwing-gray-500 text-sm mt-1">
            Register a server to manage its auth and install it as a proxy across your tools.
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {proxyServers.map((server) => {
            const isExpanded = expandedServer === server.server_id;
            const installs = proxyInstalls[server.server_id] || [];

            return (
              <div
                key={server.server_id}
                className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg overflow-hidden"
              >
                {/* Server header */}
                <div
                  className="flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-brightwing-gray-700/30 transition-colors"
                  onClick={() => setExpandedServer(isExpanded ? null : server.server_id)}
                >
                  <svg
                    className={`w-4 h-4 text-brightwing-gray-400 transition-transform ${isExpanded ? "rotate-90" : ""}`}
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5" />
                  </svg>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{server.display_name}</span>
                      <span className="text-xs font-mono text-brightwing-gray-500">{server.server_id}</span>
                    </div>
                  </div>
                  <span
                    className={`text-xs px-2 py-0.5 rounded-full ${
                      server.auth_type === "oauth"
                        ? "bg-purple-500/20 text-purple-300"
                        : server.auth_type === "api_key"
                        ? "bg-amber-500/20 text-amber-300"
                        : "bg-brightwing-gray-700 text-brightwing-gray-400"
                    }`}
                  >
                    {server.auth_type === "api_key" ? "API Key" : server.auth_type === "oauth" ? "OAuth" : "None"}
                  </span>
                </div>

                {/* Expanded details */}
                {isExpanded && (
                  <div className="border-t border-brightwing-gray-700 px-4 py-3 space-y-3">
                    {server.upstream_url && (
                      <div className="text-xs">
                        <span className="text-brightwing-gray-500">Upstream: </span>
                        <span className="font-mono text-brightwing-gray-300">{server.upstream_url}</span>
                      </div>
                    )}

                    {/* OAuth connect for OAuth-type servers */}
                    {server.auth_type === "oauth" && (
                      <div>
                        <h4 className="text-xs font-medium text-brightwing-gray-400 uppercase tracking-wider mb-2">
                          Authentication
                        </h4>
                        <OAuthConnect server={server} />
                      </div>
                    )}

                    {/* Tool installs */}
                    <div>
                      <h4 className="text-xs font-medium text-brightwing-gray-400 uppercase tracking-wider mb-2">
                        Installed In
                      </h4>
                      <div className="flex flex-wrap gap-2">
                        {detectedTools.map((tool) => {
                          const isInstalled = installs.includes(tool.id);
                          return (
                            <button
                              key={tool.id}
                              onClick={() =>
                                isInstalled
                                  ? handleUninstallFromTool(server.server_id, tool.id, server.server_id)
                                  : handleInstallToTool(server.server_id, tool.id, server.server_id)
                              }
                              className={`px-3 py-1.5 text-xs rounded-md border transition-colors ${
                                isInstalled
                                  ? "bg-green-500/10 border-green-500/30 text-green-300 hover:bg-green-500/20"
                                  : "bg-brightwing-gray-700/50 border-brightwing-gray-600 text-brightwing-gray-400 hover:bg-brightwing-gray-700"
                              }`}
                            >
                              {tool.short_name}
                              {isInstalled && " \u2713"}
                            </button>
                          );
                        })}
                      </div>
                    </div>

                    {/* Actions */}
                    <div className="flex gap-2 pt-1">
                      <button
                        onClick={() => {
                          setSelectedServer(server);
                          setSubView("filter");
                        }}
                        className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors"
                      >
                        Tool Filter
                      </button>
                      <button
                        onClick={() => handleDelete(server.server_id)}
                        className="px-3 py-1.5 text-xs text-red-400 hover:bg-red-500/10 rounded-md transition-colors"
                      >
                        Remove
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
