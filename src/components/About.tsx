import brightwingLogo from "../assets-logo-dark.png";

function ProxyDiagram() {
  return (
    <div className="bg-brightwing-gray-900 rounded-lg p-5 overflow-x-auto">
      <svg viewBox="0 0 720 260" className="w-full max-w-[720px] mx-auto" style={{ minWidth: 500 }}>
        {/* AI Tools column */}
        {[
          { label: "Claude Desktop", y: 20 },
          { label: "Cursor", y: 70 },
          { label: "Claude Code", y: 120 },
          { label: "Gemini CLI", y: 170 },
          { label: "Codex", y: 220 },
        ].map((tool) => (
          <g key={tool.label}>
            <rect x="10" y={tool.y} width="120" height="36" rx="6" fill="#1e293b" stroke="#334155" strokeWidth="1" />
            <text x="70" y={tool.y + 22} textAnchor="middle" fill="#94a3b8" fontSize="11" fontFamily="monospace">{tool.label}</text>
          </g>
        ))}
        <text x="70" y="270" textAnchor="middle" fill="#64748b" fontSize="10">AI Tools</text>

        {/* Arrows: tools → proxy */}
        {[38, 88, 138, 188, 238].map((y) => (
          <g key={y}>
            <line x1="130" y1={y} x2="218" y2={130} stroke="#475569" strokeWidth="1" strokeDasharray="4 3" />
            <text x="174" y={y < 130 ? y + 2 : y - 2} textAnchor="middle" fill="#475569" fontSize="8">stdio</text>
          </g>
        ))}

        {/* Brightwing Proxy — central box */}
        <rect x="218" y="80" width="160" height="100" rx="10" fill="#0f172a" stroke="#3b82f6" strokeWidth="2" />
        <text x="298" y="108" textAnchor="middle" fill="#3b82f6" fontSize="13" fontWeight="bold" fontFamily="sans-serif">Brightwing Proxy</text>
        <line x1="238" y1="118" x2="358" y2="118" stroke="#1e3a5f" strokeWidth="1" />
        <text x="298" y="136" textAnchor="middle" fill="#64748b" fontSize="9">Auth injection</text>
        <text x="298" y="150" textAnchor="middle" fill="#64748b" fontSize="9">Tool filtering</text>
        <text x="298" y="164" textAnchor="middle" fill="#64748b" fontSize="9">Request logging</text>

        {/* CLI arrow into proxy */}
        <rect x="218" y="200" width="80" height="30" rx="6" fill="#1e293b" stroke="#334155" strokeWidth="1" />
        <text x="258" y="219" textAnchor="middle" fill="#a78bfa" fontSize="11" fontFamily="monospace">bw CLI</text>
        <line x1="258" y1="200" x2="278" y2="180" stroke="#7c3aed" strokeWidth="1.5" strokeDasharray="4 3" />

        {/* Arrows: proxy → servers */}
        {[38, 88, 138, 188, 238].map((y) => (
          <line key={`r-${y}`} x1="378" y1={130} x2="480" y2={y} stroke="#475569" strokeWidth="1" strokeDasharray="4 3" />
        ))}
        <text x="430" y="72" textAnchor="middle" fill="#475569" fontSize="8">HTTPS</text>
        <text x="430" y="188" textAnchor="middle" fill="#475569" fontSize="8">stdio</text>

        {/* MCP Servers column */}
        {[
          { label: "GitHub MCP", y: 20 },
          { label: "Sentry MCP", y: 70 },
          { label: "Stripe MCP", y: 120 },
          { label: "Custom Server", y: 170 },
          { label: "Local Server", y: 220 },
        ].map((srv) => (
          <g key={srv.label}>
            <rect x="480" y={srv.y} width="120" height="36" rx="6" fill="#1e293b" stroke="#334155" strokeWidth="1" />
            <text x="540" y={srv.y + 22} textAnchor="middle" fill="#94a3b8" fontSize="11" fontFamily="monospace">{srv.label}</text>
          </g>
        ))}
        <text x="540" y="270" textAnchor="middle" fill="#64748b" fontSize="10">MCP Servers</text>

        {/* Auth + tokens badges */}
        <rect x="620" y="30" width="90" height="24" rx="4" fill="#166534" fillOpacity="0.3" stroke="#22c55e" strokeWidth="1" strokeOpacity="0.4" />
        <text x="665" y="46" textAnchor="middle" fill="#4ade80" fontSize="9">OAuth tokens</text>

        <rect x="620" y="64" width="90" height="24" rx="4" fill="#92400e" fillOpacity="0.2" stroke="#f59e0b" strokeWidth="1" strokeOpacity="0.4" />
        <text x="665" y="80" textAnchor="middle" fill="#fbbf24" fontSize="9">API keys</text>

        <rect x="620" y="98" width="90" height="24" rx="4" fill="#1e3a5f" fillOpacity="0.3" stroke="#3b82f6" strokeWidth="1" strokeOpacity="0.4" />
        <text x="665" y="114" textAnchor="middle" fill="#60a5fa" fontSize="9">Tool filters</text>
      </svg>
    </div>
  );
}

export default function About() {
  return (
    <div className="max-w-2xl mx-auto">
      {/* Brightwing logo */}
      <div className="flex justify-center mb-6">
        <img src={brightwingLogo} alt="Brightwing Systems" className="h-16" />
      </div>

      <h1 className="text-2xl font-semibold text-center mb-2">
        MCP Manager
      </h1>
      <p className="text-center text-brightwing-gray-400 mb-8">
        One app to install, authenticate, proxy, filter, and manage every MCP server across all your AI tools.
      </p>

      {/* Key capabilities */}
      <div className="grid grid-cols-2 gap-3 mb-8">
        {[
          {
            icon: (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75 11.25 15 15 9.75m-3-7.036A11.959 11.959 0 0 1 3.598 6 11.99 11.99 0 0 0 3 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285Z" />
              </svg>
            ),
            title: "Auth Proxy",
            desc: "OAuth and API key injection so your AI tools never see raw credentials",
            color: "text-green-400",
          },
          {
            icon: (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M16.5 10.5V6.75a4.5 4.5 0 1 0-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 0 0 2.25-2.25v-6.75a2.25 2.25 0 0 0-2.25-2.25H6.75a2.25 2.25 0 0 0-2.25 2.25v6.75a2.25 2.25 0 0 0 2.25 2.25Z" />
              </svg>
            ),
            title: "Encrypted Vault",
            desc: "API keys stored in a Stronghold encrypted vault — never in plaintext config files",
            color: "text-rose-400",
          },
          {
            icon: (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M12 3c2.755 0 5.455.232 8.083.678.533.09.917.556.917 1.096v1.044a2.25 2.25 0 0 1-.659 1.591l-5.432 5.432a2.25 2.25 0 0 0-.659 1.591v2.927a2.25 2.25 0 0 1-1.244 2.013L9.75 21v-6.568a2.25 2.25 0 0 0-.659-1.591L3.659 7.409A2.25 2.25 0 0 1 3 5.818V4.774c0-.54.384-1.006.917-1.096A48.32 48.32 0 0 1 12 3Z" />
              </svg>
            ),
            title: "Tool Filtering",
            desc: "Control which tools each AI app sees — cut token bloat per app",
            color: "text-blue-400",
          },
          {
            icon: (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3" />
              </svg>
            ),
            title: "One-Click Install",
            desc: "Install servers into Claude, Cursor, VS Code, Codex, and more simultaneously",
            color: "text-purple-400",
          },
          {
            icon: (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="m6.75 7.5 3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z" />
              </svg>
            ),
            title: "CLI Access",
            desc: "Call any MCP tool from your terminal — script, pipe, automate",
            color: "text-amber-400",
          },
        ].map((cap) => (
          <div key={cap.title} className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-4">
            <div className={`${cap.color} mb-2`}>{cap.icon}</div>
            <h3 className="text-sm font-semibold text-white mb-1">{cap.title}</h3>
            <p className="text-xs text-brightwing-gray-400">{cap.desc}</p>
          </div>
        ))}
      </div>

      {/* How the proxy works */}
      <div className="mb-8">
        <h2 className="text-base font-bold mb-2">How It Works</h2>
        <p className="text-sm text-brightwing-gray-400 mb-4">
          Every MCP server you install is routed through the Brightwing proxy. Your AI tools connect to the proxy over local stdio, and the proxy handles authentication, tool filtering, and request forwarding to the upstream server. Credentials are stored in an encrypted Stronghold vault — never in plaintext config files.
        </p>
        <ProxyDiagram />
        <p className="text-xs text-brightwing-gray-500 mt-2 text-center">
          All AI tools connect through a single local proxy that manages auth, filtering, and logging.
        </p>
      </div>

      {/* The problem we solve */}
      <div className="mb-8">
        <h2 className="text-base font-bold mb-3">The Problem</h2>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          MCP servers are powerful, but managing them is painful. Each AI tool has its own config format, its own file location, and its own quirks. Adding a server means hand-editing JSON, managing OAuth flows, and copy-pasting API keys into plaintext config files that any process on your machine can read. Multiply that across 3-5 AI tools and dozens of servers, and it becomes a security and maintenance nightmare.
        </p>
        <p className="text-sm text-brightwing-gray-400">
          Worse, every tool gets the full tool list from every server — even tools you never use. A server with 89 tools dumps all of them into context, eating thousands of tokens before your AI assistant has even started thinking about your request.
        </p>
      </div>

      {/* MCP Scoreboard */}
      <div className="bg-gradient-to-r from-brightwing-blue/10 to-purple-500/10 border border-brightwing-blue/20 rounded-lg p-5 mb-8">
        <h2 className="text-base font-bold mb-3">Powered by MCP Scoreboard</h2>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          The built-in search is powered by{" "}
          <a
            href="https://patchworkmcp.com/scoreboard/"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            MCP Scoreboard
          </a>
          , which discovers, analyzes, and scores public MCP servers across six dimensions: schema quality, protocol conformance, reliability, documentation, security, and agent usability.
        </p>
        <p className="text-sm text-brightwing-gray-400">
          When you search for a server, you see quality scores and grades alongside the results — so you can make informed decisions about what you're integrating into your workflow. The{" "}
          <a
            href="https://github.com/Brightwing-Systems-LLC/mcp-scoring-engine"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            scoring engine is open source
          </a>
          {" "}and the methodology is fully documented.
        </p>
      </div>

      {/* Independence Statement */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5 mb-8">
        <h2 className="text-sm font-medium text-brightwing-gray-400 uppercase tracking-wider mb-3">Independence Statement</h2>
        <p className="text-xs text-brightwing-gray-400 mb-2">
          MCP Scoreboard and MCP Manager are{" "}
          <strong className="text-brightwing-gray-200">independent projects</strong>{" "}
          built by{" "}
          <a href="https://brightwingsystems.com" target="_blank" rel="noopener noreferrer" className="text-brightwing-blue hover:underline">
            Brightwing Systems, LLC
          </a>
          . We are not affiliated with, endorsed by, or sponsored by the{" "}
          <a href="https://linuxfoundation.org" target="_blank" rel="noopener noreferrer" className="text-brightwing-blue hover:underline">Linux Foundation</a>,{" "}
          <a href="https://aaif.io" target="_blank" rel="noopener noreferrer" className="text-brightwing-blue hover:underline">AAIF</a>,{" "}
          Anthropic, or any other organization involved in the governance of the Model Context Protocol.
        </p>
        <p className="text-xs text-brightwing-gray-500">
          "Model Context Protocol" and "MCP" are trademarks of the Linux Foundation. All trademarks belong to their respective owners.
        </p>
      </div>

      {/* Contact */}
      <div className="mb-6">
        <h2 className="text-base font-bold mb-3">Contact</h2>
        <p className="text-sm text-brightwing-gray-400">
          Built by{" "}
          <a href="https://brightwingsystems.com" target="_blank" rel="noopener noreferrer" className="text-brightwing-blue hover:underline">
            Brightwing Systems, LLC
          </a>
          . Questions, feedback, or score disputes:{" "}
          <a href="mailto:mcpscoreboard@brightwingsystems.com" className="text-brightwing-blue hover:underline">
            mcpscoreboard@brightwingsystems.com
          </a>
          . Server authors can claim their listing on the{" "}
          <a href="https://patchworkmcp.com/scoreboard/" target="_blank" rel="noopener noreferrer" className="text-brightwing-blue hover:underline">
            leaderboard
          </a>
          .
        </p>
      </div>

      {/* License */}
      <div className="mb-6">
        <h2 className="text-base font-bold mb-3">License</h2>
        <p className="text-sm text-brightwing-gray-400">
          This project is released under the{" "}
          <a
            href="https://github.com/Brightwing-Systems-LLC/mcp-manager/blob/main/LICENSE"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            MIT License
          </a>
          . Copyright &copy; 2025–2026 Brightwing Systems, LLC.
        </p>
      </div>

      {/* Version */}
      <div className="text-center text-xs text-brightwing-gray-600 mt-8 pb-4">
        MCP Manager v0.3.14
      </div>
    </div>
  );
}
