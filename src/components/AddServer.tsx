import { useState } from "react";
import { useStore } from "../store";
import {
  installServer,
  registerProxyServer,
  storeApiKey,
  installProxyToTool,
  daemonStatus,
  startDaemon,
} from "../lib/tauri";
import type { ServerInstallConfig } from "../lib/types";

export default function AddServer() {
  const { tools, refreshConfiguredServers, refreshDisabledServers, refreshProxyServers, showToast, addPendingRestart } =
    useStore();

  const [configKey, setConfigKey] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [url, setUrl] = useState("");
  const [transport, setTransport] = useState<"stdio" | "http">("stdio");
  const [envRows, setEnvRows] = useState<{ key: string; value: string }[]>([]);
  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState(false);
  const [proxyMode, setProxyMode] = useState(false);

  // Filter out tools that don't support programmatic config updates
  const UNSUPPORTED_TOOL_IDS = new Set(["claude_desktop"]);
  const detectedTools = tools.filter((t) => t.detected && !UNSUPPORTED_TOOL_IDS.has(t.id));

  const addEnvRow = () => {
    setEnvRows([...envRows, { key: "", value: "" }]);
  };

  const updateEnvRow = (
    index: number,
    field: "key" | "value",
    value: string
  ) => {
    const updated = [...envRows];
    updated[index][field] = value;
    setEnvRows(updated);
  };

  const removeEnvRow = (index: number) => {
    setEnvRows(envRows.filter((_, i) => i !== index));
  };

  const toggleTool = (toolId: string) => {
    const next = new Set(selectedTools);
    if (next.has(toolId)) {
      next.delete(toolId);
    } else {
      next.add(toolId);
    }
    setSelectedTools(next);
  };

  const isStdio = transport === "stdio";
  const hasEnvVars = envRows.some((r) => r.key.trim() && r.value.trim());
  const canInstall = configKey.trim() &&
    selectedTools.size > 0 &&
    (isStdio ? command.trim() : url.trim());

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

  const handleInstall = async () => {
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
    if (selectedTools.size === 0) {
      showToast("Select at least one tool to install into", "error");
      return;
    }

    const env: Record<string, string> = {};
    for (const row of envRows) {
      if (row.key.trim() && row.value.trim()) {
        env[row.key.trim()] = row.value.trim();
      }
    }

    setInstalling(true);

    if (proxyMode) {
      await handleProxyInstall(env);
    } else {
      await handleDirectInstall(env);
    }

    setInstalling(false);
  };

  const handleProxyInstall = async (env: Record<string, string>) => {
    try {
      const daemonOk = await ensureDaemon();
      if (!daemonOk) return;

      const serverId = configKey.trim();
      const authType = Object.keys(env).length > 0 ? "api_key" : "none";

      await registerProxyServer({
        serverId,
        displayName: serverId,
        authType,
        upstreamUrl: !isStdio ? url.trim() : undefined,
        upstreamCommand: isStdio ? command.trim() : undefined,
        upstreamArgs: isStdio ? args.trim() || undefined : undefined,
      });

      if (Object.keys(env).length > 0) {
        await storeApiKey(serverId, env);
      }

      let successCount = 0;
      let lastError = "";

      for (const toolId of selectedTools) {
        try {
          const result = await installProxyToTool(toolId, serverId, serverId);
          if (result.success) {
            successCount++;
            if (result.needs_restart) addPendingRestart(toolId);
          } else {
            lastError = result.message;
          }
        } catch (e) {
          lastError = String(e);
        }
      }

      if (successCount === selectedTools.size) {
        showToast(
          `Installed ${serverId} into ${successCount} tool${successCount > 1 ? "s" : ""} (via proxy)`,
          "success"
        );
        resetForm();
        refreshConfiguredServers();
        refreshDisabledServers();
        refreshProxyServers();
      } else if (successCount > 0) {
        showToast(
          `Installed into ${successCount}/${selectedTools.size} tools. Last error: ${lastError}`,
          "error"
        );
        refreshConfiguredServers();
        refreshProxyServers();
      } else {
        showToast(`Proxy install failed: ${lastError}`, "error");
      }
    } catch (e) {
      showToast(`Proxy setup failed: ${e}`, "error");
    }
  };

  const handleDirectInstall = async (env: Record<string, string>) => {
    const parsedArgs = args
      .trim()
      .split(/\s+/)
      .filter((a) => a);

    const serverConfig: ServerInstallConfig = {
      server_name: configKey.trim(),
      config_key: configKey.trim(),
      command: isStdio ? command.trim() : "",
      args: isStdio ? parsedArgs : [],
      url: isStdio ? "" : url.trim(),
      env,
      transport,
    };

    let successCount = 0;
    let lastError = "";

    for (const toolId of selectedTools) {
      try {
        const result = await installServer(toolId, serverConfig);
        if (result.success) {
          successCount++;
          if (result.needs_restart) addPendingRestart(toolId);
        } else {
          lastError = result.message;
        }
      } catch (e) {
        lastError = String(e);
      }
    }

    if (successCount === selectedTools.size) {
      showToast(
        `Installed ${configKey.trim()} into ${successCount} tool${successCount > 1 ? "s" : ""}`,
        "success"
      );
      resetForm();
      refreshConfiguredServers();
      refreshDisabledServers();
    } else if (successCount > 0) {
      showToast(
        `Installed into ${successCount}/${selectedTools.size} tools. Last error: ${lastError}`,
        "error"
      );
      refreshConfiguredServers();
    } else {
      showToast(`Install failed: ${lastError}`, "error");
    }
  };

  const resetForm = () => {
    setConfigKey("");
    setCommand("");
    setArgs("");
    setUrl("");
    setEnvRows([]);
    setSelectedTools(new Set());
    setProxyMode(false);
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
              onClick={() => setTransport("stdio")}
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
          /* HTTP field */
          <div>
            <label className="block text-xs text-brightwing-gray-400 mb-1">
              Server URL
              <span className="text-red-400 ml-1">*</span>
            </label>
            <input
              type="text"
              placeholder="https://mcp.example.com/sse"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full px-3 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
            />
            <p className="text-xs text-brightwing-gray-600 mt-1">
              The MCP server endpoint URL
            </p>
          </div>
        )}

        {/* Environment Variables */}
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
                    <svg
                      className="w-4 h-4"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth={2}
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M6 18 18 6M6 6l12 12"
                      />
                    </svg>
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Proxy mode toggle — shown when env vars are present */}
        {hasEnvVars && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium">
                  {proxyMode ? "Proxy Install" : "Direct Install"}
                </p>
                <p className="text-xs text-brightwing-gray-500 mt-0.5">
                  {proxyMode
                    ? "Credentials stored securely. Tools get a local proxy — no plaintext secrets in config files."
                    : "Environment variables written directly into each tool's config file."}
                </p>
              </div>
              <button
                onClick={() => setProxyMode(!proxyMode)}
                disabled={installing}
                className={`relative w-11 h-6 rounded-full transition-colors ${
                  proxyMode ? "bg-brightwing-blue" : "bg-brightwing-gray-600"
                } disabled:opacity-50`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform ${
                    proxyMode ? "translate-x-5" : ""
                  }`}
                />
              </button>
            </div>
          </div>
        )}

        {/* Tool selection */}
        <div>
          <label className="block text-xs text-brightwing-gray-400 uppercase tracking-wider mb-2">
            Install Into
            <span className="text-red-400 ml-1">*</span>
          </label>
          <div className="space-y-2">
            {detectedTools.map((tool) => (
              <button
                key={tool.id}
                onClick={() => toggleTool(tool.id)}
                className={`w-full flex items-center gap-3 p-3 rounded-lg border transition-colors text-left ${
                  selectedTools.has(tool.id)
                    ? "bg-brightwing-blue/10 border-brightwing-blue"
                    : "bg-brightwing-gray-800 border-brightwing-gray-700 hover:border-brightwing-gray-600"
                }`}
              >
                <div
                  className={`w-4 h-4 rounded border-2 flex items-center justify-center ${
                    selectedTools.has(tool.id)
                      ? "border-brightwing-blue bg-brightwing-blue"
                      : "border-brightwing-gray-600"
                  }`}
                >
                  {selectedTools.has(tool.id) && (
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
              </button>
            ))}
            {detectedTools.length === 0 && (
              <p className="text-brightwing-gray-500 text-sm">
                No AI tools detected on your machine.
              </p>
            )}
          </div>
        </div>

        {/* Config preview */}
        {(isStdio ? command.trim() : url.trim()) && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <p className="text-xs text-brightwing-gray-500 mb-1">
              {proxyMode ? "Proxy Command (written to tool configs)" : "Config Preview"}
            </p>
            <code className="text-sm text-brightwing-gray-200 font-mono break-all">
              {proxyMode
                ? `brightwing-proxy --server ${configKey.trim() || "my-server"}`
                : isStdio
                  ? `${command.trim()} ${args.trim()}`
                  : url.trim()}
            </code>
          </div>
        )}

        {/* Install button */}
        <button
          onClick={handleInstall}
          disabled={installing || !canInstall}
          className="w-full py-2.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {installing
            ? proxyMode
              ? "Setting up proxy..."
              : "Installing..."
            : proxyMode
              ? `Install into ${selectedTools.size} Tool${selectedTools.size !== 1 ? "s" : ""} (via Proxy)`
              : `Install into ${selectedTools.size} Tool${selectedTools.size !== 1 ? "s" : ""}`}
        </button>

        <p className="text-xs text-brightwing-gray-500">
          {proxyMode
            ? "Credentials are stored in Brightwing's secure vault. Tool configs will reference the local proxy."
            : "Changes will appear in the restart banner above when tools need restarting."}
        </p>
      </div>
    </div>
  );
}
