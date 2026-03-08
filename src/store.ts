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

  // Notifications
  toast: null,
  showToast: (message, type) => {
    set({ toast: { message, type } });
    setTimeout(() => set({ toast: null }), 4000);
  },
  clearToast: () => set({ toast: null }),
}));
