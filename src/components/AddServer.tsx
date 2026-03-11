import { useState } from "react";
import { useStore } from "../store";
import {
  registerProxyServer,
  storeApiKey,
  daemonStatus,
  startDaemon,
  getProxyServer,
  probeServerAuth,
} from "../lib/tauri";
import type { AuthProbeResult, ProxyServer } from "../lib/types";
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

export default function AddServer() {
  const { refreshProxyServers, showToast, setView, setServerDetailId } =
    useStore();

  const [configKey, setConfigKey] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [url, setUrl] = useState("");
  const [transport, setTransport] = useState<"stdio" | "http">("stdio");
  const [envRows, setEnvRows] = useState<{ key: string; value: string }[]>([]);
  const [saving, setSaving] = useState(false);

  // Auto-probe state for HTTP servers
  const [probeResult, setProbeResult] = useState<AuthProbeResult | null>(null);
  const [probing, setProbing] = useState(false);
  const [proxyServer, setProxyServer] = useState<ProxyServer | null>(null);
  const [oauthConnected, setOauthConnected] = useState(false);

  const isHttp = transport === "http";
  const isStdio = transport === "stdio";

  // For HTTP+OAuth, block save until connected
  const needsOauthFirst = isHttp && probeResult?.auth_type === "oauth" && !oauthConnected;

  const canSave = configKey.trim() &&
    (isStdio ? command.trim() : url.trim()) &&
    !needsOauthFirst;

  const addEnvRow = () => {
    setEnvRows([...envRows, { key: "", value: "" }]);
  };

  const updateEnvRow = (index: number, field: "key" | "value", value: string) => {
    const updated = [...envRows];
    updated[index][field] = value;
    setEnvRows(updated);
  };

  const removeEnvRow = (index: number) => {
    setEnvRows(envRows.filter((_, i) => i !== index));
  };

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

  const handleProbe = async () => {
    if (!url.trim() || !configKey.trim()) {
      showToast("Server name and URL are required", "error");
      return;
    }

    setProbing(true);
    setProbeResult(null);
    setProxyServer(null);
    setOauthConnected(false);

    try {
      const result = await probeServerAuth(url.trim());
      setProbeResult(result);

      const daemonOk = await ensureDaemon();
      if (!daemonOk) {
        setProbing(false);
        return;
      }

      const serverId = configKey.trim();
      const authType = result.auth_type === "oauth" ? "oauth"
        : result.auth_type === "api_key" ? "api_key"
        : "none";

      await registerProxyServer({
        serverId,
        displayName: serverId,
        authType,
        upstreamUrl: url.trim(),
      });

      const ps = await getProxyServer(serverId);
      if (ps) setProxyServer(ps);
    } catch (e) {
      setProbeResult({
        auth_type: "unknown",
        server_reachable: false,
        error_message: String(e),
        has_oauth_metadata: false,
      });
    }
    setProbing(false);
  };

  const handleSave = async () => {
    if (!configKey.trim()) {
      showToast("Server name is required", "error");
      return;
    }
    if (isStdio && !command.trim()) {
      showToast("Command is required for stdio transport", "error");
      return;
    }
    if (!isStdio && !url.trim()) {
      showToast("URL is required for HTTP transport", "error");
      return;
    }

    setSaving(true);

    try {
      const daemonOk = await ensureDaemon();
      if (!daemonOk) {
        setSaving(false);
        return;
      }

      const serverId = configKey.trim();

      // For HTTP servers with auto-probe, proxy is already registered
      if (!isHttp || !proxyServer) {
        const env: Record<string, string> = {};
        for (const row of envRows) {
          if (row.key.trim() && row.value.trim()) {
            env[row.key.trim()] = row.value.trim();
          }
        }
        const authType = Object.keys(env).length > 0 ? "api_key" : "none";
        await registerProxyServer({
          serverId,
          displayName: serverId,
          authType,
          upstreamUrl: !isStdio ? url.trim() : undefined,
          upstreamCommand: isStdio ? command.trim() : undefined,
          upstreamArgs: isStdio ? args.trim() || undefined : undefined,
        });
      }

      // Store credentials if any
      const env: Record<string, string> = {};
      for (const row of envRows) {
        if (row.key.trim() && row.value.trim()) {
          env[row.key.trim()] = row.value.trim();
        }
      }
      if (Object.keys(env).length > 0 && probeResult?.auth_type !== "oauth") {
        await storeApiKey(serverId, env);
      }

      await refreshProxyServers();
      showToast(`Added ${serverId}`, "success");

      // Navigate to server detail
      setServerDetailId(normalizeServerName(serverId));
      setView("server-detail");
    } catch (e) {
      showToast(`Failed to add server: ${e}`, "error");
    }

    setSaving(false);
  };

  return (
    <div>
      <h1 className="text-2xl font-semibold mb-6">Add Server Manually</h1>

      <div className="space-y-5 max-w-xl">
        {/* Server name */}
        <div>
          <label className="block text-xs text-brightwing-gray-400 mb-1">
            Server Name / Config Key
            <span className="text-red-400 ml-1">*</span>
          </label>
          <input
            type="text"
            placeholder="my-mcp-server"
            value={configKey}
            onChange={(e) => setConfigKey(e.target.value)}
            className="w-full px-3 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
          />
          <p className="text-xs text-brightwing-gray-600 mt-1">
            The key used in the config file (e.g. "my-server")
          </p>
        </div>

        {/* Transport */}
        <div>
          <label className="block text-xs text-brightwing-gray-400 mb-1">
            Transport
          </label>
          <div className="flex gap-3">
            <button
              onClick={() => {
                setTransport("stdio");
                setProbeResult(null);
                setProxyServer(null);
              }}
              className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
                transport === "stdio"
                  ? "bg-brightwing-blue text-white"
                  : "bg-brightwing-gray-800 border border-brightwing-gray-700 text-brightwing-gray-400 hover:border-brightwing-gray-600"
              }`}
            >
              stdio
            </button>
            <button
              onClick={() => setTransport("http")}
              className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
                transport === "http"
                  ? "bg-brightwing-blue text-white"
                  : "bg-brightwing-gray-800 border border-brightwing-gray-700 text-brightwing-gray-400 hover:border-brightwing-gray-600"
              }`}
            >
              http
            </button>
          </div>
        </div>

        {/* stdio fields */}
        {isStdio ? (
          <>
            <div>
              <label className="block text-xs text-brightwing-gray-400 mb-1">
                Command
                <span className="text-red-400 ml-1">*</span>
              </label>
              <input
                type="text"
                placeholder="npx"
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                className="w-full px-3 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
              />
            </div>
            <div>
              <label className="block text-xs text-brightwing-gray-400 mb-1">
                Arguments
              </label>
              <input
                type="text"
                placeholder="-y @my-org/my-mcp-server"
                value={args}
                onChange={(e) => setArgs(e.target.value)}
                className="w-full px-3 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
              />
              <p className="text-xs text-brightwing-gray-600 mt-1">
                Space-separated arguments
              </p>
            </div>
          </>
        ) : (
          <div>
            <label className="block text-xs text-brightwing-gray-400 mb-1">
              Server URL
              <span className="text-red-400 ml-1">*</span>
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                placeholder="https://mcp.example.com/sse"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                className="flex-1 px-3 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
              />
              <button
                onClick={handleProbe}
                disabled={probing || !url.trim() || !configKey.trim()}
                className="px-3 py-2 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50 whitespace-nowrap"
              >
                {probing ? "Checking..." : "Check Server"}
              </button>
            </div>
          </div>
        )}

        {/* Probe result for HTTP servers */}
        {isHttp && probeResult && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
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
          </div>
        )}

        {/* Inline OAuth connect for HTTP+OAuth servers */}
        {isHttp && probeResult?.auth_type === "oauth" && proxyServer && (
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
        {isHttp && probeResult && !probeResult.server_reachable && (
          <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4">
            <p className="text-sm text-red-400 font-medium mb-1">Server Unreachable</p>
            <p className="text-xs text-brightwing-gray-400">
              Could not connect to the server. You can still add it and configure auth later.
            </p>
          </div>
        )}

        {/* Environment Variables — not shown for HTTP+OAuth */}
        {!(isHttp && probeResult?.auth_type === "oauth") && (
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs text-brightwing-gray-400">
                Environment Variables
              </label>
              <button
                onClick={addEnvRow}
                className="text-xs text-brightwing-blue hover:text-brightwing-blue-dark"
              >
                + Add Variable
              </button>
            </div>
            {envRows.length > 0 && (
              <div className="space-y-2">
                {envRows.map((row, i) => (
                  <div key={i} className="flex gap-2 items-center">
                    <input
                      type="text"
                      placeholder="KEY"
                      value={row.key}
                      onChange={(e) => updateEnvRow(i, "key", e.target.value)}
                      className="flex-1 px-3 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
                    />
                    <input
                      type="text"
                      placeholder="value"
                      value={row.value}
                      onChange={(e) => updateEnvRow(i, "value", e.target.value)}
                      className="flex-1 px-3 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
                    />
                    <button
                      onClick={() => removeEnvRow(i)}
                      className="p-2 text-brightwing-gray-500 hover:text-red-400"
                    >
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Save button */}
        <button
          onClick={handleSave}
          disabled={saving || !canSave}
          className="w-full py-2.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {saving
            ? "Adding server..."
            : needsOauthFirst
              ? "Connect OAuth First"
              : "Add Server"}
        </button>

        <p className="text-xs text-brightwing-gray-500">
          Registers the server with Brightwing's proxy. Use the Dashboard to install it into your tools.
        </p>
      </div>
    </div>
  );
}
