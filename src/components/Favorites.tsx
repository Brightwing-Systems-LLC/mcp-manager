import { useStore } from "../store";

function gradeColor(grade: string | null): string {
  if (!grade) return "text-brightwing-gray-500";
  if (grade.startsWith("A")) return "text-green-400";
  if (grade.startsWith("B")) return "text-blue-400";
  if (grade.startsWith("C")) return "text-yellow-400";
  if (grade.startsWith("D")) return "text-orange-400";
  return "text-red-400";
}

export default function Favorites() {
  const { favorites, favoritesLoading, refreshFavorites } = useStore();

  const handleRemove = async (uuid: string) => {
    try {
      await import("../lib/tauri").then((t) => t.removeFavorite(uuid));
      await refreshFavorites();
    } catch (e) {
      console.error("Failed to remove favorite:", e);
    }
  };

  return (
    <div>
      <h1 className="text-2xl font-semibold mb-6">Favorites</h1>

      {favoritesLoading ? (
        <p className="text-brightwing-gray-500 text-sm">Loading...</p>
      ) : favorites.length === 0 ? (
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
          <p className="text-brightwing-gray-400">No favorites yet.</p>
          <p className="text-brightwing-gray-500 text-sm mt-1">
            Search for MCP servers and star them to add favorites.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {favorites.map((fav) => (
            <div
              key={fav.server_uuid}
              className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4 flex items-center gap-4"
            >
              {/* Grade */}
              <div className="text-center w-12">
                <span
                  className={`text-lg font-bold ${gradeColor(fav.grade)}`}
                >
                  {fav.grade || "?"}
                </span>
                {fav.score != null && (
                  <p className="text-xs text-brightwing-gray-500">
                    {fav.score}
                  </p>
                )}
              </div>

              {/* Info */}
              <div className="flex-1 min-w-0">
                <h3 className="font-mono font-medium truncate">
                  {fav.display_name || fav.server_name}
                </h3>
                <div className="flex gap-2 mt-1">
                  {fav.language && (
                    <span className="text-xs text-brightwing-gray-500">
                      {fav.language}
                    </span>
                  )}
                </div>
              </div>

              {/* Actions */}
              <button
                onClick={() => handleRemove(fav.server_uuid)}
                className="p-2 text-brightwing-gray-400 hover:text-red-400 hover:bg-red-400/10 rounded-md transition-colors"
                title="Remove from favorites"
              >
                <svg
                  className="w-5 h-5"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={1.5}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M6 18 18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
