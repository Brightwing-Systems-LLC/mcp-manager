import { useEffect } from "react";
import { useStore } from "../store";
import DaemonStatus from "./DaemonStatus";
import MigrationAssistant from "./MigrationAssistant";

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

export default function ProxyServers() {
  const {
    proxyServers,
    proxyServersLoading,
    refreshProxyServers,
    setView,
    setServerDetailId,
  } = useStore();

  useEffect(() => {
    refreshProxyServers();
  }, [refreshProxyServers]);

  const handleServerClick = (serverId: string) => {
    setServerDetailId(normalizeServerName(serverId));
    setView("server-detail");
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Proxy Servers</h1>
      </div>

      <div className="mb-6">
        <DaemonStatus />
      </div>

      <MigrationAssistant />

      {proxyServersLoading ? (
        <p className="text-brightwing-gray-500 text-sm">Loading...</p>
      ) : proxyServers.length === 0 ? (
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
          <p className="text-brightwing-gray-400">No proxy servers registered.</p>
          <p className="text-brightwing-gray-500 text-sm mt-1">
            Install a server from Search to register it as a proxy across your tools.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {proxyServers.map((server) => (
            <button
              key={server.server_id}
              onClick={() => handleServerClick(server.server_id)}
              className="w-full bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg px-4 py-3 flex items-center gap-3 hover:border-brightwing-gray-600 transition-colors text-left"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-medium">{server.display_name}</span>
                  <span className="text-xs font-mono text-brightwing-gray-500">{server.server_id}</span>
                </div>
                {server.upstream_url && (
                  <p className="text-xs font-mono text-brightwing-gray-500 truncate mt-0.5">
                    {server.upstream_url}
                  </p>
                )}
              </div>
              <span
                className={`text-xs px-2 py-0.5 rounded-full shrink-0 ${
                  server.auth_type === "oauth"
                    ? "bg-purple-500/20 text-purple-300"
                    : server.auth_type === "api_key"
                    ? "bg-amber-500/20 text-amber-300"
                    : "bg-brightwing-gray-700 text-brightwing-gray-400"
                }`}
              >
                {server.auth_type === "api_key" ? "API Key" : server.auth_type === "oauth" ? "OAuth" : "None"}
              </span>
              <svg className="w-4 h-4 text-brightwing-gray-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5" />
              </svg>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
