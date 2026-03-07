import { useEffect } from "react";
import { useStore } from "../store";

export default function Dashboard() {
  const {
    tools,
    toolsLoading,
    refreshTools,
    configuredServers,
    configuredServersLoading,
    refreshConfiguredServers,
  } = useStore();

  useEffect(() => {
    refreshConfiguredServers();
  }, [refreshConfiguredServers]);

  const detectedTools = tools.filter((t) => t.detected);
  const notDetected = tools.filter((t) => !t.detected);

  // Group configured servers by name, collecting which tools they're in
  const serverMap = new Map<string, { name: string; tools: { id: string; shortName: string }[] }>();
  for (const cs of configuredServers) {
    const existing = serverMap.get(cs.server_name);
    if (existing) {
      if (!existing.tools.some((t) => t.id === cs.tool_id)) {
        existing.tools.push({ id: cs.tool_id, shortName: cs.tool_short_name });
      }
    } else {
      serverMap.set(cs.server_name, {
        name: cs.server_name,
        tools: [{ id: cs.tool_id, shortName: cs.tool_short_name }],
      });
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Dashboard</h1>
        <div className="flex gap-2">
          <button
            onClick={refreshConfiguredServers}
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
          Configured MCP Servers ({serverMap.size})
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
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-brightwing-gray-700">
                  <th className="text-left px-4 py-2.5 text-brightwing-gray-400 font-medium">
                    Server
                  </th>
                  <th className="text-left px-4 py-2.5 text-brightwing-gray-400 font-medium">
                    Configured In
                  </th>
                </tr>
              </thead>
              <tbody>
                {Array.from(serverMap.entries()).map(([name, server]) => (
                  <tr
                    key={name}
                    className="border-b border-brightwing-gray-700/50 hover:bg-brightwing-gray-700/30"
                  >
                    <td className="px-4 py-2.5 font-mono">
                      {server.name}
                    </td>
                    <td className="px-4 py-2.5">
                      <div className="flex gap-1.5 flex-wrap">
                        {server.tools.map((tool) => (
                          <span
                            key={tool.id}
                            className="px-1.5 py-0.5 text-xs bg-brightwing-blue/20 text-brightwing-blue rounded"
                          >
                            {tool.shortName}
                          </span>
                        ))}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
