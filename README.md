# Brightwing MCP Manager

A desktop application for managing MCP (Model Context Protocol) servers across all your AI tools from a single interface.

Install, enable, disable, and sync MCP servers across Claude Code, Cursor, VS Code, OpenAI Codex, Windsurf, Gemini CLI, and more — without hand-editing JSON or TOML config files.

Built with [Tauri v2](https://v2.tauri.app/) (Rust + React + TypeScript).

## Features

- **Unified dashboard** — See all your MCP servers across all tools in one grid. Tool abbreviations as columns, server names as rows, checkboxes in cells.
- **Cross-tool sync** — Check a box to install a server into a new tool. The config is copied automatically.
- **Batch operations** — Make multiple changes, then save them all at once with a progress indicator.
- **Enable/disable toggles** — Temporarily disable an MCP server without losing its config. Re-enable restores it exactly as it was.
- **Auto-detection** — Scans your machine for installed AI tools and their configured MCP servers on launch.
- **Scoreboard search** — Search the [MCP Scoreboard](https://mcpscoreboard.com) for quality-scored MCP servers and install them directly.
- **One-click install** — Supports `brightwing://` deep links for installing servers from the web.
- **Tool restart** — Prompts to restart tools after config changes and can restart them for you.
- **Favorites** — Bookmark servers for quick access.

## Supported Tools

| Tool | Config Format | Status |
|------|--------------|--------|
| Claude Code | CLI (`claude mcp`) | Full support |
| Cursor | JSON | Full support |
| VS Code (Copilot) | JSON | Full support |
| OpenAI Codex | TOML | Full support |
| Windsurf | JSON | Full support |
| Gemini CLI | JSON | Full support |
| Antigravity | JSON | Full support |
| Claude Desktop | JSON (local servers only) | Read-only for cloud connectors |

Claude Desktop's OAuth-based cloud connectors (Google Calendar, Gmail, Slack, etc.) are managed by Anthropic's servers and cannot be controlled externally. Local MCP servers added to `claude_desktop_config.json` work normally.

## Download

Pre-built binaries are available on the [Releases](https://github.com/Brightwing-Systems-LLC/mcp-manager/releases) page.

| Platform | File |
|----------|------|
| macOS (Apple Silicon) | `Brightwing.MCP.Manager_x.x.x_aarch64.dmg` |
| macOS (Intel) | `Brightwing.MCP.Manager_x.x.x_x64.dmg` |
| Windows (64-bit) | `Brightwing.MCP.Manager_x.x.x_x64-setup.exe` |
| Windows (MSI) | `Brightwing.MCP.Manager_x.x.x_x64_en-US.msi` |
| Linux (Debian/Ubuntu) | `brightwing-mcp-manager_x.x.x_amd64.deb` |
| Linux (AppImage) | `brightwing-mcp-manager_x.x.x_amd64.AppImage` |

### Unsigned Build Workarounds

Builds are not yet code-signed. Your OS will likely warn you when you first run the app.

**macOS:**

1. Open the `.dmg` and drag the app to Applications
2. On first launch, macOS will block it. Go to **System Settings → Privacy & Security**
3. Scroll down to the "Brightwing MCP Manager was blocked" message and click **Open Anyway**
4. Alternatively, run this in Terminal before first launch:
   ```bash
   xattr -cr "/Applications/Brightwing MCP Manager.app"
   ```

**Windows:**

1. Run the `.exe` or `.msi` installer
2. Windows SmartScreen will show "Windows protected your PC"
3. Click **More info** → **Run anyway**

**Linux:**

For the AppImage:
```bash
chmod +x brightwing-mcp-manager_*.AppImage
./brightwing-mcp-manager_*.AppImage
```

For the `.deb`:
```bash
sudo dpkg -i brightwing-mcp-manager_*.deb
```

## Build from Source

### Prerequisites

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) (stable)
- Platform-specific dependencies:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Linux**: `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`
  - **Windows**: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with C++ workload

### Steps

```bash
# Clone the repo
git clone https://github.com/Brightwing-Systems-LLC/mcp-manager.git
cd mcp-manager

# Install dependencies
npm install

# Run in development mode
npx tauri dev

# Build for production
npx tauri build
```

The built app will be in `src-tauri/target/release/bundle/`.

## How It Works

MCP Manager reads and writes the native config files that each AI tool uses:

- **JSON-based tools** (Claude Desktop, Cursor, VS Code, Windsurf, Antigravity): Reads/writes their config JSON files directly (e.g., `~/.cursor/mcp.json`, `claude_desktop_config.json`)
- **TOML-based tools** (Codex): Reads/writes TOML config with `toml_edit` to preserve formatting
- **CLI-based tools** (Claude Code, Gemini CLI): Uses the tool's CLI commands (`claude mcp add-json`, `claude mcp remove`, etc.)

The disable/enable feature works by snapshotting the server config to a local SQLite database, removing it from the tool's config file, and restoring it on re-enable.

## Architecture

```
src/                     # React frontend (TypeScript)
├── components/          # UI components (Dashboard, ApiKeys, InstallDialog, etc.)
├── lib/                 # Types, Tauri invoke wrappers
└── store.ts             # Zustand state management

src-tauri/               # Rust backend
├── src/
│   ├── config/          # Config reader/writer for all tool formats
│   ├── db/              # SQLite database (installations, disabled servers, favorites)
│   ├── tools/           # Tool definitions, scanner
│   └── lib.rs           # Tauri commands
```

## Deep Links

MCP Manager registers the `brightwing://` URL scheme. Websites can link directly to server installation:

```
brightwing://install?server=<uuid>&tool=<tool_id>
```

This is used by the [MCP Scoreboard](https://mcpscoreboard.com) for one-click "Install with Brightwing" buttons.

## Roadmap

- [ ] Encrypted API key vault (Stronghold)
- [ ] OAuth detection with per-tool auth instructions
- [ ] Vault export/import for backup and machine migration
- [ ] Code signing for macOS, Windows, and Linux
- [ ] Auto-update via Tauri updater plugin

## License

Proprietary. Copyright 2026 Brightwing Systems LLC.
