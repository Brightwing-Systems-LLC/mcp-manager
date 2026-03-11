import { useEffect, useState, useCallback } from "react";
import type { ProxyServer, OAuthStatus } from "../lib/types";
import * as tauri from "../lib/tauri";
import { useStore } from "../store";

interface Props {
  server: ProxyServer;
  onStatusChange?: (status: string) => void;
}

export default function OAuthConnect({ server, onStatusChange }: Props) {
  const { showToast } = useStore();
  const [status, setStatus] = useState<OAuthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [connecting, setConnecting] = useState(false);
  const [pendingState, setPendingState] = useState<string | null>(null);
  const [polling, setPolling] = useState(false);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await tauri.getOAuthStatus(server.server_id);
      // Send desktop notification if token just became expired
      if (s.status === "expired" && status?.status !== "expired") {
        try {
          const { sendNotification, isPermissionGranted, requestPermission } =
            await import("@tauri-apps/plugin-notification");
          let granted = await isPermissionGranted();
          if (!granted) granted = (await requestPermission()) === "granted";
          if (granted) {
            sendNotification({
              title: "Brightwing: Token Expired",
              body: `OAuth token for ${server.display_name} has expired. Open Brightwing to re-authenticate.`,
            });
          }
        } catch {
          // Notification not available (e.g., tests or unsupported platform)
        }
      }
      setStatus(s);
      onStatusChange?.(s.status);
    } catch (e) {
      setStatus({
        status: "error",
        expires_at: null,
        error_message: String(e),
      });
      onStatusChange?.("error");
    }
    setLoading(false);
  }, [server.server_id, server.display_name, status?.status]);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  // Poll for callback completion when a flow is pending
  useEffect(() => {
    if (!pendingState || !polling) return;

    const interval = setInterval(async () => {
      try {
        await tauri.completeOAuthCallback(pendingState);
        // Success — token exchanged
        setPendingState(null);
        setPolling(false);
        setConnecting(false);
        showToast(`Connected to ${server.display_name}`, "success");
        refreshStatus();
      } catch {
        // Callback not received yet — keep polling
      }
    }, 2000);

    // Stop polling after 5 minutes
    const timeout = setTimeout(() => {
      setPolling(false);
      setConnecting(false);
      setPendingState(null);
      showToast("OAuth flow timed out. Please try again.", "error");
    }, 300_000);

    return () => {
      clearInterval(interval);
      clearTimeout(timeout);
    };
  }, [pendingState, polling, server.display_name, showToast, refreshStatus]);

  const handleConnect = async () => {
    if (!server.upstream_url) {
      showToast("Server URL is required for OAuth", "error");
      return;
    }

    setConnecting(true);
    try {
      const flow = await tauri.startOAuthFlow(server.server_id, server.upstream_url);
      setPendingState(flow.state);
      setPolling(true);

      // Open the authorization URL in the user's browser
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(flow.auth_url);
    } catch (e) {
      setConnecting(false);
      showToast(`OAuth failed: ${e}`, "error");
    }
  };

  const handleDisconnect = async () => {
    try {
      await tauri.disconnectOAuth(server.server_id);
      showToast(`Disconnected from ${server.display_name}`, "success");
      refreshStatus();
    } catch (e) {
      showToast(`Disconnect failed: ${e}`, "error");
    }
  };

  const handleRefresh = async () => {
    try {
      await tauri.refreshOAuthToken(server.server_id);
      showToast("Token refreshed", "success");
      refreshStatus();
    } catch (e) {
      showToast(`Refresh failed: ${e}`, "error");
    }
  };

  if (loading) {
    return (
      <div className="text-xs text-brightwing-gray-500">Checking auth...</div>
    );
  }

  const st = status?.status ?? "disconnected";

  return (
    <div className="space-y-2">
      {/* Status indicator */}
      <div className="flex items-center gap-2">
        <span
          className={`w-2 h-2 rounded-full ${
            st === "connected"
              ? "bg-green-400"
              : st === "expired"
              ? "bg-amber-400"
              : st === "error"
              ? "bg-red-400"
              : "bg-brightwing-gray-500"
          }`}
        />
        <span className="text-xs text-brightwing-gray-300 capitalize">{st}</span>
        {status?.expires_at && (
          <span className="text-xs text-brightwing-gray-500">
            expires {new Date(status.expires_at).toLocaleString()}
          </span>
        )}
      </div>

      {status?.error_message && (
        <p className="text-xs text-red-400">{status.error_message}</p>
      )}

      {/* Actions */}
      <div className="flex gap-2">
        {st === "disconnected" || st === "error" ? (
          <button
            onClick={handleConnect}
            disabled={connecting}
            className="px-3 py-1.5 text-xs font-medium bg-purple-600 hover:bg-purple-500 text-white rounded-md transition-colors disabled:opacity-50"
          >
            {connecting ? (
              <span className="flex items-center gap-1.5">
                <svg className="w-3 h-3 animate-spin" viewBox="0 0 24 24" fill="none">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
                Waiting for browser...
              </span>
            ) : (
              "Connect with OAuth"
            )}
          </button>
        ) : st === "expired" ? (
          <>
            <button
              onClick={handleRefresh}
              className="px-3 py-1.5 text-xs font-medium bg-amber-600 hover:bg-amber-500 text-white rounded-md transition-colors"
            >
              Refresh Token
            </button>
            <button
              onClick={handleConnect}
              disabled={connecting}
              className="px-3 py-1.5 text-xs font-medium bg-purple-600 hover:bg-purple-500 text-white rounded-md transition-colors disabled:opacity-50"
            >
              {connecting ? "Waiting..." : "Re-authenticate"}
            </button>
            <button
              onClick={handleDisconnect}
              className="px-3 py-1.5 text-xs text-red-400 hover:bg-red-500/10 rounded-md transition-colors"
            >
              Disconnect
            </button>
          </>
        ) : (
          <>
            <button
              onClick={handleRefresh}
              className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 text-brightwing-gray-300 rounded-md transition-colors"
            >
              Refresh Token
            </button>
            <button
              onClick={handleDisconnect}
              className="px-3 py-1.5 text-xs text-red-400 hover:bg-red-500/10 rounded-md transition-colors"
            >
              Disconnect
            </button>
          </>
        )}
      </div>
    </div>
  );
}
