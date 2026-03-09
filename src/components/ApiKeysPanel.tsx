import { useEffect, useState } from "react";
import { useStore } from "../store";
import type { ProxyApiKey } from "../lib/types";
import * as tauri from "../lib/tauri";

interface EnvEntry {
  key: string;
  value: string;
}

export default function ApiKeysPanel() {
  const { proxyServers, proxyServersLoading, refreshProxyServers, showToast } = useStore();
  const [apiKeys, setApiKeys] = useState<Record<string, ProxyApiKey>>({});
  const [editingServer, setEditingServer] = useState<string | null>(null);
  const [envEntries, setEnvEntries] = useState<EnvEntry[]>([{ key: "", value: "" }]);
  const [saving, setSaving] = useState(false);
  const [showValues, setShowValues] = useState<Set<string>>(new Set());

  useEffect(() => {
    refreshProxyServers();
    loadAllKeys();
  }, [refreshProxyServers]);

  const loadAllKeys = async () => {
    try {
      const keys = await tauri.getAllApiKeys();
      const byServer: Record<string, ProxyApiKey> = {};
      for (const k of keys) {
        byServer[k.server_id] = k;
      }
      setApiKeys(byServer);
    } catch (e) {
      console.error("Failed to load API keys:", e);
    }
  };

  const apiKeyServers = proxyServers.filter((s) => s.auth_type === "api_key");

  const startEditing = (serverId: string) => {
    const existing = apiKeys[serverId];
    if (existing && Object.keys(existing.env).length > 0) {
      const entries = Object.entries(existing.env).map(([key, value]) => ({ key, value }));
      entries.push({ key: "", value: "" });
      setEnvEntries(entries);
    } else {
      setEnvEntries([{ key: "", value: "" }]);
    }
    setEditingServer(serverId);
  };

  const cancelEditing = () => {
    setEditingServer(null);
    setEnvEntries([{ key: "", value: "" }]);
  };

  const updateEntry = (index: number, field: "key" | "value", val: string) => {
    setEnvEntries((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], [field]: val };
      // Auto-add a new empty row if the last row has content
      if (index === next.length - 1 && (next[index].key || next[index].value)) {
        next.push({ key: "", value: "" });
      }
      return next;
    });
  };

  const removeEntry = (index: number) => {
    setEnvEntries((prev) => {
      if (prev.length <= 1) return [{ key: "", value: "" }];
      return prev.filter((_, i) => i !== index);
    });
  };

  const handleSave = async () => {
    if (!editingServer) return;

    const env: Record<string, string> = {};
    for (const entry of envEntries) {
      const k = entry.key.trim();
      const v = entry.value.trim();
      if (k && v) {
        env[k] = v;
      }
    }

    if (Object.keys(env).length === 0) {
      showToast("At least one env var is required", "error");
      return;
    }

    setSaving(true);
    try {
      await tauri.storeApiKey(editingServer, env);
      showToast("API key saved", "success");
      setEditingServer(null);
      setEnvEntries([{ key: "", value: "" }]);
      await loadAllKeys();
    } catch (e) {
      showToast(`Failed to save: ${e}`, "error");
    }
    setSaving(false);
  };

  const handleDelete = async (serverId: string) => {
    try {
      await tauri.deleteApiKey(serverId);
      showToast("API key removed", "success");
      await loadAllKeys();
    } catch (e) {
      showToast(`Failed to delete: ${e}`, "error");
    }
  };

  const toggleShowValue = (serverId: string) => {
    setShowValues((prev) => {
      const next = new Set(prev);
      if (next.has(serverId)) {
        next.delete(serverId);
      } else {
        next.add(serverId);
      }
      return next;
    });
  };

  const maskValue = (value: string) => {
    if (value.length <= 4) return "••••••••";
    return value.slice(0, 4) + "••••••••";
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">API Keys</h1>
          <p className="text-sm text-brightwing-gray-400 mt-1">
            Manage credentials for proxy servers with API key authentication.
          </p>
        </div>
      </div>

      {proxyServersLoading ? (
        <p className="text-brightwing-gray-500 text-sm">Loading...</p>
      ) : apiKeyServers.length === 0 ? (
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-8 text-center">
          <p className="text-brightwing-gray-400">No proxy servers with API key auth.</p>
          <p className="text-brightwing-gray-500 text-sm mt-1">
            Register a proxy server with "API Key" auth type to manage its credentials here.
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {apiKeyServers.map((server) => {
            const stored = apiKeys[server.server_id];
            const isEditing = editingServer === server.server_id;
            const hasKey = stored && Object.keys(stored.env).length > 0;
            const isRevealed = showValues.has(server.server_id);

            return (
              <div
                key={server.server_id}
                className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg overflow-hidden"
              >
                {/* Header */}
                <div className="flex items-center justify-between px-4 py-3">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="font-medium">{server.display_name}</span>
                    <span className="text-xs font-mono text-brightwing-gray-500">
                      {server.server_id}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    {hasKey ? (
                      <span className="text-xs px-2 py-0.5 rounded-full bg-green-500/20 text-green-300">
                        Configured
                      </span>
                    ) : (
                      <span className="text-xs px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-300">
                        Not Set
                      </span>
                    )}
                  </div>
                </div>

                {/* Stored key display (when not editing) */}
                {hasKey && !isEditing && (
                  <div className="border-t border-brightwing-gray-700 px-4 py-3">
                    <div className="space-y-1.5">
                      {Object.entries(stored.env).map(([envKey, envVal]) => (
                        <div key={envKey} className="flex items-center gap-2 text-xs">
                          <span className="font-mono text-brightwing-gray-300 min-w-[140px]">
                            {envKey}
                          </span>
                          <span className="font-mono text-brightwing-gray-500">
                            {isRevealed ? envVal : maskValue(envVal)}
                          </span>
                        </div>
                      ))}
                    </div>
                    <div className="flex gap-2 mt-3">
                      <button
                        onClick={() => toggleShowValue(server.server_id)}
                        className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors"
                      >
                        {isRevealed ? "Hide" : "Reveal"}
                      </button>
                      <button
                        onClick={() => startEditing(server.server_id)}
                        className="px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors"
                      >
                        Edit
                      </button>
                      <button
                        onClick={() => handleDelete(server.server_id)}
                        className="px-3 py-1.5 text-xs text-red-400 hover:bg-red-500/10 rounded-md transition-colors"
                      >
                        Remove
                      </button>
                    </div>
                    <p className="text-xs text-brightwing-gray-600 mt-2">
                      Updated {new Date(stored.updated_at + "Z").toLocaleDateString()}
                    </p>
                  </div>
                )}

                {/* No key, not editing — show add button */}
                {!hasKey && !isEditing && (
                  <div className="border-t border-brightwing-gray-700 px-4 py-3">
                    <button
                      onClick={() => startEditing(server.server_id)}
                      className="px-3 py-1.5 text-xs bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors"
                    >
                      Add API Key
                    </button>
                  </div>
                )}

                {/* Editing form */}
                {isEditing && (
                  <div className="border-t border-brightwing-gray-700 px-4 py-3 space-y-3">
                    <p className="text-xs text-brightwing-gray-400">
                      Enter environment variables that the proxy will inject when spawning the
                      upstream server.
                    </p>
                    <div className="space-y-2">
                      {envEntries.map((entry, i) => (
                        <div key={i} className="flex items-center gap-2">
                          <input
                            type="text"
                            value={entry.key}
                            onChange={(e) => updateEntry(i, "key", e.target.value)}
                            placeholder="ENV_VAR_NAME"
                            className="flex-1 px-2 py-1.5 text-xs font-mono bg-brightwing-gray-900 border border-brightwing-gray-700 rounded focus:outline-none focus:ring-1 focus:ring-brightwing-blue/50 placeholder-brightwing-gray-600"
                          />
                          <span className="text-brightwing-gray-600">=</span>
                          <input
                            type="password"
                            value={entry.value}
                            onChange={(e) => updateEntry(i, "value", e.target.value)}
                            placeholder="value"
                            className="flex-[2] px-2 py-1.5 text-xs font-mono bg-brightwing-gray-900 border border-brightwing-gray-700 rounded focus:outline-none focus:ring-1 focus:ring-brightwing-blue/50 placeholder-brightwing-gray-600"
                          />
                          {envEntries.length > 1 && (entry.key || entry.value) && (
                            <button
                              onClick={() => removeEntry(i)}
                              className="p-1 text-brightwing-gray-500 hover:text-red-400 transition-colors"
                            >
                              <svg
                                className="w-4 h-4"
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
                          )}
                        </div>
                      ))}
                    </div>
                    <div className="flex gap-2">
                      <button
                        onClick={handleSave}
                        disabled={saving}
                        className="px-4 py-1.5 text-xs font-medium bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors disabled:opacity-50"
                      >
                        {saving ? "Saving..." : "Save"}
                      </button>
                      <button
                        onClick={cancelEditing}
                        className="px-4 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors"
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
