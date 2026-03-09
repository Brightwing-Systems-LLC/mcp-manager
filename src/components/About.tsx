import brightwingLogo from "../assets-logo-dark.png";

export default function About() {
  return (
    <div className="max-w-2xl mx-auto">
      {/* Brightwing logo */}
      <div className="flex justify-center mb-8">
        <img src={brightwingLogo} alt="Brightwing Systems" className="h-16" />
      </div>

      <h1 className="text-2xl font-semibold text-center mb-2">
        About MCP Manager
      </h1>
      <p className="text-center text-brightwing-gray-400 mb-8">
        The desktop companion for MCP Scoreboard
      </p>

      {/* What is MCP Manager */}
      <div className="mb-6">
        <h2 className="text-base font-bold mb-3">What is MCP Manager?</h2>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          MCP Manager is the desktop companion app for{" "}
          <a
            href="https://patchworkmcp.com/scoreboard/"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            MCP Scoreboard
          </a>
          . It lets you install, manage, and organize MCP servers across every
          AI tool on your machine from a single interface — powered by the same
          quality scores and server data from the Scoreboard.
        </p>
        <p className="text-sm text-brightwing-gray-400">
          Instead of hand-editing JSON config files across Claude Desktop,
          Cursor, VS Code, and other tools, MCP Manager handles the config
          writing, format translation, and installation tracking for you.
        </p>
      </div>

      {/* Independence Statement */}
      <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5 mb-6">
        <h2 className="text-base font-bold mb-3">Independence Statement</h2>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          MCP Scoreboard and MCP Manager are{" "}
          <strong className="text-brightwing-gray-200">
            independent projects
          </strong>{" "}
          built and operated by{" "}
          <a
            href="https://brightwingsystems.com"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            Brightwing Systems, LLC
          </a>
          .
        </p>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          We are{" "}
          <strong className="text-brightwing-gray-200">
            not affiliated with, endorsed by, or sponsored by
          </strong>{" "}
          the{" "}
          <a
            href="https://linuxfoundation.org"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            Linux Foundation
          </a>
          , the{" "}
          <a
            href="https://aaif.io"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            Agentic AI Foundation (AAIF)
          </a>
          , Anthropic, or any other organization involved in the governance of
          the Model Context Protocol.
        </p>
        <p className="text-sm text-brightwing-gray-400">
          "Model Context Protocol" and "MCP" are trademarks of the Linux
          Foundation. We use these terms solely to describe the technology our
          tools analyze. All trademarks belong to their respective owners.
        </p>
      </div>

      {/* What We Do */}
      <div className="mb-6">
        <h2 className="text-base font-bold mb-3">MCP Scoreboard</h2>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          MCP Scoreboard discovers, analyzes, and scores public MCP servers to
          help developers and teams make informed decisions about the tools they
          integrate into their AI workflows. We evaluate servers across six
          dimensions — schema quality, protocol conformance, reliability,
          documentation and maintenance, security, and agent usability — using a
          fully automated pipeline.
        </p>
        <p className="text-sm text-brightwing-gray-400">
          MCP Manager brings those scores and server data to your desktop,
          making it easy to find quality servers and install them into your AI
          tools without leaving this app.
        </p>
      </div>

      {/* How We're Built */}
      <div className="mb-6">
        <h2 className="text-base font-bold mb-3">How We're Built</h2>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          Our scoring engine is{" "}
          <a
            href="https://github.com/Brightwing-Systems-LLC/mcp-scoring-engine"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            open source
          </a>{" "}
          and available on PyPI. Anyone can run the same analysis we do. Our
          methodology is fully documented — we believe transparency is essential
          for any quality index to be credible.
        </p>
        <p className="text-sm text-brightwing-gray-400">
          We aggregate data from multiple public sources (GitHub, npm, PyPI,
          Docker Hub, and several MCP registries) and apply consistent analysis
          across all servers. Scores are updated automatically as servers
          evolve.
        </p>
      </div>

      {/* Contact */}
      <div className="mb-6">
        <h2 className="text-base font-bold mb-3">Contact</h2>
        <p className="text-sm text-brightwing-gray-400 mb-3">
          MCP Manager is built by{" "}
          <a
            href="https://brightwingsystems.com"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            Brightwing Systems, LLC
          </a>
          . If you have questions, feedback, or concerns about a score, you can
          reach us at{" "}
          <a
            href="mailto:mcpscoreboard@brightwingsystems.com"
            className="text-brightwing-blue hover:underline"
          >
            mcpscoreboard@brightwingsystems.com
          </a>
          .
        </p>
        <p className="text-sm text-brightwing-gray-400">
          If you are a server author, you can find your server on the{" "}
          <a
            href="https://patchworkmcp.com/scoreboard/"
            target="_blank"
            rel="noopener noreferrer"
            className="text-brightwing-blue hover:underline"
          >
            leaderboard
          </a>{" "}
          and claim it to access owner tools, improvement checklists, and score
          alerts.
        </p>
      </div>

      {/* Version */}
      <div className="text-center text-xs text-brightwing-gray-600 mt-8 pb-4">
        MCP Manager v0.3.3
      </div>
    </div>
  );
}
