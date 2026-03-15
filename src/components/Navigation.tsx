import { useEffect } from "react";
import { useStore } from "../store";
import type { View } from "../lib/types";
import birdIcon from "../assets-logo-bird.png";

const APP_VERSION = "0.3.14";

const baseNavItems: { id: View; label: string; icon: string }[] = [
  { id: "dashboard", label: "Dashboard", icon: "grid" },
  { id: "search", label: "Search", icon: "search" },
  { id: "add-server", label: "Add Server", icon: "plus" },
  { id: "proxy", label: "Proxy", icon: "shield" },
  { id: "api-keys", label: "API Keys", icon: "key" },
  { id: "cli", label: "CLI", icon: "terminal" },
  { id: "about", label: "About", icon: "info" },
];

const icons: Record<string, JSX.Element> = {
  grid: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M3.75 6A2.25 2.25 0 0 1 6 3.75h2.25A2.25 2.25 0 0 1 10.5 6v2.25a2.25 2.25 0 0 1-2.25 2.25H6a2.25 2.25 0 0 1-2.25-2.25V6ZM3.75 15.75A2.25 2.25 0 0 1 6 13.5h2.25a2.25 2.25 0 0 1 2.25 2.25V18a2.25 2.25 0 0 1-2.25 2.25H6A2.25 2.25 0 0 1 3.75 18v-2.25ZM13.5 6a2.25 2.25 0 0 1 2.25-2.25H18A2.25 2.25 0 0 1 20.25 6v2.25A2.25 2.25 0 0 1 18 10.5h-2.25a2.25 2.25 0 0 1-2.25-2.25V6ZM13.5 15.75a2.25 2.25 0 0 1 2.25-2.25H18a2.25 2.25 0 0 1 2.25 2.25V18A2.25 2.25 0 0 1 18 20.25h-2.25a2.25 2.25 0 0 1-2.25-2.25v-2.25Z" />
    </svg>
  ),
  search: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z" />
    </svg>
  ),
  star: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M11.48 3.499a.562.562 0 0 1 1.04 0l2.125 5.111a.563.563 0 0 0 .475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 0 0-.182.557l1.285 5.385a.562.562 0 0 1-.84.61l-4.725-2.885a.562.562 0 0 0-.586 0L6.982 20.54a.562.562 0 0 1-.84-.61l1.285-5.386a.562.562 0 0 0-.182-.557l-4.204-3.602a.562.562 0 0 1 .321-.988l5.518-.442a.563.563 0 0 0 .475-.345L11.48 3.5Z" />
    </svg>
  ),
  plus: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
    </svg>
  ),
  shield: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75 11.25 15 15 9.75m-3-7.036A11.959 11.959 0 0 1 3.598 6 11.99 11.99 0 0 0 3 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285Z" />
    </svg>
  ),
  key: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 5.25a3 3 0 0 1 3 3m3 0a6 6 0 0 1-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1 1 21.75 8.25Z" />
    </svg>
  ),
  lock: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M16.5 10.5V6.75a4.5 4.5 0 1 0-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 0 0 2.25-2.25v-6.75a2.25 2.25 0 0 0-2.25-2.25H6.75a2.25 2.25 0 0 0-2.25 2.25v6.75a2.25 2.25 0 0 0 2.25 2.25Z" />
    </svg>
  ),
  terminal: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="m6.75 7.5 3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z" />
    </svg>
  ),
  info: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="m11.25 11.25.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z" />
    </svg>
  ),
};

export default function Navigation() {
  const { view, setView, updateAvailable, updateDownloading, installUpdate, checkForUpdate, governanceStatus, refreshGovernanceStatus } = useStore();

  // Check for updates on mount and every 4 hours
  useEffect(() => {
    checkForUpdate();
    refreshGovernanceStatus();
    const interval = setInterval(checkForUpdate, 4 * 60 * 60 * 1000);
    return () => clearInterval(interval);
  }, [checkForUpdate, refreshGovernanceStatus]);

  // Only show governance nav when it's been set up or an external policy exists
  const showGovernance = governanceStatus && (governanceStatus.has_admin_pin || governanceStatus.policy_enforced);
  const navItems = showGovernance
    ? [
        ...baseNavItems.slice(0, 5),
        { id: "governance" as View, label: "Governance", icon: "lock" },
        ...baseNavItems.slice(5),
      ]
    : baseNavItems;

  return (
    <nav className="w-56 bg-brightwing-gray-800 border-r border-brightwing-gray-700 flex flex-col">
      {/* App name */}
      <div className="px-4 py-5 border-b border-brightwing-gray-700">
        <div className="flex items-center gap-2.5">
          <img src={birdIcon} alt="Brightwing" className="w-9 h-9" />
          <span className="text-lg font-semibold text-brightwing-gray-100">
            MCP Manager
          </span>
        </div>
      </div>

      {/* Nav items */}
      <div className="flex-1 py-3">
        {navItems.map((item) => (
          <button
            key={item.id}
            onClick={() => setView(item.id)}
            className={`w-full flex items-center gap-3 px-4 py-2.5 text-sm transition-colors ${
              view === item.id
                ? "bg-brightwing-blue/10 text-brightwing-blue border-r-2 border-brightwing-blue"
                : "text-brightwing-gray-300 hover:bg-brightwing-gray-700 hover:text-brightwing-gray-100"
            }`}
          >
            {icons[item.icon]}
            {item.label}
          </button>
        ))}
      </div>

      {/* Footer */}
      <div className="px-4 py-3 border-t border-brightwing-gray-700">
        {updateAvailable ? (
          <button
            onClick={installUpdate}
            disabled={updateDownloading}
            className="w-full flex items-center gap-2 px-2 py-1.5 text-xs font-medium bg-green-600/20 hover:bg-green-600/30 text-green-400 rounded-md transition-colors disabled:opacity-60"
          >
            <svg className="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3" />
            </svg>
            {updateDownloading ? "Updating..." : `Update to v${updateAvailable.version}`}
          </button>
        ) : (
          <p className="text-xs text-brightwing-gray-500">v{APP_VERSION}</p>
        )}
      </div>
    </nav>
  );
}
