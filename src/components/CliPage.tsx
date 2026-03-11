import { useEffect, useState } from "react";
import { useStore } from "../store";
import * as tauri from "../lib/tauri";

interface BinaryStatus {
  installed: boolean;
  path: string;
  version: string | null;
}

export default function CliPage() {
  const { showToast, proxyServers } = useStore();
  const [status, setStatus] = useState<BinaryStatus | null>(null);
  const [installing, setInstalling] = useState(false);
  const [pathOnPath, setPathOnPath] = useState<boolean | null>(null);

  const installDir = "~/.local/bin";
  const fullPath = `${installDir}/bw`;

  // Check binary status on mount
  useEffect(() => {
    checkStatus();
  }, []);

  const checkStatus = async () => {
    try {
      const versions = await tauri.getBinaryVersions();
      const bwVersion = versions["bw"] || null;
      setStatus({
        installed: bwVersion !== null,
        path: fullPath,
        version: bwVersion,
      });

      // Check if ~/.local/bin is on PATH
      try {
        const onPath = await tauri.checkCliPath();
        setPathOnPath(onPath);
      } catch {
        setPathOnPath(null);
      }
    } catch {
      setStatus({ installed: false, path: fullPath, version: null });
    }
  };

  const handleInstall = async () => {
    setInstalling(true);
    try {
      const installed = await tauri.distributeBinaries();
      if (installed.includes("bw")) {
        showToast("CLI installed successfully", "success");
        await checkStatus();
      } else {
        showToast("bw binary not found in app bundle", "error");
      }
    } catch (e) {
      showToast(`Install failed: ${e}`, "error");
    }
    setInstalling(false);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    showToast("Copied to clipboard", "success");
  };

  // Pick a sample server for examples
  const sampleServer = proxyServers.length > 0
    ? (proxyServers[0].server_id.includes(" ")
      ? `"${proxyServers[0].server_id}"`
      : proxyServers[0].server_id)
    : '"my-server"';

  return (
    <div className="max-w-2xl">
      {/* Header */}
      <div className="flex items-center gap-3 mb-2">
        <svg className="w-8 h-8 text-brightwing-blue" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path strokeLinecap="round" strokeLinejoin="round" d="m6.75 7.5 3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z" />
        </svg>
        <h1 className="text-2xl font-semibold">Brightwing CLI</h1>
      </div>
      <p className="text-sm text-brightwing-gray-400 mb-6">
        Use your MCP servers directly from the terminal — no AI tool required.
      </p>

      {/* Value proposition */}
      <div className="bg-gradient-to-r from-brightwing-blue/10 to-purple-500/10 border border-brightwing-blue/20 rounded-lg p-5 mb-6">
        <h2 className="text-sm font-semibold text-brightwing-blue mb-2">Why use the CLI?</h2>
        <ul className="space-y-2 text-sm text-brightwing-gray-300">
          <li className="flex items-start gap-2">
            <span className="text-brightwing-blue mt-0.5">{'>'}</span>
            <span><strong className="text-white">Call any MCP tool from your terminal</strong> — search, create, query, and automate without opening an AI assistant</span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-brightwing-blue mt-0.5">{'>'}</span>
            <span><strong className="text-white">Use in scripts and pipelines</strong> — chain MCP tool calls with grep, jq, and other CLI tools for powerful automation</span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-brightwing-blue mt-0.5">{'>'}</span>
            <span><strong className="text-white">Auth handled for you</strong> — the CLI connects through the Brightwing proxy, so OAuth tokens and API keys are managed automatically</span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-brightwing-blue mt-0.5">{'>'}</span>
            <span><strong className="text-white">Zero token cost</strong> — interact with MCP servers directly without consuming LLM context tokens</span>
          </li>
        </ul>
      </div>

      {/* Installation status */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5 mb-6">
        <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">
          Installation
        </h2>

        {status === null ? (
          <div className="flex items-center gap-2 text-sm text-brightwing-gray-400">
            <svg className="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            Checking...
          </div>
        ) : status.installed ? (
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-green-400" />
              <span className="text-sm text-green-300 font-medium">Installed</span>
              {status.version && status.version !== "installed" && (
                <span className="text-xs text-brightwing-gray-500">({status.version})</span>
              )}
            </div>

            <div className="text-xs space-y-1.5">
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Location</span>
                <code className="font-mono text-brightwing-gray-300">{fullPath}</code>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-brightwing-gray-500">Install directory</span>
                <code className="font-mono text-brightwing-gray-300">{installDir}</code>
              </div>
            </div>

            {pathOnPath === false && (
              <div className="bg-amber-500/10 border border-amber-500/20 rounded-md p-3 mt-2">
                <p className="text-xs text-amber-300 font-medium mb-1.5">
                  ~/.local/bin is not on your PATH
                </p>
                <p className="text-xs text-brightwing-gray-400 mb-2">
                  Add this line to your shell profile (~/.zshrc, ~/.bashrc, or ~/.bash_profile):
                </p>
                <div className="flex items-center gap-2">
                  <code className="flex-1 text-xs font-mono text-brightwing-gray-200 bg-brightwing-gray-900 px-2 py-1.5 rounded block">
                    export PATH="$HOME/.local/bin:$PATH"
                  </code>
                  <button
                    onClick={() => copyToClipboard('export PATH="$HOME/.local/bin:$PATH"')}
                    className="p-1.5 text-brightwing-gray-500 hover:text-brightwing-gray-300 transition-colors shrink-0"
                    title="Copy to clipboard"
                  >
                    <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M15.666 3.888A2.25 2.25 0 0 0 13.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 0 1-.75.75H9.75a.75.75 0 0 1-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 0 1-2.25 2.25H6.75A2.25 2.25 0 0 1 4.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 0 1 1.927-.184" />
                    </svg>
                  </button>
                </div>
                <p className="text-xs text-brightwing-gray-500 mt-1.5">
                  Then restart your terminal or run <code className="text-brightwing-gray-400">source ~/.zshrc</code>
                </p>
              </div>
            )}

            {pathOnPath === true && (
              <div className="flex items-center gap-2 text-xs text-green-400 mt-1">
                <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                </svg>
                On your PATH — ready to use
              </div>
            )}

            <button
              onClick={handleInstall}
              disabled={installing}
              className="mt-2 px-3 py-1.5 text-xs bg-brightwing-gray-700 hover:bg-brightwing-gray-600 rounded-md transition-colors disabled:opacity-50"
            >
              {installing ? "Reinstalling..." : "Reinstall / Update"}
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-amber-400" />
              <span className="text-sm text-amber-300 font-medium">Not installed</span>
            </div>
            <p className="text-xs text-brightwing-gray-400">
              The CLI binary will be installed to <code className="text-brightwing-gray-300">{fullPath}</code>
            </p>
            <button
              onClick={handleInstall}
              disabled={installing}
              className="px-4 py-2 text-sm font-medium bg-brightwing-blue hover:bg-brightwing-blue/80 text-white rounded-md transition-colors disabled:opacity-50"
            >
              {installing ? "Installing..." : "Install CLI"}
            </button>
          </div>
        )}
      </div>

      {/* Usage examples */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5 mb-6">
        <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-4">
          Usage
        </h2>
        <div className="space-y-4">
          {[
            { label: "List all servers", cmd: "bw list", desc: "See all connected MCP servers and their tool counts" },
            { label: "List tools on a server", cmd: `bw ${sampleServer}`, desc: "Browse available tools and descriptions" },
            { label: "Get tool help", cmd: `bw ${sampleServer} <tool> --help`, desc: "See parameters, types, and usage for any tool" },
            { label: "Call a tool", cmd: `bw ${sampleServer} <tool> --key value`, desc: "Execute a tool with named parameters" },
            { label: "JSON output", cmd: `bw ${sampleServer} <tool> --key value --json`, desc: "Get raw JSON for piping to jq or other tools" },
          ].map(({ label, cmd, desc }) => (
            <div key={label}>
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs font-medium text-brightwing-gray-300">{label}</span>
                <span className="text-[10px] text-brightwing-gray-500">{desc}</span>
              </div>
              <div className="flex items-center gap-2">
                <code className="flex-1 text-xs font-mono text-brightwing-gray-200 bg-brightwing-gray-900 px-3 py-2 rounded block">
                  $ {cmd}
                </code>
                <button
                  onClick={() => copyToClipboard(cmd)}
                  className="p-1.5 text-brightwing-gray-500 hover:text-brightwing-gray-300 transition-colors shrink-0"
                  title="Copy to clipboard"
                >
                  <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M15.666 3.888A2.25 2.25 0 0 0 13.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 0 1-.75.75H9.75a.75.75 0 0 1-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 0 1-2.25 2.25H6.75A2.25 2.25 0 0 1 4.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 0 1 1.927-.184" />
                  </svg>
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Scripting examples */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5">
        <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-4">
          Scripting Examples
        </h2>
        <div className="space-y-4">
          {[
            {
              label: "Pipe to jq",
              cmd: `bw ${sampleServer} <tool> --json | jq '.content[0].text'`,
            },
            {
              label: "Use in a shell script",
              cmd: `result=$(bw ${sampleServer} <tool> --key value --json)\necho "$result" | jq .`,
            },
            {
              label: "Combine with other CLI tools",
              cmd: `bw ${sampleServer} <tool> --query "search term" | grep "pattern"`,
            },
          ].map(({ label, cmd }) => (
            <div key={label}>
              <div className="text-xs font-medium text-brightwing-gray-300 mb-1">{label}</div>
              <div className="flex items-start gap-2">
                <code className="flex-1 text-xs font-mono text-brightwing-gray-200 bg-brightwing-gray-900 px-3 py-2 rounded block whitespace-pre">
                  $ {cmd}
                </code>
                <button
                  onClick={() => copyToClipboard(cmd)}
                  className="p-1.5 text-brightwing-gray-500 hover:text-brightwing-gray-300 transition-colors shrink-0 mt-1"
                  title="Copy to clipboard"
                >
                  <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M15.666 3.888A2.25 2.25 0 0 0 13.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 0 1-.75.75H9.75a.75.75 0 0 1-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 0 1-2.25 2.25H6.75A2.25 2.25 0 0 1 4.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 0 1 1.927-.184" />
                  </svg>
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
