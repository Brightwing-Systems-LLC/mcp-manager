import { useState, useEffect } from "react";
import { useStore } from "../store";
import { installServer, uninstallServer, apiGetServer } from "../lib/tauri";
import type { ServerInstallConfig, ScoreboardServer } from "../lib/types";

export default function InstallDialog() {
  const {
    installTarget,
    installConfig,
    installConfigLoading,
    pendingDeepLink,
    tools,
    installations,
    setView,
    setInstallTarget,
    setPendingDeepLink,
    refreshInstallations,
    showToast,
    addPendingRestart,
  } = useStore();

  const [installing, setInstalling] = useState<string | null>(null);
  const [envValues, setEnvValues] = useState<Record<string, string>>({});
  const [deepLinkLoading, setDeepLinkLoading] = useState(false);
  const [deepLinkError, setDeepLinkError] = useState<string | null>(null);

  const server = installTarget;
  const deepLink = pendingDeepLink;

  // When we have a deep link but no server, fetch the server details
  useEffect(() => {
    if (deepLink && !server && !deepLinkLoading) {
      setDeepLinkLoading(true);
      setDeepLinkError(null);
      apiGetServer(deepLink.server_uuid)
        .then((serverData: ScoreboardServer) => {
          setInstallTarget(serverData);
          setPendingDeepLink(null);
        })
        .catch((e) => {
          setDeepLinkError(`Failed to fetch server: ${e}`);
        })
        .finally(() => {
          setDeepLinkLoading(false);
        });
    }
  }, [deepLink, server, deepLinkLoading, setInstallTarget, setPendingDeepLink]);

  if (!server && !deepLink) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <p className="text-brightwing-gray-400">
          No server selected for installation.
        </p>
        <button
          onClick={() => setView("search")}
          className="mt-3 px-4 py-2 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md"
        >
          Search Servers
        </button>
      </div>
    );
  }

  if (deepLink && !server) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center max-w-md">
          {deepLinkError ? (
            <>
              <h2 className="text-lg font-semibold mb-2 text-red-400">Error</h2>
              <p className="text-brightwing-gray-400 text-sm">{deepLinkError}</p>
            </>
          ) : (
            <>
              <h2 className="text-lg font-semibold mb-2">Loading Server</h2>
              <p className="text-brightwing-gray-500 text-sm">
                Fetching server details from MCP Scoreboard...
              </p>
            </>
          )}
          <button
            onClick={() => {
              setPendingDeepLink(null);
              setDeepLinkError(null);
              setView("dashboard");
            }}
            className="mt-4 px-4 py-2 text-sm bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md"
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }

  if (!server) return null;

  const detectedTools = tools.filter((t) => t.detected);
  const installedToolIds = new Set(
    installations
      .filter((i) => i.server_name === server.name || i.server_uuid === String(server.id))
      .map((i) => i.tool_id)
  );

  const config = installConfig;
  const hasConfig = config !== null;

  const handleEnvChange = (key: string, value: string) => {
    setEnvValues((prev) => ({ ...prev, [key]: value }));
  };

  const handleInstall = async (toolId: string) => {
    if (!config) return;

    // Check required env vars
    const missingRequired = Object.entries(config.env_schema)
      .filter(([, schema]) => schema.required)
      .filter(([key]) => !envValues[key]?.trim());

    if (missingRequired.length > 0) {
      showToast(
        `Missing required: ${missingRequired.map(([k]) => k).join(", ")}`,
        "error"
      );
      return;
    }

    setInstalling(toolId);
    try {
      const env: Record<string, string> = {};
      for (const [key, schema] of Object.entries(config.env_schema)) {
        const value = envValues[key]?.trim();
        if (value) {
          env[key] = value;
        } else if (schema.default) {
          env[key] = schema.default;
        }
      }

      const installConfig: ServerInstallConfig = {
        server_name: server.name,
        config_key: config.config_key,
        command: config.command,
        args: config.args,
        env,
        transport: config.transport,
      };

      const result = await installServer(toolId, installConfig);
      if (result.success) {
        showToast(result.message, "success");
        if (result.needs_restart) addPendingRestart(toolId);
        await refreshInstallations();
      } else {
        showToast(result.message, "error");
      }
    } catch (e) {
      showToast(`Install failed: ${e}`, "error");
    } finally {
      setInstalling(null);
    }
  };

  const handleUninstall = async (toolId: string) => {
    if (!config) return;
    setInstalling(toolId);
    try {
      const result = await uninstallServer(
        toolId,
        config.config_key,
        String(server.id)
      );
      if (result.success) {
        showToast(result.message, "success");
        if (result.needs_restart) addPendingRestart(toolId);
        await refreshInstallations();
      } else {
        showToast(result.message, "error");
      }
    } catch (e) {
      showToast(`Uninstall failed: ${e}`, "error");
    } finally {
      setInstalling(null);
    }
  };

  const handleBack = () => {
    setInstallTarget(null);
    setPendingDeepLink(null);
    setView("search");
  };

  return (
    <div>
      <button
        onClick={handleBack}
        className="flex items-center gap-1 text-sm text-brightwing-gray-400 hover:text-brightwing-gray-200 mb-4"
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
        </svg>
        Back
      </button>

      {/* Server info */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-6 mb-6">
        <div className="flex items-start gap-4">
          <div className="flex-1">
            <h1 className="text-xl font-mono font-semibold">{server.name}</h1>
            <p className="text-sm text-brightwing-gray-400 mt-1">
              {server.description}
            </p>
            <div className="flex gap-3 mt-2 text-xs text-brightwing-gray-500">
              {server.current_grade && (
                <span className="font-semibold text-green-400">
                  {server.current_grade}
                  {server.current_score != null && ` (${server.current_score})`}
                </span>
              )}
              {server.language && <span>{server.language}</span>}
              {server.stars_count > 0 && <span>{server.stars_count} stars</span>}
            </div>
          </div>
          {config?.verified && (
            <span className="px-2 py-1 text-xs bg-green-500/10 text-green-400 rounded-md">
              Verified
            </span>
          )}
        </div>
        {config?.install_notes && (
          <p className="text-xs text-brightwing-gray-500 mt-3 border-t border-brightwing-gray-700 pt-3">
            {config.install_notes}
          </p>
        )}
      </div>

      {/* Install config status */}
      {installConfigLoading ? (
        <p className="text-brightwing-gray-500 text-sm mb-6">
          Fetching install configuration...
        </p>
      ) : !hasConfig ? (
        <div className="bg-brightwing-gray-800 border border-yellow-500/30 rounded-lg p-5 mb-6">
          <p className="text-yellow-400 text-sm font-medium mb-1">
            No install configuration available
          </p>
          <p className="text-brightwing-gray-500 text-xs">
            This server doesn't have a verified install config on MCP Scoreboard
            yet. You can install it manually by editing your tool's config file
            directly.
          </p>
        </div>
      ) : (
        <>
          {/* Command preview */}
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4 mb-4">
            <p className="text-xs text-brightwing-gray-500 mb-1">Command</p>
            <code className="text-sm text-brightwing-gray-200 font-mono">
              {config.command} {config.args.join(" ")}
            </code>
          </div>

          {/* Env var form */}
          {Object.keys(config.env_schema).length > 0 && (
            <div className="mb-6">
              <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
                Configuration
              </h2>
              <div className="space-y-3">
                {Object.entries(config.env_schema).map(([key, schema]) => (
                  <div key={key}>
                    <label className="block text-xs text-brightwing-gray-400 mb-1">
                      {key}
                      {schema.required && (
                        <span className="text-red-400 ml-1">*</span>
                      )}
                      {schema.description && (
                        <span className="text-brightwing-gray-600 ml-2">
                          — {schema.description}
                        </span>
                      )}
                    </label>
                    <input
                      type={schema.sensitive ? "password" : "text"}
                      placeholder={schema.default || ""}
                      value={envValues[key] || ""}
                      onChange={(e) => handleEnvChange(key, e.target.value)}
                      className="w-full px-3 py-2 bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-md text-sm font-mono placeholder-brightwing-gray-600 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
                    />
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Tool installation grid */}
          <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
            Install Into
          </h2>

          <div className="space-y-2">
            {detectedTools.map((tool) => {
              const isInstalled = installedToolIds.has(tool.id);
              const isLoading = installing === tool.id;

              return (
                <div
                  key={tool.id}
                  className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4 flex items-center justify-between"
                >
                  <div className="flex items-center gap-3">
                    <div
                      className={`w-2 h-2 rounded-full ${
                        isInstalled ? "bg-green-400" : "bg-brightwing-gray-600"
                      }`}
                    />
                    <div>
                      <span className="text-sm font-medium">
                        {tool.display_name}
                      </span>
                      <span className="text-xs text-brightwing-gray-500 ml-2">
                        ({tool.short_name})
                      </span>
                    </div>
                  </div>

                  <div>
                    {isInstalled ? (
                      <button
                        onClick={() => handleUninstall(tool.id)}
                        disabled={isLoading}
                        className="px-3 py-1.5 text-sm bg-red-500/10 text-red-400 hover:bg-red-500/20 rounded-md transition-colors disabled:opacity-50"
                      >
                        {isLoading ? "Removing..." : "Remove"}
                      </button>
                    ) : (
                      <button
                        onClick={() => handleInstall(tool.id)}
                        disabled={isLoading}
                        className="px-3 py-1.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50"
                      >
                        {isLoading ? "Installing..." : "Install"}
                      </button>
                    )}
                  </div>
                </div>
              );
            })}

            {detectedTools.length === 0 && (
              <p className="text-brightwing-gray-500 text-sm">
                No AI tools detected on your machine.
              </p>
            )}
          </div>
        </>
      )}

      <p className="text-xs text-brightwing-gray-500 mt-4">
        Changes will appear in the restart banner above when tools need
        restarting.
      </p>
    </div>
  );
}
