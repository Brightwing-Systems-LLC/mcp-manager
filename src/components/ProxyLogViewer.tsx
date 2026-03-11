import { useEffect, useState, useRef } from "react";
import type { ProxyServer } from "../lib/types";
import type { ProxyLogEvent } from "../lib/types";
import { getProxyLogs } from "../lib/tauri";

// Map MCP clientInfo.name to short display labels
const CLIENT_LABELS: Record<string, { abbr: string; color: string }> = {
  "gemini-cli": { abbr: "GC", color: "bg-blue-600/20 text-blue-300" },
  "gemini": { abbr: "GC", color: "bg-blue-600/20 text-blue-300" },
  "claude-code": { abbr: "CC", color: "bg-orange-500/20 text-orange-300" },
  "claude-desktop": { abbr: "CD", color: "bg-orange-500/20 text-orange-300" },
  "cursor": { abbr: "CU", color: "bg-cyan-500/20 text-cyan-300" },
  "codex": { abbr: "CX", color: "bg-green-500/20 text-green-300" },
  "codex-mcp-client": { abbr: "CX", color: "bg-green-500/20 text-green-300" },
  "vscode": { abbr: "VS", color: "bg-sky-500/20 text-sky-300" },
  "copilot": { abbr: "CP", color: "bg-sky-500/20 text-sky-300" },
  "windsurf": { abbr: "WS", color: "bg-teal-500/20 text-teal-300" },
  "antigravity": { abbr: "AG", color: "bg-indigo-500/20 text-indigo-300" },
};

function getClientLabel(name: string | null): { abbr: string; color: string } | null {
  if (!name) return null;
  const lower = name.toLowerCase();
  if (CLIENT_LABELS[lower]) return CLIENT_LABELS[lower];
  // Fuzzy match: check if any key is contained in the name
  for (const [key, val] of Object.entries(CLIENT_LABELS)) {
    if (lower.includes(key)) return val;
  }
  // Unknown client — show first 2 chars
  return { abbr: name.slice(0, 2).toUpperCase(), color: "bg-brightwing-gray-700 text-brightwing-gray-300" };
}

const EVENT_STYLES: Record<string, { label: string; color: string }> = {
  connect: { label: "CONN", color: "bg-green-500/20 text-green-300" },
  request: { label: "REQ", color: "bg-blue-500/20 text-blue-300" },
  response: { label: "RES", color: "bg-emerald-500/20 text-emerald-300" },
  error: { label: "ERR", color: "bg-red-500/20 text-red-300" },
  session: { label: "SESS", color: "bg-purple-500/20 text-purple-300" },
  disconnect: { label: "DISC", color: "bg-gray-500/20 text-gray-300" },
};

function formatTime(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  } catch {
    return timestamp;
  }
}

export default function ProxyLogViewer({
  server,
  onBack,
}: {
  server: ProxyServer;
  onBack: () => void;
}) {
  const [events, setEvents] = useState<ProxyLogEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let active = true;

    const poll = async () => {
      try {
        const logs = await getProxyLogs(server.server_id);
        if (active) {
          setEvents(logs);
          setError(null);
        }
      } catch (e) {
        if (active) setError(String(e));
      }
    };

    poll();
    const interval = setInterval(poll, 2000);
    return () => {
      active = false;
      clearInterval(interval);
    };
  }, [server.server_id]);

  // Auto-scroll to bottom when new events arrive
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [events, autoScroll]);

  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    // If user scrolled up more than 40px from bottom, disable auto-scroll
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 40);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 mb-4">
        <button
          onClick={onBack}
          className="p-1.5 rounded-md hover:bg-brightwing-gray-700 transition-colors"
        >
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M10.5 19.5 3 12m0 0 7.5-7.5M3 12h18" />
          </svg>
        </button>
        <div className="flex-1">
          <h1 className="text-xl font-semibold">Proxy Logs</h1>
          <p className="text-sm text-brightwing-gray-400">{server.display_name}</p>
        </div>
        <span className="text-xs text-brightwing-gray-500">
          {events.length} event{events.length !== 1 ? "s" : ""}
        </span>
      </div>

      {error && (
        <div className="mb-3 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg text-xs text-red-300">
          {error}
        </div>
      )}

      {/* Log entries */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 bg-brightwing-gray-900 border border-brightwing-gray-700 rounded-lg overflow-y-auto font-mono text-xs"
        style={{ minHeight: "300px", maxHeight: "calc(100vh - 200px)" }}
      >
        {events.length === 0 ? (
          <div className="flex items-center justify-center h-full text-brightwing-gray-600">
            No log events yet. Logs appear when a client connects to this proxy.
          </div>
        ) : (
          <table className="w-full">
            <tbody>
              {events.map((evt, i) => {
                const style = EVENT_STYLES[evt.event_type] || EVENT_STYLES.request;
                return (
                  <tr
                    key={i}
                    className={`border-b border-brightwing-gray-800/50 hover:bg-brightwing-gray-800/30 ${
                      evt.event_type === "error" ? "bg-red-500/5" : ""
                    }`}
                  >
                    <td className="px-3 py-1.5 text-brightwing-gray-500 whitespace-nowrap align-top">
                      {formatTime(evt.timestamp)}
                    </td>
                    <td className="px-1.5 py-1.5 whitespace-nowrap align-top">
                      {(() => {
                        const cl = getClientLabel(evt.client_name);
                        if (!cl) return <span className="inline-block w-6" />;
                        return (
                          <span
                            className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-semibold ${cl.color}`}
                            title={evt.client_name || ""}
                          >
                            {cl.abbr}
                          </span>
                        );
                      })()}
                    </td>
                    <td className="px-2 py-1.5 whitespace-nowrap align-top">
                      <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-semibold ${style.color}`}>
                        {style.label}
                      </span>
                    </td>
                    <td className="px-2 py-1.5 text-brightwing-gray-300 align-top">
                      {evt.method && (
                        <span className="text-brightwing-gray-200">{evt.method}</span>
                      )}
                      {evt.status && (
                        <span className="ml-2 text-brightwing-gray-500">[{evt.status}]</span>
                      )}
                      {evt.detail && (
                        <span className="ml-2 text-brightwing-gray-500">{evt.detail}</span>
                      )}
                      {evt.error_message && (
                        <span className="ml-2 text-red-400">{evt.error_message}</span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Auto-scroll indicator */}
      {!autoScroll && events.length > 0 && (
        <button
          onClick={() => {
            setAutoScroll(true);
            if (scrollRef.current) {
              scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
            }
          }}
          className="mt-2 self-center px-3 py-1 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-full transition-colors text-brightwing-gray-400"
        >
          Scroll to latest
        </button>
      )}
    </div>
  );
}
