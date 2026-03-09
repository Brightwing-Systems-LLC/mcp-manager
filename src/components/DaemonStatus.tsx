import { useEffect } from "react";
import { useStore } from "../store";

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${h}h ${m}m`;
}

export default function DaemonStatus() {
  const {
    daemonStatus,
    daemonLoading,
    autostart,
    refreshDaemonStatus,
    startDaemon,
    stopDaemon,
    toggleAutostart,
  } = useStore();

  useEffect(() => {
    refreshDaemonStatus();
    const interval = setInterval(refreshDaemonStatus, 10_000);
    return () => clearInterval(interval);
  }, [refreshDaemonStatus]);

  const running = daemonStatus?.running ?? false;

  return (
    <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4 space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-brightwing-gray-200">
          Auth Daemon
        </h3>
        <div className="flex items-center gap-2">
          <span
            className={`w-2 h-2 rounded-full ${
              running ? "bg-green-400" : "bg-brightwing-gray-500"
            }`}
          />
          <span className="text-xs text-brightwing-gray-400">
            {daemonLoading
              ? "Checking..."
              : running
              ? "Running"
              : "Stopped"}
          </span>
        </div>
      </div>

      {running && daemonStatus && (
        <div className="text-xs text-brightwing-gray-500 space-y-0.5">
          {daemonStatus.pid && <div>PID: {daemonStatus.pid}</div>}
          {daemonStatus.uptime_secs != null && (
            <div>Uptime: {formatUptime(daemonStatus.uptime_secs)}</div>
          )}
          {daemonStatus.daemon_version && (
            <div>Version: v{daemonStatus.daemon_version}</div>
          )}
        </div>
      )}

      <div className="flex items-center gap-2">
        {running ? (
          <button
            onClick={stopDaemon}
            className="px-3 py-1.5 text-xs font-medium text-red-400 bg-red-500/10 hover:bg-red-500/20 rounded-md transition-colors"
          >
            Stop
          </button>
        ) : (
          <button
            onClick={startDaemon}
            className="px-3 py-1.5 text-xs font-medium text-green-400 bg-green-500/10 hover:bg-green-500/20 rounded-md transition-colors"
          >
            Start
          </button>
        )}
        <button
          onClick={refreshDaemonStatus}
          disabled={daemonLoading}
          className="px-3 py-1.5 text-xs text-brightwing-gray-400 hover:bg-brightwing-gray-700 rounded-md transition-colors disabled:opacity-50"
        >
          Refresh
        </button>
      </div>

      {/* Auto-start toggle */}
      <div className="flex items-center justify-between pt-2 border-t border-brightwing-gray-700">
        <span className="text-xs text-brightwing-gray-400">
          Start on login
        </span>
        <button
          onClick={toggleAutostart}
          className={`relative w-9 h-5 rounded-full transition-colors ${
            autostart ? "bg-brightwing-blue" : "bg-brightwing-gray-600"
          }`}
        >
          <span
            className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
              autostart ? "translate-x-4" : ""
            }`}
          />
        </button>
      </div>
    </div>
  );
}
