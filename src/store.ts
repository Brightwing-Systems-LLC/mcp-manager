import { create } from "zustand";
import type {
  DetectedTool,
  ConfiguredServer,
  DisabledServer,
  Installation,
  Favorite,
  DeepLinkAction,
  View,
  ScoreboardServer,
  ApiInstallConfig,
  ProxyServer,
  ToolFilterEntry,
  DaemonStatusInfo,
  GovernanceStatus,
  GovernanceAllowlistEntry,
  GovernanceRequest,
  GovernanceAuditEntry,
} from "./lib/types";
import * as tauri from "./lib/tauri";

interface AppState {
  // Navigation
  view: View;
  setView: (view: View) => void;

  // Detected tools
  tools: DetectedTool[];
  toolsLoading: boolean;
  refreshTools: () => Promise<void>;

  // Configured servers (read from tool config files)
  configuredServers: ConfiguredServer[];
  configuredServersLoading: boolean;
  refreshConfiguredServers: () => Promise<void>;

  // Disabled servers
  disabledServers: DisabledServer[];
  refreshDisabledServers: () => Promise<void>;
  disableServer: (toolId: string, serverName: string, configJson: string) => Promise<void>;
  enableServer: (toolId: string, serverName: string) => Promise<void>;

  // Installations
  installations: Installation[];
  installationsLoading: boolean;
  refreshInstallations: () => Promise<void>;

  // Favorites
  favorites: Favorite[];
  favoritesLoading: boolean;
  refreshFavorites: () => Promise<void>;
  toggleFavorite: (server: ScoreboardServer) => Promise<void>;
  isFavorite: (id: string) => boolean;

  // Deep link
  pendingDeepLink: DeepLinkAction | null;
  setPendingDeepLink: (action: DeepLinkAction | null) => void;
  checkPendingDeepLink: () => Promise<void>;

  // Installable server IDs
  installableIds: Set<string>;
  installableIdsLoaded: boolean;
  refreshInstallableIds: () => Promise<void>;
  isInstallable: (id: string) => boolean;

  // Search
  searchQuery: string;
  searchResults: ScoreboardServer[];
  searchLoading: boolean;
  setSearchQuery: (q: string) => void;
  performSearch: (query: string) => Promise<void>;

  // Install dialog
  installTarget: ScoreboardServer | null;
  installConfig: ApiInstallConfig | null;
  installConfigLoading: boolean;
  setInstallTarget: (server: ScoreboardServer | null) => void;
  fetchInstallConfig: (serverId: string) => Promise<void>;

  // Pending restarts
  pendingRestarts: Set<string>;
  addPendingRestart: (toolId: string) => void;
  clearPendingRestart: (toolId: string) => void;
  restartTool: (toolId: string) => Promise<void>;

  // Server detail
  serverDetailId: string | null;
  setServerDetailId: (id: string | null) => void;

  // Proxy servers
  proxyServers: ProxyServer[];
  proxyServersLoading: boolean;
  refreshProxyServers: () => Promise<void>;

  // Tool filter (per-server per-app, loaded on demand)
  activeFilterServerId: string | null;
  activeFilterToolId: string | null;
  activeFilter: ToolFilterEntry[];
  activeFilterLoading: boolean;
  loadToolFilter: (serverId: string, toolId?: string) => Promise<void>;
  toggleToolFilter: (serverId: string, toolName: string, enabled: boolean, tokenEstimate: number, toolId?: string) => Promise<void>;

  // Daemon lifecycle
  daemonStatus: DaemonStatusInfo | null;
  daemonLoading: boolean;
  autostart: boolean;
  refreshDaemonStatus: () => Promise<void>;
  startDaemon: () => Promise<void>;
  stopDaemon: () => Promise<void>;
  toggleAutostart: () => Promise<void>;

  // App updates
  updateAvailable: { version: string; notes: string } | null;
  updateDownloading: boolean;
  updateProgress: number;
  checkForUpdate: () => Promise<void>;
  installUpdate: () => Promise<void>;

  // Governance
  governanceStatus: GovernanceStatus | null;
  governanceAllowlist: GovernanceAllowlistEntry[];
  governanceRequests: GovernanceRequest[];
  governanceAuditLog: GovernanceAuditEntry[];
  governanceLoading: boolean;
  refreshGovernanceStatus: () => Promise<void>;
  refreshGovernanceAllowlist: () => Promise<void>;
  refreshGovernanceRequests: (statusFilter?: string) => Promise<void>;
  refreshGovernanceAuditLog: () => Promise<void>;

  // Notifications
  toast: { message: string; type: "success" | "error" } | null;
  showToast: (message: string, type: "success" | "error") => void;
  clearToast: () => void;
}

export const useStore = create<AppState>((set, get) => ({
  // Navigation
  view: "dashboard",
  setView: (view) => set({ view }),

  // Detected tools
  tools: [],
  toolsLoading: false,
  refreshTools: async () => {
    set({ toolsLoading: true });
    try {
      const tools = await tauri.scanTools();
      set({ tools, toolsLoading: false });
    } catch (e) {
      console.error("Failed to scan tools:", e);
      set({ toolsLoading: false });
    }
  },

  // Configured servers
  configuredServers: [],
  configuredServersLoading: false,
  refreshConfiguredServers: async () => {
    set({ configuredServersLoading: true });
    try {
      const servers = await tauri.scanConfiguredServers();
      set({ configuredServers: servers, configuredServersLoading: false });
    } catch (e) {
      console.error("Failed to scan configured servers:", e);
      set({ configuredServersLoading: false });
    }
  },

  // Disabled servers
  disabledServers: [],
  refreshDisabledServers: async () => {
    try {
      const servers = await tauri.getDisabledServers();
      set({ disabledServers: servers });
    } catch (e) {
      console.error("Failed to get disabled servers:", e);
    }
  },
  disableServer: async (toolId, serverName, configJson) => {
    try {
      const result = await tauri.disableServer(toolId, serverName, configJson);
      if (result.success) {
        get().showToast(result.message, "success");
        if (result.needs_restart) get().addPendingRestart(toolId);
        await get().refreshConfiguredServers();
        await get().refreshDisabledServers();
      } else {
        get().showToast(result.message, "error");
      }
    } catch (e) {
      get().showToast(`Failed to disable: ${e}`, "error");
    }
  },
  enableServer: async (toolId, serverName) => {
    try {
      const result = await tauri.enableServer(toolId, serverName);
      if (result.success) {
        get().showToast(result.message, "success");
        if (result.needs_restart) get().addPendingRestart(toolId);
        await get().refreshConfiguredServers();
        await get().refreshDisabledServers();
      } else {
        get().showToast(result.message, "error");
      }
    } catch (e) {
      get().showToast(`Failed to enable: ${e}`, "error");
    }
  },

  // Installations
  installations: [],
  installationsLoading: false,
  refreshInstallations: async () => {
    set({ installationsLoading: true });
    try {
      const installations = await tauri.getInstallations();
      set({ installations, installationsLoading: false });
    } catch (e) {
      console.error("Failed to get installations:", e);
      set({ installationsLoading: false });
    }
  },

  // Favorites
  favorites: [],
  favoritesLoading: false,
  refreshFavorites: async () => {
    set({ favoritesLoading: true });
    try {
      const favorites = await tauri.getFavorites();
      set({ favorites, favoritesLoading: false });
    } catch (e) {
      console.error("Failed to get favorites:", e);
      set({ favoritesLoading: false });
    }
  },
  toggleFavorite: async (server) => {
    const { favorites } = get();
    const serverId = String(server.id);
    const existing = favorites.find((f) => f.server_uuid === serverId);
    if (existing) {
      await tauri.removeFavorite(serverId);
    } else {
      await tauri.addFavorite({
        serverUuid: serverId,
        serverName: server.name,
        displayName: server.name,
        grade: server.current_grade ?? undefined,
        score: server.current_score ?? undefined,
        language: server.language,
      });
    }
    await get().refreshFavorites();
  },
  isFavorite: (id) => {
    return get().favorites.some((f) => f.server_uuid === String(id));
  },

  // Deep link
  pendingDeepLink: null,
  setPendingDeepLink: (action) => set({ pendingDeepLink: action }),
  checkPendingDeepLink: async () => {
    try {
      const action = await tauri.getPendingDeepLink();
      if (action) {
        set({ pendingDeepLink: action, view: "install" });
        await tauri.clearPendingDeepLink();
      }
    } catch (e) {
      console.error("Failed to check deep link:", e);
    }
  },

  // Installable server IDs
  installableIds: new Set<string>(),
  installableIdsLoaded: false,
  refreshInstallableIds: async () => {
    try {
      const ids = await tauri.apiGetInstallableIds();
      set({ installableIds: new Set(ids), installableIdsLoaded: true });
    } catch (e) {
      console.error("Failed to fetch installable IDs:", e);
    }
  },
  isInstallable: (id) => {
    return get().installableIds.has(id);
  },

  // Search
  searchQuery: "",
  searchResults: [],
  searchLoading: false,
  setSearchQuery: (q) => set({ searchQuery: q }),
  performSearch: async (query) => {
    if (!query.trim()) {
      set({ searchResults: [], searchLoading: false });
      return;
    }
    set({ searchLoading: true });
    try {
      const data = await tauri.apiSearchServers(query, 25);
      set({
        searchResults: data.results,
        searchLoading: false,
      });
    } catch (e) {
      console.error("Search failed:", e);
      set({ searchResults: [], searchLoading: false });
    }
  },

  // Install dialog
  installTarget: null,
  installConfig: null,
  installConfigLoading: false,
  setInstallTarget: (server) => {
    set({
      installTarget: server,
      installConfig: null,
      view: server ? "install" : get().view,
    });
    if (server) {
      get().fetchInstallConfig(server.id);
    }
  },
  fetchInstallConfig: async (serverId) => {
    set({ installConfigLoading: true });
    try {
      const config = await tauri.apiGetInstallConfig(serverId);
      set({ installConfig: config, installConfigLoading: false });
    } catch (e) {
      console.error("Failed to fetch install config:", e);
      set({ installConfig: null, installConfigLoading: false });
    }
  },

  // Pending restarts
  pendingRestarts: new Set<string>(),
  addPendingRestart: (toolId) => {
    set((state) => {
      const next = new Set(state.pendingRestarts);
      next.add(toolId);
      return { pendingRestarts: next };
    });
  },
  clearPendingRestart: (toolId) => {
    set((state) => {
      const next = new Set(state.pendingRestarts);
      next.delete(toolId);
      return { pendingRestarts: next };
    });
  },
  restartTool: async (toolId) => {
    try {
      const msg = await tauri.restartTool(toolId);
      get().showToast(msg, "success");
      get().clearPendingRestart(toolId);
    } catch (e) {
      get().showToast(`Restart failed: ${e}`, "error");
    }
  },

  // Server detail
  serverDetailId: null,
  setServerDetailId: (id) => set({ serverDetailId: id }),

  // Proxy servers
  proxyServers: [],
  proxyServersLoading: false,
  refreshProxyServers: async () => {
    set({ proxyServersLoading: true });
    try {
      const servers = await tauri.getProxyServers();
      set({ proxyServers: servers, proxyServersLoading: false });
    } catch (e) {
      console.error("Failed to get proxy servers:", e);
      set({ proxyServersLoading: false });
    }
  },

  // Tool filter
  activeFilterServerId: null,
  activeFilterToolId: null,
  activeFilter: [],
  activeFilterLoading: false,
  loadToolFilter: async (serverId: string, toolId?: string) => {
    set({ activeFilterServerId: serverId, activeFilterToolId: toolId || null, activeFilterLoading: true });
    try {
      const filter = await tauri.getToolFilter(serverId, toolId);
      set({ activeFilter: filter, activeFilterLoading: false });
    } catch (e) {
      console.error("Failed to load tool filter:", e);
      set({ activeFilter: [], activeFilterLoading: false });
    }
  },
  toggleToolFilter: async (serverId: string, toolName: string, enabled: boolean, tokenEstimate: number, toolId?: string) => {
    try {
      await tauri.setToolFilter(serverId, toolName, enabled, tokenEstimate, toolId);
      // Optimistic update
      set((state) => ({
        activeFilter: state.activeFilter.map((t) =>
          t.tool_name === toolName ? { ...t, enabled } : t
        ),
      }));
    } catch (e) {
      get().showToast(`Failed to update filter: ${e}`, "error");
    }
  },

  // Daemon lifecycle
  daemonStatus: null,
  daemonLoading: false,
  autostart: false,
  refreshDaemonStatus: async () => {
    set({ daemonLoading: true });
    try {
      const [status, autostart] = await Promise.all([
        tauri.daemonStatus(),
        tauri.isAutostartEnabled(),
      ]);
      set({ daemonStatus: status, autostart, daemonLoading: false });
    } catch (e) {
      console.error("Failed to get daemon status:", e);
      set({ daemonLoading: false });
    }
  },
  startDaemon: async () => {
    try {
      const status = await tauri.startDaemon();
      set({ daemonStatus: status });
      get().showToast("Daemon started", "success");
    } catch (e) {
      get().showToast(`Failed to start daemon: ${e}`, "error");
    }
  },
  stopDaemon: async () => {
    try {
      const status = await tauri.stopDaemon();
      set({ daemonStatus: status });
      get().showToast("Daemon stopped", "success");
    } catch (e) {
      get().showToast(`Failed to stop daemon: ${e}`, "error");
    }
  },
  toggleAutostart: async () => {
    try {
      const newValue = !get().autostart;
      await tauri.setAutostart(newValue);
      set({ autostart: newValue });
      get().showToast(
        newValue ? "Auto-start enabled" : "Auto-start disabled",
        "success"
      );
    } catch (e) {
      get().showToast(`Failed to toggle auto-start: ${e}`, "error");
    }
  },

  // App updates
  updateAvailable: null,
  updateDownloading: false,
  updateProgress: 0,
  checkForUpdate: async () => {
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        set({
          updateAvailable: {
            version: update.version,
            notes: update.body || "",
          },
        });
        // Store the update object for later install
        (get() as unknown as Record<string, unknown>)._updateObj = update;
      }
    } catch (e) {
      console.error("Update check failed:", e);
    }
  },
  installUpdate: async () => {
    const updateObj = (get() as unknown as Record<string, unknown>)._updateObj as
      | { downloadAndInstall: (cb?: (event: { event: string; data: { contentLength?: number; chunkLength?: number } }) => void) => Promise<void> }
      | undefined;
    if (!updateObj) return;
    set({ updateDownloading: true, updateProgress: 0 });
    try {
      let downloaded = 0;
      await updateObj.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          downloaded = 0;
        } else if (event.event === "Progress" && event.data.chunkLength) {
          downloaded += event.data.chunkLength;
        } else if (event.event === "Finished") {
          set({ updateProgress: 100 });
        }
        // Estimate progress if we have content length
        if (downloaded > 0) {
          set({ updateProgress: Math.min(99, Math.round(downloaded / 1024 / 1024)) });
        }
      });
      // Relaunch the app
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      get().showToast(`Update failed: ${e}`, "error");
      set({ updateDownloading: false });
    }
  },

  // Governance
  governanceStatus: null,
  governanceAllowlist: [],
  governanceRequests: [],
  governanceAuditLog: [],
  governanceLoading: false,
  refreshGovernanceStatus: async () => {
    try {
      const status = await tauri.getGovernanceStatus();
      set({ governanceStatus: status });
    } catch (e) {
      console.error("Failed to get governance status:", e);
    }
  },
  refreshGovernanceAllowlist: async () => {
    set({ governanceLoading: true });
    try {
      const allowlist = await tauri.governanceGetAllowlist();
      set({ governanceAllowlist: allowlist, governanceLoading: false });
    } catch (e) {
      console.error("Failed to get governance allowlist:", e);
      set({ governanceLoading: false });
    }
  },
  refreshGovernanceRequests: async (statusFilter?: string) => {
    try {
      const requests = await tauri.governanceGetRequests(statusFilter);
      set({ governanceRequests: requests });
    } catch (e) {
      console.error("Failed to get governance requests:", e);
    }
  },
  refreshGovernanceAuditLog: async () => {
    try {
      const log = await tauri.governanceGetAuditLog(200);
      set({ governanceAuditLog: log });
    } catch (e) {
      console.error("Failed to get governance audit log:", e);
    }
  },

  // Notifications
  toast: null,
  showToast: (message, type) => {
    set({ toast: { message, type } });
    setTimeout(() => set({ toast: null }), 4000);
  },
  clearToast: () => set({ toast: null }),
}));
