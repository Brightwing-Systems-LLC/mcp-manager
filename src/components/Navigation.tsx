import { useStore } from "../store";
import type { View } from "../lib/types";
import birdIcon from "../assets-logo-bird.png";

const navItems: { id: View; label: string; icon: string }[] = [
  { id: "dashboard", label: "Dashboard", icon: "grid" },
  { id: "search", label: "Search", icon: "search" },
  { id: "favorites", label: "Favorites", icon: "star" },
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
  info: (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="m11.25 11.25.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z" />
    </svg>
  ),
};

export default function Navigation() {
  const { view, setView } = useStore();

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
        <p className="text-xs text-brightwing-gray-500">v0.1.0</p>
      </div>
    </nav>
  );
}
