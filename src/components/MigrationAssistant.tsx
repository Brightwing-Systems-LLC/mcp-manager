import { useState, useEffect } from "react";
import { useStore } from "../store";
import {
  registerProxyServer,
  storeApiKey,
  installProxyToTool,
  uninstallServer,
  daemonStatus,
  startDaemon,
} from "../lib/tauri";
import type { ConfiguredServer } from "../lib/types";

/** A server that can be migrated to the proxy. */
interface MigratableServer {
  serverName: string;
  toolId: string;
  toolShortName: string;
  env: Record<string, string>;
  command: string;
  args: string[];
  transport: string;
  url: string;
  configJson: string;
}

/** Parse config JSON and extract env vars. Returns null if no env vars found. */
function parseMigratable(server: ConfiguredServer): MigratableServer | null {
  if (!server.config_json) return null;
  try {
    const config = JSON.parse(server.config_json);
    const env = config.env || {};
    if (Object.keys(env).length === 0) return null;

    return {
      serverName: server.server_name,
      toolId: server.tool_id,
      toolShortName: server.tool_short_name,
      env,
      command: config.command || "",
      args: config.args || [],
      transport: config.url ? "http" : "stdio",
      url: config.url || "",
      configJson: server.config_json,
    };
  } catch {
    return null;
  }
}

/** Group migratable servers by server name (same server across multiple tools). */
function groupByServer(servers: MigratableServer[]): Map<string, MigratableServer[]> {
  const grouped = new Map<string, MigratableServer[]>();
  for (const s of servers) {
    const existing = grouped.get(s.serverName) || [];
    existing.push(s);
    grouped.set(s.serverName, existing);
  }
  return grouped;
}

export default function MigrationAssistant() {
  const {
    configuredServers,
    proxyServers,
    refreshConfiguredServers,
    refreshProxyServers,
    refreshInstallations,
    showToast,
    addPendingRestart,
  } = useStore();

  const [dismissed, setDismissed] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [migrating, setMigrating] = useState<string | null>(null);
  const [completed, setCompleted] = useState<Set<string>>(new Set());

  // Find migratable servers (have env vars, not already proxied)
  const proxyServerIds = new Set(proxyServers.map((p) => p.server_id));
  const allMigratable = configuredServers
    .map(parseMigratable)
    .filter((s): s is MigratableServer => s !== null)
    .filter((s) => !proxyServerIds.has(s.serverName));

  const grouped = groupByServer(allMigratable);
  const migratableCount = grouped.size;

  // Reset dismissed state when new migratable servers appear
  useEffect(() => {
    if (migratableCount === 0) setDismissed(false);
  }, [migratableCount]);

  if (dismissed || migratableCount === 0) return null;

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

  const handleMigrate = async (serverName: string) => {
    const instances = grouped.get(serverName);
    if (!instances || instances.length === 0) return;

    setMigrating(serverName);

    try {
      const daemonOk = await ensureDaemon();
      if (!daemonOk) {
        setMigrating(null);
        return;
      }

      // Use the first instance's config as the template
      const template = instances[0];

      // 1. Register server with daemon
      await registerProxyServer({
        serverId: serverName,
        displayName: serverName,
        authType: "api_key",
        upstreamCommand: template.transport === "stdio" ? template.command : undefined,
        upstreamArgs: template.transport === "stdio" ? template.args.join(" ") : undefined,
        upstreamUrl: template.transport === "http" ? template.url : undefined,
      });

      // 2. Store credentials
      await storeApiKey(serverName, template.env);

      // 3. For each tool instance: uninstall direct config, install proxy config
      let successCount = 0;
      for (const instance of instances) {
        try {
          // Remove the direct install
          const removeResult = await uninstallServer(
            instance.toolId,
            instance.serverName,
            ""
          );
          if (!removeResult.success) {
            showToast(`Failed to remove ${serverName} from ${instance.toolShortName}: ${removeResult.message}`, "error");
            continue;
          }

          // Install proxy config
          const installResult = await installProxyToTool(
            instance.toolId,
            serverName,
            serverName
          );
          if (installResult.success) {
            successCount++;
            if (installResult.needs_restart) addPendingRestart(instance.toolId);
          } else {
            showToast(`Failed to install proxy for ${instance.toolShortName}: ${installResult.message}`, "error");
          }
        } catch (e) {
          showToast(`Migration error for ${instance.toolShortName}: ${e}`, "error");
        }
      }

      if (successCount > 0) {
        showToast(
          `Migrated ${serverName} to proxy in ${successCount} tool${successCount > 1 ? "s" : ""}`,
          "success"
        );
        setCompleted((prev) => new Set(prev).add(serverName));
        await Promise.all([
          refreshConfiguredServers(),
          refreshProxyServers(),
          refreshInstallations(),
        ]);
      }
    } catch (e) {
      showToast(`Migration failed: ${e}`, "error");
    } finally {
      setMigrating(null);
    }
  };

  return (
    <div className="bg-brightwing-gray-800 border border-yellow-500/20 rounded-lg mb-6">
      {/* Collapsed banner */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-3 px-4 py-3 text-left"
      >
        <svg className="w-5 h-5 text-yellow-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
        </svg>
        <div className="flex-1">
          <p className="text-sm font-medium text-yellow-400">
            {migratableCount} server{migratableCount > 1 ? "s" : ""} with plaintext secrets detected
          </p>
          <p className="text-xs text-brightwing-gray-500">
            Migrate to the proxy to secure credentials and authenticate once across all tools.
          </p>
        </div>
        <svg
          className={`w-4 h-4 text-brightwing-gray-500 transition-transform ${expanded ? "rotate-180" : ""}`}
          fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5" />
        </svg>
      </button>

      {/* Expanded list */}
      {expanded && (
        <div className="px-4 pb-4 space-y-3">
          {[...grouped.entries()].map(([serverName, instances]) => {
            const isDone = completed.has(serverName);
            const isMigrating = migrating === serverName;
            const envKeys = Object.keys(instances[0].env);
            const toolNames = instances.map((i) => i.toolShortName);

            return (
              <div
                key={serverName}
                className={`flex items-center gap-3 p-3 rounded-lg border ${
                  isDone
                    ? "border-green-500/30 bg-green-500/5"
                    : "border-brightwing-gray-700 bg-brightwing-gray-900"
                }`}
              >
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-mono font-medium truncate">
                    {serverName}
                  </p>
                  <p className="text-xs text-brightwing-gray-500 mt-0.5">
                    {toolNames.join(", ")} — {envKeys.length} env var{envKeys.length > 1 ? "s" : ""}: {envKeys.join(", ")}
                  </p>
                </div>
                {isDone ? (
                  <span className="text-xs text-green-400 shrink-0">Migrated</span>
                ) : (
                  <button
                    onClick={() => handleMigrate(serverName)}
                    disabled={isMigrating || migrating !== null}
                    className="px-3 py-1.5 text-xs bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors disabled:opacity-50 shrink-0"
                  >
                    {isMigrating ? "Migrating..." : "Migrate"}
                  </button>
                )}
              </div>
            );
          })}

          <div className="flex justify-end pt-1">
            <button
              onClick={() => setDismissed(true)}
              className="text-xs text-brightwing-gray-500 hover:text-brightwing-gray-400"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
