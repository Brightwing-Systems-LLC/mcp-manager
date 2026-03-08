import { useState } from "react";
import { useStore } from "../store";
import { installServer } from "../lib/tauri";
import type { ServerInstallConfig } from "../lib/types";

export default function AddServer() {
  const { tools, refreshConfiguredServers, refreshDisabledServers, showToast, addPendingRestart } =
    useStore();

  const [configKey, setConfigKey] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [transport, setTransport] = useState<"stdio" | "http">("stdio");
  const [envRows, setEnvRows] = useState<{ key: string; value: string }[]>([]);
  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState(false);

  const detectedTools = tools.filter((t) => t.detected);

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

  const handleInstall = async () => {
    if (!configKey.trim()) {
      showToast("Server name is required", "error");
      return;
    }
    if (!command.trim()) {
      showToast("Command is required", "error");
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

    const parsedArgs = args
      .trim()
      .split(/\s+/)
      .filter((a) => a);

    const serverConfig: ServerInstallConfig = {
      server_name: configKey.trim(),
      config_key: configKey.trim(),
      command: command.trim(),
      args: parsedArgs,
      env,
      transport,
    };

    setInstalling(true);
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

    setInstalling(false);

    if (successCount === selectedTools.size) {
      showToast(
        `Installed ${configKey.trim()} into ${successCount} tool${successCount > 1 ? "s" : ""}`,
        "success"
      );
      // Reset form
      setConfigKey("");
      setCommand("");
      setArgs("");
      setEnvRows([]);
      setSelectedTools(new Set());
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

        {/* Command */}
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

        {/* Args */}
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

        {/* Command preview */}
        {command.trim() && (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <p className="text-xs text-brightwing-gray-500 mb-1">
              Config Preview
            </p>
            <code className="text-sm text-brightwing-gray-200 font-mono">
              {command.trim()} {args.trim()}
            </code>
          </div>
        )}

        {/* Install button */}
        <button
          onClick={handleInstall}
          disabled={installing || !configKey.trim() || !command.trim() || selectedTools.size === 0}
          className="w-full py-2.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {installing
            ? "Installing..."
            : `Install into ${selectedTools.size} Tool${selectedTools.size !== 1 ? "s" : ""}`}
        </button>

        <p className="text-xs text-brightwing-gray-500">
          Changes will appear in the restart banner above when tools need
          restarting.
        </p>
      </div>
    </div>
  );
}
