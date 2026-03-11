import { useState, useCallback, useEffect } from "react";
import { useStore } from "../store";
import type { ScoreboardServer } from "../lib/types";

function gradeColor(grade: string | null): string {
  if (!grade) return "text-brightwing-gray-500";
  if (grade.startsWith("A")) return "text-green-400";
  if (grade.startsWith("B")) return "text-blue-400";
  if (grade.startsWith("C")) return "text-yellow-400";
  if (grade.startsWith("D")) return "text-orange-400";
  return "text-red-400";
}

export default function Search() {
  const {
    searchQuery,
    searchResults,
    searchLoading,
    setSearchQuery,
    performSearch,
    setInstallTarget,
    isFavorite,
    toggleFavorite,
    isInstallable,
    installableIdsLoaded,
    refreshInstallableIds,
  } = useStore();

  const [debounceTimer, setDebounceTimer] = useState<ReturnType<typeof setTimeout> | null>(null);

  // Load installable IDs in background for "verified" badges — non-blocking
  useEffect(() => {
    if (!installableIdsLoaded) {
      refreshInstallableIds();
    }
  }, [installableIdsLoaded, refreshInstallableIds]);

  const handleInput = useCallback(
    (value: string) => {
      setSearchQuery(value);
      if (debounceTimer) clearTimeout(debounceTimer);
      const timer = setTimeout(() => {
        performSearch(value);
      }, 400);
      setDebounceTimer(timer);
    },
    [debounceTimer, setSearchQuery, performSearch]
  );

  return (
    <div>
      <h1 className="text-2xl font-semibold mb-6">Search MCP Servers</h1>

      {/* Search input */}
      <div className="relative mb-6">
        <svg
          className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-brightwing-gray-400"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={1.5}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"
          />
        </svg>
        <input
          type="text"
          placeholder="Search by name, category, or keyword..."
          value={searchQuery}
          onChange={(e) => handleInput(e.target.value)}
          className="w-full pl-10 pr-4 py-3 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg text-sm placeholder-brightwing-gray-500 focus:outline-none focus:border-brightwing-blue focus:ring-1 focus:ring-brightwing-blue"
        />
      </div>

      {/* Results */}
      {searchLoading ? (
        <p className="text-brightwing-gray-500 text-sm">Searching...</p>
      ) : searchResults.length === 0 && searchQuery ? (
        <p className="text-brightwing-gray-500 text-sm">
          No results for "{searchQuery}"
        </p>
      ) : (
        <div className="space-y-2">
          {searchResults.map((server) => (
            <ServerRow
              key={server.id}
              server={server}
              isFav={isFavorite(server.id)}
              verified={isInstallable(server.id)}
              onInstall={() => setInstallTarget(server)}
              onToggleFav={() => toggleFavorite(server)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function ServerRow({
  server,
  isFav,
  verified,
  onInstall,
  onToggleFav,
}: {
  server: ScoreboardServer;
  isFav: boolean;
  verified: boolean;
  onInstall: () => void;
  onToggleFav: () => void;
}) {
  return (
    <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4 flex items-center gap-4 hover:border-brightwing-gray-600 transition-colors">
      {/* Grade */}
      <div className="text-center w-12">
        <span className={`text-lg font-bold ${gradeColor(server.current_grade)}`}>
          {server.current_grade || "?"}
        </span>
        {server.current_score != null && (
          <p className="text-xs text-brightwing-gray-500">{server.current_score}</p>
        )}
      </div>

      {/* Info */}
      <div className="flex-1 min-w-0">
        <h3 className="font-mono font-medium truncate">{server.name}</h3>
        <p className="text-sm text-brightwing-gray-400 truncate">
          {server.description}
        </p>
        <div className="flex gap-2 mt-1">
          {server.language && (
            <span className="text-xs text-brightwing-gray-500">
              {server.language}
            </span>
          )}
          {server.stars_count > 0 && (
            <span className="text-xs text-brightwing-gray-500">
              {server.stars_count} stars
            </span>
          )}
          {server.is_remote && (
            <span className="text-xs text-cyan-500">remote</span>
          )}
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 shrink-0">
        {verified && (
          <span className="text-xs text-green-400" title="Verified install config">
            <svg className="w-4 h-4 inline" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
            </svg>
          </span>
        )}
        <button
          onClick={onToggleFav}
          className={`p-2 rounded-md transition-colors ${
            isFav
              ? "text-brightwing-orange hover:bg-brightwing-orange/10"
              : "text-brightwing-gray-500 hover:bg-brightwing-gray-700"
          }`}
          title={isFav ? "Remove from favorites" : "Add to favorites"}
        >
          <svg className="w-5 h-5" fill={isFav ? "currentColor" : "none"} viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M11.48 3.499a.562.562 0 0 1 1.04 0l2.125 5.111a.563.563 0 0 0 .475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 0 0-.182.557l1.285 5.385a.562.562 0 0 1-.84.61l-4.725-2.885a.562.562 0 0 0-.586 0L6.982 20.54a.562.562 0 0 1-.84-.61l1.285-5.386a.562.562 0 0 0-.182-.557l-4.204-3.602a.562.562 0 0 1 .321-.988l5.518-.442a.563.563 0 0 0 .475-.345L11.48 3.5Z" />
          </svg>
        </button>
        <button
          onClick={onInstall}
          className="px-3 py-1.5 text-sm bg-brightwing-blue hover:bg-brightwing-blue-dark text-white rounded-md transition-colors"
        >
          Install
        </button>
      </div>
    </div>
  );
}
