# MCP Server Installation Landscape: Every Desktop LLM Tool
## A Comprehensive Analysis for Building a Universal MCP Installer
**Last Updated: March 6, 2026**

---

## Executive Summary

The Model Context Protocol (MCP), originally released by Anthropic in November 2024, has become the universal standard for connecting AI desktop tools to external data sources and capabilities. As of March 2026, MCP is governed by the Agentic AI Foundation (under the Linux Foundation), co-founded by Anthropic, Block, and OpenAI in December 2025.

Every major AI desktop tool now supports MCP in some form, but the **installation and configuration mechanisms vary significantly** across tools. This document catalogs every method, every config path, every format difference, and every edge case you'd need to handle to build a universal MCP server installer.

---

## The Four Installation Mechanisms

### 1. Manual JSON Config File Editing

The original and most universal method. Every tool has a JSON config file at a specific filesystem path. You add server entries by hand (or programmatically). The JSON schema is *nearly* identical across tools, with important exceptions noted below.

### 2. Remote Connectors / Apps

Cloud-hosted MCP servers connected through a UI flow, typically with OAuth authentication. This is how non-technical users add integrations. Claude calls these "Connectors," OpenAI calls them "Apps" (renamed from "Connectors" in December 2025).

### 3. MCPB Bundles (formerly DXT)

A packaging format (`.mcpb`) that bundles an entire MCP server — code, dependencies, manifest — into a single installable file. Think Chrome extensions (`.crx`) or VS Code extensions (`.vsix`) but for MCP servers. Originally developed by Anthropic as "Desktop Extensions" (`.dxt`), now open-sourced under the MCP project.

### 4. Deep Links, CLI Commands, and One-Click Buttons

Convenience shortcuts that write to the JSON config for you. Includes "Add to Cursor" buttons on documentation pages, CLI commands like `claude mcp add`, `gemini mcp add`, and `q mcp add`, and VS Code deep link URIs.

---

## Tool-by-Tool Breakdown

### Claude Desktop

**Category:** Chat application (macOS, Windows)
**MCP Support:** Full — local stdio, remote HTTP/SSE, MCPB bundles, connectors

**Config File Paths:**
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`

**Config Format:**
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "some-package"],
      "env": {
        "API_KEY": "value"
      }
    }
  }
}
```

**Key Differentiators:**
- **MCPB/DXT support:** Native. Click `.mcpb` file → installation dialog. Built-in extension directory (Settings > Extensions > Browse).
- **Built-in Node.js:** Ships its own Node runtime, so users don't need Node installed for MCPB bundles.
- **Connectors:** UI-based remote MCP connections (Settings > Connectors or "+" button > Connectors). Curated library includes Google Calendar, Gmail, Slack, etc. Custom connectors via URL + OAuth.
- **Secret storage:** MCPB extensions store sensitive config in OS keychain (macOS Keychain, Windows Credential Manager).
- **Enterprise controls:** Group Policy (Windows) and MDM (macOS) management, blocklists, allowlists for extensions.
- **Auto-updates:** Extensions from the official directory update automatically.

**Installation Methods:**
1. Edit `claude_desktop_config.json` directly
2. Install `.mcpb` file (Settings > Extensions > Install Extension)
3. Browse built-in extension directory
4. Add remote connector via UI
5. Use `claude mcp add` via Claude Code CLI (writes to shared config)

---

### Claude Code

**Category:** CLI coding agent
**MCP Support:** Full — local stdio, remote HTTP, acts as both MCP client and server

**Config Mechanism:** CLI commands with scope control

**Commands:**
```bash
# Add a server
claude mcp add server-name -- command arg1 arg2

# Add with JSON
claude mcp add-json server-name '{"type":"stdio","command":"npx","args":["-y","package"]}'

# Add remote HTTP server
claude mcp add --transport http server-name https://url

# Scopes
--scope local    # Current project only (default)
--scope project  # Shared .mcp.json file in project root
--scope user     # Available across all projects
```

**Key Differentiators:**
- **Three scopes:** local (personal, current project), project (shared `.mcp.json` in repo root), user (global)
- **Dual nature:** Can simultaneously be an MCP client (consuming other servers) AND an MCP server (exposing its tools to Claude Desktop, Cursor, etc. via `claude mcp serve`)
- **MCPB support:** Yes, can install `.mcpb` bundles
- **No JSON file to edit manually** — everything goes through the CLI

---

### ChatGPT Desktop / Web

**Category:** Chat application (macOS, Windows, web)
**MCP Support:** Remote only — HTTP/SSE. No local stdio support natively.

**Connection Method:** Settings → Connectors → Create (or Settings → Apps & Connectors)

**Key Differentiators:**
- **Remote-only MCP:** ChatGPT does NOT spawn local processes. All MCP servers must be reachable over HTTPS. For local servers, you need an intermediary like Docker MCP Toolkit, ngrok, or Cloudflare Tunnel.
- **Developer Mode required:** MCP tools in regular chat require Developer Mode, available on Pro, Team, Enterprise, and Edu plans. Plus users also have access.
- **Deep Research mode:** Works without Developer Mode but requires servers to implement specific `search` and `fetch` tools conforming to OpenAI's compatibility schema.
- **"Apps" (formerly Connectors):** Renamed December 17, 2025. These are the MCP integration points. Available via Settings → Apps & Connectors.
- **Write actions:** Supported in Developer Mode with confirmation modals for safety.
- **OAuth:** Primary authentication method. Dynamic client registration supported.
- **No config file:** Everything is configured through the UI. No JSON file to edit.
- **Docker MCP Toolkit:** The primary workaround for local servers — Docker Desktop runs an MCP Gateway container that ChatGPT connects to as a remote URL.

**This is NOT the same as Custom GPTs:**
Custom GPTs use OpenAI's "Actions" system (OpenAPI/Swagger specs calling REST endpoints). They are a completely different, older, proprietary integration mechanism — not MCP at all. Custom GPTs are gradually being complemented by the Apps system but still exist separately.

---

### Cursor

**Category:** AI code editor (VS Code fork)
**MCP Support:** Full — stdio, Streamable HTTP, SSE

**Config File Paths:**
- Global: `~/.cursor/mcp.json`
- Project-scoped: `.cursor/mcp.json` (in project root)

**Config Format:**
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "some-package"],
      "env": { "API_KEY": "value" }
    }
  }
}
```

**Key Differentiators:**
- **"Add to Cursor" buttons:** Many MCP server docs have one-click buttons that open a pre-filled dialog in Cursor.
- **Settings UI:** Cursor Settings > Tools & MCP > New MCP Server.
- **~40 tool limit:** Past ~40 active tools across all servers, the agent degrades at picking the right tool.
- **Resources support:** Shipped in v1.6 (September 2025) — contextual data like schemas and file contents.
- **Elicitation:** Landed in v1.5 (August 2025) — servers can ask for structured user input mid-execution.
- **Security note:** Multiple critical CVEs were published in 2025 (MCPoison, etc.). Keep Cursor current, pin package versions, audit servers.

---

### Windsurf (by Codeium)

**Category:** AI code editor
**MCP Support:** Full — stdio, Streamable HTTP, SSE

**Config File Path:**
- `~/.codeium/windsurf/mcp_config.json`

**Config Format:**
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "some-package"],
      "env": { "API_KEY": "value" }
    }
  }
}
```

**Key Differentiators:**
- **Per-tool toggling:** Can enable/disable individual tools within a server, not just entire servers. Useful for the ~100 tool budget.
- **Built-in marketplace:** MCPs icon in Cascade panel > browse/search available servers.
- **Admin whitelist:** Teams and Enterprise plans support regex pattern matching against server IDs to restrict which servers developers can install.
- **Some tools require hardcoded tokens:** Environment variable expansion doesn't work the same as other tools — some servers need tokens directly in the config.
- **Different path from Cursor:** `~/.codeium/windsurf/mcp_config.json` vs `~/.cursor/mcp.json`. Editing one does NOT affect the other.

---

### VS Code (GitHub Copilot)

**Category:** Code editor with AI assistant
**MCP Support:** Full — stdio, Streamable HTTP, SSE (requires Copilot Chat extension)

**Config File Paths:**
- Workspace: `.vscode/mcp.json`
- User settings: In `settings.json`

**Config Format (DIFFERENT from others):**
```json
{
  "servers": {
    "server-name": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "some-package"],
      "env": { "API_KEY": "value" }
    }
  }
}
```

**Key Differentiators:**
- **Uses `"servers"` not `"mcpServers"`** — This is a critical difference. Your installer must account for this.
- **`inputs` array for secrets:** VS Code supports prompted secret entry via an `inputs` mechanism:
  ```json
  {
    "inputs": [
      {
        "type": "promptString",
        "id": "api_key",
        "description": "API Key",
        "password": true
      }
    ]
  }
  ```
- **Agent Mode required:** Must be in Agent Mode (not just Copilot Chat) for MCP tools to work.
- **Deep links:** Some servers offer one-click install via VS Code deep link URIs.
- **Requires `type` field:** Must specify `"type": "stdio"` or `"type": "http"` in the config, unlike most other tools where stdio is assumed.

---

### Visual Studio (not VS Code)

**Category:** Full IDE (Windows)
**MCP Support:** Full — stdio, SSE, Streamable HTTP (version 17.14+ or Visual Studio 2026)

**Config File Paths:**
- Solution-level: `<SOLUTIONDIR>\.mcp.json`
- User-level: `%USERPROFILE%\.mcp.json`

**Config Format:** Same as VS Code — uses `"servers"` key.

**Key Differentiators:**
- **One-click install from GitHub registry:** Extensions > MCP Registries in Visual Studio menu.
- **Green "+" button in chat:** Direct UI for adding servers.
- **MCP Server Manager:** Built-in UI for browsing and installing from the GitHub MCP server registry.

---

### JetBrains IDEs (IntelliJ, PyCharm, WebStorm, etc.)

**Category:** Full IDE family
**MCP Support:** Full — stdio, Streamable HTTP, SSE (version 2025.2+)

**Config Method:** GUI-based (Settings > Tools > AI Assistant > Model Context Protocol)

**Key Differentiators:**
- **Import from Claude:** Can directly import MCP server configurations from Claude Desktop's config file.
- **Auto-configure for external clients:** The IDE itself ships an MCP *server* (since 2025.2) that external clients (Claude Desktop, Cursor, etc.) can connect to. Has "Auto-Configure" buttons for each client.
- **JSON or As JSON dialog:** When adding a server, can paste JSON snippets in standard `mcpServers` format.
- **Agent Client Protocol (ACP):** Supports ACP for connecting external agents (Kiro, Gemini CLI) as AI providers within the IDE.
- **No direct file path to edit** — configuration is stored internally and managed through the GUI. But can export/import JSON configs.

---

### Gemini CLI

**Category:** Terminal-based AI agent
**MCP Support:** Full — stdio, Streamable HTTP, SSE with OAuth 2.0

**Config File Paths:**
- User: `~/.gemini/settings.json`
- Project: `.gemini/settings.json`

**Config Format:**
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "some-package"],
      "env": { "API_KEY": "value" }
    }
  }
}
```

For remote servers:
```json
{
  "mcpServers": {
    "remote-server": {
      "httpUrl": "https://example.com/mcp"
    }
  }
}
```

**Key Differentiators:**
- **Uses `httpUrl` for remote servers** — not `url` like other tools. Important format difference.
- **CLI add command:** `gemini mcp add --transport http name url`
- **Built-in OAuth 2.0:** Supports dynamic discovery, Google credentials, and service account impersonation.
- **Automatic env var redaction:** Sensitive environment variables (matching patterns like `*TOKEN*`, `*SECRET*`, `*KEY*`) are automatically redacted from the base environment to prevent leakage to MCP servers. Must explicitly declare env vars in config.
- **`includeTools` / `excludeTools`:** Can filter which tools from a server are exposed to the model.
- **Rich content support:** MCP tool responses can include images and other binary content, not just text.

---

### Gemini Code Assist (in VS Code / JetBrains)

**Category:** IDE AI assistant plugin
**MCP Support:** Via agent mode — local and remote servers

**Config Method:**
- VS Code: Gemini settings JSON file (separate from VS Code's own MCP config)
- JetBrains: Not yet supported (as of late 2025 docs)

**Key Differentiator:** Cannot use Command Palette to install MCP servers — must edit config directly.

---

### Zed

**Category:** Code editor
**MCP Support:** Partial — stdio only. **No remote server support yet** (Streamable HTTP and SSE are not supported as of early 2026, with community PRs in progress).

**Config Format (DIFFERENT from others):**
```json
{
  "context_servers": {
    "server-name": {
      "source": "custom",
      "command": "some-command",
      "args": ["arg1", "arg2"],
      "env": {}
    }
  }
}
```

Placed in Zed's settings file (Preferences > Settings).

**Key Differentiators:**
- **Uses `"context_servers"` not `"mcpServers"`** — Another critical format difference for your installer.
- **Extension-based MCP:** Zed's primary MCP distribution mechanism is through its own extension system (Rust/WASM-based). Extensions are registered in `extension.toml` and distributed as Zed extensions.
- **Stdio only:** As of early 2026, remote MCP servers (HTTP/SSE) are not supported. This is a known gap the community is actively working to close.
- **Agent Client Protocol (ACP):** Supports external agents (Kiro, Claude Agent, Codex) via ACP, which is a separate protocol from MCP but complementary.
- **Tool permissions:** Granular per-tool permissions using `mcp:<server>:<tool_name>` format.

---

### Amazon Q Developer (IDE + CLI)

**Category:** AI developer assistant (VS Code, JetBrains, CLI)
**MCP Support:** Full — stdio, HTTP with OAuth

**Config File Paths:**
- **IDE Global:** `~/.aws/amazonq/default.json` (or `~/.aws/amazonq/agents/default.json` for agent configs)
- **IDE Local:** `.amazonq/default.json` (or `.amazonq/agents/default.json`)
- **Legacy:** Also reads `mcp.json` files (toggle via `useLegacyMcpJson` field)
- **CLI:** Uses `q mcp add` commands or agent configuration files

**Config Format:**
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "some-package"],
      "env": { "API_KEY": "value" }
    }
  }
}
```

**Key Differentiators:**
- **MCP Governance / Registry:** Enterprise admins can define an "MCP Registry URL" — an HTTPS-hosted JSON file listing approved servers. Q Developer fetches this at startup and every 24 hours. Servers not in the registry are terminated.
- **GUI-based config:** MCP configuration UI accessed via the Q Developer panel's tools icon.
- **Workspace precedence:** Workspace-level configs override global.
- **Remote server OAuth:** Supports OAuth with automatic browser-based auth flow.

---

### Kiro (by AWS)

**Category:** AI IDE
**MCP Support:** Full — stdio, HTTP with OAuth

**Config File Paths:**
- Global: `~/.kiro/settings/mcp.json`
- Workspace: `.kiro/settings/mcp.json`

**Config Format:**
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "some-package"],
      "env": {
        "API_KEY": "${API_KEY}"
      },
      "disabled": false,
      "autoApprove": ["tool1"],
      "disabledTools": ["tool2"]
    }
  }
}
```

**Key Differentiators:**
- **Environment variable expansion:** Supports `${VAR_NAME}` syntax in env values.
- **`autoApprove` and `disabledTools`:** Built into the config format for granular tool control.
- **ACP support:** Kiro CLI implements the Agent Client Protocol, allowing it to work as an agent in JetBrains IDEs and Zed.
- **Shared governance with Q Developer:** Uses the same MCP Registry governance model.

---

### Cline (VS Code Extension)

**Category:** VS Code extension for autonomous coding
**MCP Support:** Full — stdio, with its own config

**Config File:** Managed through Cline's settings UI within VS Code. Uses standard `mcpServers` format internally.

---

### OpenCode

**Category:** Terminal-based coding agent
**MCP Support:** Via project config

**Config File:** `opencode.jsonc` in project root

**Config Format (different):**
```json
{
  "mcp": {
    "server-name": {
      "type": "local",
      "command": ["npx", "-y", "some-package"],
      "enabled": true
    }
  }
}
```

**Key Differentiator:** Uses `"mcp"` as the top-level key, not `"mcpServers"`. Command is an array, not a string + args.

---

## The Config Format Matrix

| Tool | Top-Level Key | Config Location | Remote Key | Notes |
|---|---|---|---|---|
| Claude Desktop | `mcpServers` | Platform-specific AppData | N/A (UI-based) | Also supports `.mcpb` |
| Claude Code | `mcpServers` | CLI-managed, 3 scopes | `type: "http"` | `.mcp.json` for project scope |
| ChatGPT | N/A (no file) | UI only | UI only | Remote-only, no local config |
| Cursor | `mcpServers` | `~/.cursor/mcp.json` | Same file, SSE/HTTP URL | Project: `.cursor/mcp.json` |
| Windsurf | `mcpServers` | `~/.codeium/windsurf/mcp_config.json` | Same file | Per-tool toggling |
| VS Code | **`servers`** | `.vscode/mcp.json` | `type: "http"` | **Different key!** Has `inputs` |
| Visual Studio | **`servers`** | `<SOLUTIONDIR>\.mcp.json` | `url` field | Same as VS Code format |
| JetBrains | `mcpServers` | GUI (importable from Claude) | Streamable HTTP in UI | Can import from Claude config |
| Gemini CLI | `mcpServers` | `~/.gemini/settings.json` | **`httpUrl`** | **Different remote key!** |
| Zed | **`context_servers`** | Zed settings.json | **Not supported** | **Different key, stdio only!** |
| Q Developer | `mcpServers` | `~/.aws/amazonq/default.json` | `type: "http"` | Also legacy `mcp.json` |
| Kiro | `mcpServers` | `~/.kiro/settings/mcp.json` | `url` field | Has `autoApprove`, `disabledTools` |
| OpenCode | **`mcp`** | `opencode.jsonc` | N/A | **Different key and format** |

---

## MCPB Bundle Format Deep Dive

### History
- Originally released by Anthropic as "Desktop Extensions" with `.dxt` file extension
- Renamed to `.mcpb` (MCP Bundle) when donated to the MCP project (November 2025)
- Old `.dxt` files still work in Claude Desktop
- Current manifest spec version: 0.3 (December 2025)

### What's Inside
```
bundle.mcpb (ZIP file)
├── manifest.json          # Required: metadata, config, tool declarations
├── server/
│   └── index.js           # Server implementation
├── node_modules/          # Bundled dependencies
├── icon.png               # Optional: bundle icon
└── assets/                # Optional: screenshots, etc.
```

### Manifest Structure (Key Fields)
```json
{
  "name": "my-extension",
  "version": "1.0.0",
  "description": "What it does",
  "server": {
    "type": "node",           // "node", "python", or "binary"
    "entry_point": "server/index.js",
    "mcp_config": {
      "command": "node",
      "args": ["${__dirname}/server/index.js"],
      "env": {
        "API_KEY": "${user_config.api_key}"
      }
    }
  },
  "user_config": {
    "api_key": {
      "type": "string",
      "description": "Your API key",
      "required": true,
      "sensitive": true       // Stored in OS keychain
    }
  },
  "tools": [
    { "name": "tool_name", "description": "What it does" }
  ]
}
```

### Current Client Support for MCPB
| Client | MCPB Support |
|---|---|
| Claude Desktop | ✅ Native |
| Claude Code | ✅ Native |
| MCP for Windows | ✅ Native |
| Cursor | ❌ Not yet |
| Windsurf | ❌ Not yet |
| VS Code | ❌ Not yet |
| ChatGPT | ❌ Not applicable (remote-only) |
| All others | ❌ Not yet |

### Toolchain
```bash
npm install -g @anthropic-ai/mcpb
cd your-mcp-server/
mcpb init      # Interactive manifest.json creation
mcpb pack      # Package into .mcpb file
```

---

## Transport Protocol Landscape

MCP defines three transport mechanisms, and client support varies:

| Transport | Description | Adoption |
|---|---|---|
| **stdio** | Local process, JSON-RPC over stdin/stdout | Universal — every tool supports this |
| **Streamable HTTP** | Modern HTTP-based transport (spec 2025-03-26+) | Most tools support this now |
| **SSE (Server-Sent Events)** | Legacy remote transport | Being deprecated (e.g., Atlassian ending SSE June 2026) |

**Notable gaps:**
- **ChatGPT:** Remote only (HTTP/SSE). No stdio.
- **Zed:** Stdio only. No remote support yet (community PR in progress).
- **Some Windsurf servers:** Require hardcoded tokens due to limited env var expansion.

---

## Claude "Connectors" vs ChatGPT "Apps" vs Custom GPTs

### Claude Connectors
- Remote MCP servers connected via URL + OAuth
- Available in the "+" button menu or Settings > Connectors
- Curated library of first-party integrations
- Can add custom connectors with any MCP server URL
- **Also supports local servers** via config file and MCPB

### ChatGPT Apps (formerly Connectors)
- Remote MCP servers connected via Settings > Apps & Connectors
- Renamed from "Connectors" on December 17, 2025
- Require Developer Mode for full MCP tool use in chat
- Without Developer Mode, only `search`/`fetch` tools work (Deep Research)
- OAuth-based authentication
- **Remote-only** — no local server support in ChatGPT itself
- Partner-built connectors reviewed by OpenAI (Amplitude, Stripe, Vercel, etc.)

### Custom GPTs (OpenAI — NOT MCP)
- **Completely different system** — uses OpenAPI/Swagger specs, not MCP
- REST endpoint integration via "Actions"
- Predates MCP adoption
- Still exists alongside Apps but serves a different purpose
- Packages persona + instructions + API actions together
- Being gradually complemented (not replaced) by Apps

### Key Differences Summary

| Feature | Claude Connectors | ChatGPT Apps | Custom GPTs |
|---|---|---|---|
| Protocol | MCP | MCP | OpenAPI/REST |
| Local servers | ✅ (via config/MCPB) | ❌ | ❌ |
| Remote servers | ✅ | ✅ | ✅ |
| Auth method | OAuth | OAuth | OAuth / API key |
| Write actions | ✅ | ✅ (with confirmation) | ✅ |
| Config file | Yes | No | No |
| Developer mode needed | No | Yes (for full tools) | No |

---

## Requirements for a Universal MCP Installer App

### 1. Tool Detection
Scan the system for installed tools by checking for:
- **Claude Desktop:** Config file existence at platform-specific paths
- **Cursor:** `~/.cursor/` directory
- **Windsurf:** `~/.codeium/windsurf/` directory
- **VS Code:** `.vscode/` or VS Code settings path
- **JetBrains:** Running IDE instances or known config paths
- **Gemini CLI:** `~/.gemini/` directory or `gemini` on PATH
- **Zed:** Zed config directory
- **Q Developer:** `~/.aws/amazonq/` directory
- **Kiro:** `~/.kiro/` directory
- **Claude Code:** `claude` on PATH

### 2. Config Format Writers
You need a config writer per target that handles:
- **Key name differences:** `mcpServers` vs `servers` vs `context_servers` vs `mcp`
- **Remote URL key:** `url` vs `httpUrl` vs `type + url`
- **Type field:** VS Code requires `"type": "stdio"`, most others don't
- **Merge behavior:** Read existing config, add server, write back without breaking other entries
- **Platform-specific paths:** macOS vs Windows vs Linux for each tool

### 3. Scope Management
Let users choose installation scope:
- **Global** (available across all projects)
- **Project-scoped** (available only in current workspace)
- Each tool has different scope mechanisms

### 4. Transport Type Handling
- **stdio** (local process): Universal support, needs runtime (Node.js, Python, or binary)
- **Streamable HTTP**: Most tools except Zed
- **SSE**: Legacy, being phased out, but still needed for some servers

### 5. Runtime Dependency Management
- Check for Node.js, Python, uv, Docker as needed
- Claude Desktop bundles Node.js (for MCPB), but other tools don't
- Consider shipping a runtime or checking/installing prerequisites

### 6. Credential Management
Different tools handle secrets differently:
- **MCPB:** OS keychain
- **Cursor/Windsurf:** Plain text in JSON (⚠️)
- **VS Code:** `inputs` mechanism for prompted entry
- **Gemini CLI:** Env var expansion with automatic redaction of sensitive patterns
- **Kiro:** Env var expansion with `${VAR}` syntax
- **Your installer should:** Manage a credential store and inject appropriately per target

### 7. MCPB Generation (Optional but High Value)
For Claude Desktop specifically, generate `.mcpb` bundles for a superior install experience. Fall back to JSON config for all other tools.

### 8. Docker Integration
Docker MCP Toolkit is the primary bridge for ChatGPT and is popular for sandboxed server execution. Detect Docker Desktop, offer container-based installations as an option.

### 9. ChatGPT Handling
Since ChatGPT has no local config file:
- Option A: Skip ChatGPT, document that it's remote-only
- Option B: Offer to set up a Docker MCP Gateway or ngrok/Cloudflare tunnel for local servers
- Option C: For remote MCP servers, provide a "copy URL" button for manual paste into ChatGPT Settings

### 10. Existing Tools in This Space
- **mcphub:** Tauri app that syncs MCP configs across Claude Code, Cursor, etc. Early stage.
- **MCP SuperAssistant:** Chrome extension that bridges MCP to chat UIs via the browser
- **Docker MCP Toolkit:** Docker Desktop integration for managing MCP servers
- **mcp.run:** Platform for discovering and managing MCP tools across clients

---

## Emerging Standards and Trends

### Agent Client Protocol (ACP)
A separate but complementary protocol standardizing agent-to-editor communication (like LSP did for language servers). Supported by JetBrains, Zed, and Kiro. Allows external agents to be plugged into any ACP-compatible editor.

### MCP Registry Standard
A formal JSON schema (v0.1) for defining lists of approved MCP servers. Used by Amazon Q Developer for enterprise governance. Expect other tools to adopt similar registry-based governance.

### SSE Deprecation
The MCP ecosystem is moving from SSE to Streamable HTTP. Atlassian has announced SSE endpoint deprecation by June 30, 2026. Build for Streamable HTTP as the primary remote transport.

### Protocol Versioning
The MCP spec is versioned (current: 2025-06-18). Some clients lag behind — Zed notably still doesn't support the latest protocol version. Your installer should be aware of which protocol features each client supports.

---

## Recommended Architecture for the Installer

```
┌─────────────────────────────────────┐
│         Universal MCP Installer      │
├─────────────────────────────────────┤
│  1. Tool Scanner                     │
│     Detect installed AI tools        │
├─────────────────────────────────────┤
│  2. Server Registry                  │
│     Catalog of available MCP servers │
│     with metadata, required config   │
├─────────────────────────────────────┤
│  3. Config Writers (per tool)        │
│     Claude | Cursor | Windsurf |     │
│     VS Code | Gemini | Zed |         │
│     Q Dev | Kiro | JetBrains         │
├─────────────────────────────────────┤
│  4. Credential Manager               │
│     Secure storage + injection       │
├─────────────────────────────────────┤
│  5. Runtime Manager                  │
│     Node.js / Python / Docker check  │
├─────────────────────────────────────┤
│  6. MCPB Packager (optional)         │
│     Generate bundles for Claude      │
└─────────────────────────────────────┘
```

This gap in the market is real. No tool today covers the full matrix of clients, formats, scopes, and transport types. The closest are mcphub and Docker MCP Toolkit, but both are limited in scope. A well-executed universal installer would be a genuine utility for the MCP ecosystem.
