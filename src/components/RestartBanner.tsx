import { useState } from "react";
import { useStore } from "../store";

const TOOL_NAMES: Record<string, string> = {
  claude_desktop: "Claude Desktop",
  cursor: "Cursor",
  windsurf: "Windsurf",
  vscode: "VS Code",
  codex: "OpenAI Codex",
  gemini_cli: "Gemini CLI",
  antigravity: "Antigravity",
};

// IDE tools that may have unsaved work
const WARN_TOOLS = new Set(["cursor", "windsurf", "vscode"]);

export default function RestartBanner() {
  const { pendingRestarts, restartTool, clearPendingRestart } = useStore();
  const [restarting, setRestarting] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  if (pendingRestarts.size === 0) return null;

  const toolIds = Array.from(pendingRestarts);

  const handleRestart = async (toolId: string) => {
    setRestarting(toolId);
    setConfirming(null);
    await restartTool(toolId);
    setRestarting(null);
  };

  return (
    <div className="bg-amber-500/10 border border-amber-500/30 rounded-lg px-4 py-3 mb-4">
      <div className="flex items-start gap-3">
        <svg
          className="w-5 h-5 text-amber-400 mt-0.5 shrink-0"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={1.5}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182M2.985 19.644l3.181-3.18"
          />
        </svg>
        <div className="flex-1 min-w-0">
          <p className="text-sm text-amber-200 font-medium">
            Restart needed — changes won't take effect until these tools are restarted
          </p>
          <div className="flex flex-wrap gap-2 mt-2">
            {toolIds.map((toolId) => {
              const isRestarting = restarting === toolId;
              const isConfirming = confirming === toolId;
              const needsWarn = WARN_TOOLS.has(toolId);

              if (isConfirming) {
                return (
                  <div
                    key={toolId}
                    className="flex items-center gap-1.5 bg-amber-500/20 rounded-md px-2 py-1"
                  >
                    <span className="text-xs text-amber-200">
                      {needsWarn
                        ? "Save work first!"
                        : `Restart ${TOOL_NAMES[toolId] || toolId}?`}
                    </span>
                    <button
                      onClick={() => handleRestart(toolId)}
                      className="text-xs px-1.5 py-0.5 bg-amber-500 text-black rounded font-medium hover:bg-amber-400"
                    >
                      Yes
                    </button>
                    <button
                      onClick={() => setConfirming(null)}
                      className="text-xs px-1.5 py-0.5 text-amber-300 hover:text-amber-100"
                    >
                      No
                    </button>
                  </div>
                );
              }

              return (
                <div key={toolId} className="flex items-center gap-1.5">
                  <button
                    onClick={() => setConfirming(toolId)}
                    disabled={isRestarting}
                    className="flex items-center gap-1.5 px-2.5 py-1 text-xs bg-amber-500/20 hover:bg-amber-500/30 text-amber-200 rounded-md transition-colors disabled:opacity-50"
                  >
                    {isRestarting ? (
                      "Restarting..."
                    ) : (
                      <>
                        <span className="font-medium">
                          {TOOL_NAMES[toolId] || toolId}
                        </span>
                        <svg
                          className="w-3 h-3"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                          strokeWidth={2}
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182M2.985 19.644l3.181-3.18"
                          />
                        </svg>
                      </>
                    )}
                  </button>
                  <button
                    onClick={() => clearPendingRestart(toolId)}
                    className="text-amber-500/50 hover:text-amber-300 p-0.5"
                    title="Dismiss"
                  >
                    <svg
                      className="w-3 h-3"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth={2}
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M6 18 18 6M6 6l12 12"
                      />
                    </svg>
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
