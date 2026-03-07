import { useState } from "react";
import { useStore } from "../store";
import { installServer, uninstallServer } from "../lib/tauri";
import type { ServerInstallConfig } from "../lib/types";

export default function InstallDialog() {
  const {
    installTarget,
    pendingDeepLink,
    tools,
    installations,
    setView,
    setInstallTarget,
    setPendingDeepLink,
    refreshInstallations,
    showToast,
  } = useStore();

  const [installing, setInstalling] = useState<string | null>(null);

  // Determine which server we're looking at
  const server = installTarget;
  const deepLink = pendingDeepLink;

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

  // If deep link but no server details yet, show placeholder
  if (deepLink && !server) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center max-w-md">
          <h2 className="text-lg font-semibold mb-2">Deep Link Received</h2>
          <p className="text-brightwing-gray-400 text-sm">
            Action: <span className="font-mono">{deepLink.action}</span>
          </p>
          <p className="text-brightwing-gray-400 text-sm">
            Server: <span className="font-mono">{deepLink.server_uuid}</span>
          </p>
          <p className="text-brightwing-gray-500 text-xs mt-4">
            Fetching server details from PatchworkMCP...
          </p>
          <button
            onClick={() => {
              setPendingDeepLink(null);
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

  // Check which tools already have this server installed
  const installedToolIds = new Set(
    installations
      .filter((i) => i.server_name === server.name)
      .map((i) => i.tool_id)
  );

  const handleInstall = async (toolId: string) => {
    setInstalling(toolId);
    try {
      // Build a basic config from the server info
      // In a full version, this comes from the PatchworkMCP API install-config endpoint
      const config: ServerInstallConfig = {
        server_name: server.name,
        config_key: server.name,
        command: "npx",
        args: ["-y", server.name],
        env: {},
        transport: "stdio",
      };

      const result = await installServer(toolId, config);
      if (result.success) {
        showToast(result.message, "success");
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
    setInstalling(toolId);
    try {
      const result = await uninstallServer(toolId, server.name, server.uuid);
      if (result.success) {
        showToast(result.message, "success");
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
      {/* Back button */}
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
          <div>
            <h1 className="text-xl font-mono font-semibold">{server.name}</h1>
            <p className="text-sm text-brightwing-gray-400 mt-1">
              {server.description}
            </p>
            <div className="flex gap-3 mt-2 text-xs text-brightwing-gray-500">
              {server.grade && (
                <span className="font-semibold text-green-400">
                  {server.grade} ({server.overall_score})
                </span>
              )}
              {server.language && <span>{server.language}</span>}
              {server.stars > 0 && <span>{server.stars} stars</span>}
            </div>
          </div>
        </div>
      </div>

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

      <p className="text-xs text-brightwing-gray-500 mt-4">
        Most tools need a restart after config changes to activate new MCP
        servers.
      </p>
    </div>
  );
}
