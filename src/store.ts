import { create } from "zustand";
import type {
  DetectedTool,
  Installation,
  Favorite,
  DeepLinkAction,
  View,
  ScoreboardServer,
  ApiInstallConfig,
  ScoreboardSearchResponse,
} from "./lib/types";
import * as tauri from "./lib/tauri";

const API_BASE = "https://patchworkmcp.com/scoreboard/api/v1";

interface AppState {
  // Navigation
  view: View;
  setView: (view: View) => void;

  // Detected tools
  tools: DetectedTool[];
  toolsLoading: boolean;
  refreshTools: () => Promise<void>;

  // Installations
  installations: Installation[];
  installationsLoading: boolean;
  refreshInstallations: () => Promise<void>;

  // Favorites
  favorites: Favorite[];
  favoritesLoading: boolean;
  refreshFavorites: () => Promise<void>;
  toggleFavorite: (server: ScoreboardServer) => Promise<void>;
  isFavorite: (id: number) => boolean;

  // Deep link
  pendingDeepLink: DeepLinkAction | null;
  setPendingDeepLink: (action: DeepLinkAction | null) => void;
  checkPendingDeepLink: () => Promise<void>;

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
  fetchInstallConfig: (serverId: number) => Promise<void>;

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
      const response = await fetch(
        `${API_BASE}/servers/?q=${encodeURIComponent(query)}&per_page=25`
      );
      if (!response.ok) throw new Error(`API error: ${response.status}`);
      const data: ScoreboardSearchResponse = await response.json();
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
      const response = await fetch(
        `${API_BASE}/servers/${serverId}/install-config/`
      );
      if (response.ok) {
        const config: ApiInstallConfig = await response.json();
        set({ installConfig: config, installConfigLoading: false });
      } else if (response.status === 404) {
        // No install config available for this server
        set({ installConfig: null, installConfigLoading: false });
      } else {
        throw new Error(`API error: ${response.status}`);
      }
    } catch (e) {
      console.error("Failed to fetch install config:", e);
      set({ installConfig: null, installConfigLoading: false });
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
