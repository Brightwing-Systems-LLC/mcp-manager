import { useEffect, useState } from "react";
import { useStore } from "../store";

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
  } = useStore();

  const [togglingKey, setTogglingKey] = useState<string | null>(null);

  useEffect(() => {
    refreshConfiguredServers();
    refreshDisabledServers();
  }, [refreshConfiguredServers, refreshDisabledServers]);

  const detectedTools = tools.filter((t) => t.detected);
  const notDetected = tools.filter((t) => !t.detected);

  // Map tool_id -> short_name from tools list
  const toolShortNames = new Map(tools.map((t) => [t.id, t.short_name]));

  // Build a combined list: active servers + disabled servers
  // Group by server_name, collecting tool entries
  type ServerEntry = {
    toolId: string;
    shortName: string;
    configJson: string | null;
    isCliOnly: boolean;
    enabled: boolean;
  };

  const serverMap = new Map<string, ServerEntry[]>();

  // Active servers from config files
  for (const cs of configuredServers) {
    const entries = serverMap.get(cs.server_name) || [];
    entries.push({
      toolId: cs.tool_id,
      shortName: cs.tool_short_name,
      configJson: cs.config_json,
      isCliOnly: cs.is_cli_only,
      enabled: true,
    });
    serverMap.set(cs.server_name, entries);
  }

  // Disabled servers from DB
  for (const ds of disabledServers) {
    const entries = serverMap.get(ds.server_name) || [];
    entries.push({
      toolId: ds.tool_id,
      shortName: toolShortNames.get(ds.tool_id) || ds.tool_id,
      configJson: ds.config_json,
      isCliOnly: false,
      enabled: false,
    });
    serverMap.set(ds.server_name, entries);
  }

  const handleToggle = async (
    serverName: string,
    entry: ServerEntry
  ) => {
    const key = `${entry.toolId}:${serverName}`;
    setTogglingKey(key);
    try {
      if (entry.enabled) {
        if (!entry.configJson) return;
        await disableServer(entry.toolId, serverName, entry.configJson);
      } else {
        await enableServer(entry.toolId, serverName);
      }
    } finally {
      setTogglingKey(null);
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
          {detectedTools.map((tool) => (
            <div
              key={tool.id}
              className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-3"
            >
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-green-400" />
                <span className="text-sm font-medium">{tool.display_name}</span>
              </div>
              <p className="text-xs text-brightwing-gray-500 mt-1 font-mono">
                {tool.short_name}
              </p>
            </div>
          ))}
          {notDetected.map((tool) => (
            <div
              key={tool.id}
              className="bg-brightwing-gray-800/50 border border-brightwing-gray-700/50 rounded-lg p-3 opacity-50"
            >
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-brightwing-gray-600" />
                <span className="text-sm font-medium text-brightwing-gray-500">
                  {tool.display_name}
                </span>
              </div>
              <p className="text-xs text-brightwing-gray-600 mt-1">
                Not detected
              </p>
            </div>
          ))}
        </div>
      </section>

      {/* Configured Servers */}
      <section>
        <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
          MCP Servers ({serverMap.size})
        </h2>
        {configuredServersLoading ? (
          <p className="text-brightwing-gray-500 text-sm">Scanning config files...</p>
        ) : serverMap.size === 0 ? (
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
            <p className="text-brightwing-gray-400">
              No MCP servers configured in any tool.
            </p>
            <p className="text-brightwing-gray-500 text-sm mt-1">
              Use Search to find and install MCP servers.
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {Array.from(serverMap.entries()).map(([name, entries]) => (
              <div
                key={name}
                className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4"
              >
                <h3 className="font-mono font-medium text-sm mb-2">{name}</h3>
                <div className="space-y-1.5">
                  {entries.map((entry) => {
                    const key = `${entry.toolId}:${name}`;
                    const isToggling = togglingKey === key;
                    const canToggle = entry.configJson != null;

                    return (
                      <div
                        key={key}
                        className="flex items-center justify-between"
                      >
                        <div className="flex items-center gap-2">
                          <span
                            className={`px-1.5 py-0.5 text-xs rounded ${
                              entry.enabled
                                ? "bg-brightwing-blue/20 text-brightwing-blue"
                                : "bg-brightwing-gray-700 text-brightwing-gray-500"
                            }`}
                          >
                            {entry.shortName}
                          </span>
                          {!entry.enabled && (
                            <span className="text-xs text-brightwing-gray-500">
                              disabled
                            </span>
                          )}
                        </div>
                        {canToggle ? (
                          <button
                            onClick={() => handleToggle(name, entry)}
                            disabled={isToggling}
                            className={`relative w-10 h-5 rounded-full transition-colors disabled:opacity-50 ${
                              entry.enabled
                                ? "bg-green-500"
                                : "bg-brightwing-gray-600"
                            }`}
                          >
                            <span
                              className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                                entry.enabled
                                  ? "translate-x-5"
                                  : "translate-x-0.5"
                              }`}
                            />
                          </button>
                        ) : (
                          <span className="text-xs text-brightwing-gray-600">
                            read-only
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        )}
        <p className="text-xs text-brightwing-gray-500 mt-4">
          Toggle switches disable/enable MCP servers in each tool's config file.
          Most tools need a restart after changes.
        </p>
      </section>
    </div>
  );
}
