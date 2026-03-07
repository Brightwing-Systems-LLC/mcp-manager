import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useStore } from "./store";
import Navigation from "./components/Navigation";
import Dashboard from "./components/Dashboard";
import Search from "./components/Search";
import Favorites from "./components/Favorites";
import InstallDialog from "./components/InstallDialog";
import About from "./components/About";
import Toast from "./components/Toast";
import type { DeepLinkAction } from "./lib/types";

export default function App() {
  const { view, refreshTools, refreshInstallations, refreshFavorites, checkPendingDeepLink, setPendingDeepLink, setView } =
    useStore();

  useEffect(() => {
    // Initial data load
    refreshTools();
    refreshInstallations();
    refreshFavorites();
    checkPendingDeepLink();

    // Listen for deep link events from Tauri
    const unlisten = listen<DeepLinkAction>("deep-link", (event) => {
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
        {view === "dashboard" && <Dashboard />}
        {view === "search" && <Search />}
        {view === "favorites" && <Favorites />}
        {view === "install" && <InstallDialog />}
        {view === "about" && <About />}
      </main>
      <Toast />
    </div>
  );
}
