import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useStore } from "./store";
import * as tauri from "./lib/tauri";
import Navigation from "./components/Navigation";
import Dashboard from "./components/Dashboard";
import Search from "./components/Search";
import Favorites from "./components/Favorites";
import InstallDialog from "./components/InstallDialog";
import AddServer from "./components/AddServer";
import About from "./components/About";
import ProxyServers from "./components/ProxyServers";
import ServerDetail from "./components/ServerDetail";
import ApiKeysPanel from "./components/ApiKeysPanel";
import Governance from "./components/Governance";
import CliPage from "./components/CliPage";
import Toast from "./components/Toast";
import RestartBanner from "./components/RestartBanner";
import type { DeepLinkAction } from "./lib/types";

export default function App() {
  const { view, refreshTools, refreshInstallations, refreshFavorites, refreshProxyServers, checkPendingDeepLink, setPendingDeepLink, setView } =
    useStore();

  useEffect(() => {
    // Initial data load
    refreshTools();
    refreshInstallations();
    refreshFavorites();
    refreshProxyServers();
    checkPendingDeepLink();

    // Auto-install CLI binaries silently
    tauri.distributeBinaries().catch(() => {});

    // Listen for deep link events from Tauri
    const unlisten = listen<DeepLinkAction>("deep-link-action", (event) => {
      setPendingDeepLink(event.payload);
      setView("install");
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="flex h-screen overflow-hidden">
      <Navigation />
      <main className="flex-1 overflow-y-auto p-6">
        <RestartBanner />
        {view === "dashboard" && <Dashboard />}
        {view === "search" && <Search />}
        {view === "favorites" && <Favorites />}
        {view === "install" && <InstallDialog />}
        {view === "add-server" && <AddServer />}
        {view === "proxy" && <ProxyServers />}
        {view === "server-detail" && <ServerDetail />}
        {view === "api-keys" && <ApiKeysPanel />}
        {view === "governance" && <Governance />}
        {view === "cli" && <CliPage />}
        {view === "about" && <About />}
      </main>
      <Toast />
    </div>
  );
}
