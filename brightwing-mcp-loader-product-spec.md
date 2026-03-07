# Brightwing MCP Loader — Product Specification
## A Tauri v2 Desktop Application for Universal MCP Server Installation
**Version:** 0.1 (Initial Spec)
**Author:** Keyton Weissinger / Brightwing Systems
**Date:** March 6, 2026

---

## 1. Product Overview

### 1.1 What It Is
Brightwing MCP Loader ("Brightwing") is a lightweight desktop application that lets users install, uninstall, and manage MCP servers across every AI tool on their machine from a single interface. It connects to the PatchworkMCP Scoreboard (patchworkmcp.com) to provide quality-scored server recommendations and supports one-click installation from the website via a custom URL protocol.

### 1.2 The Problem
Installing an MCP server today requires knowing which config file to edit, what JSON format each tool expects, what command to run, and what environment variables are needed. Each AI tool has a different config path, a different JSON structure, and different quirks. This friction limits MCP adoption to developers comfortable hand-editing JSON.

### 1.3 The Solution
Brightwing abstracts all of this away. The user picks a server, picks which AI tools to install it into, and Brightwing handles the config writing. It knows the paths, the formats, and the differences. It's the missing bridge between the MCP server ecosystem and the end user.

### 1.4 Strategic Position
Brightwing sits inside the Brightwing Systems product ecosystem as the distribution/activation layer for PatchworkMCP. The value chain is: PatchworkMCP scores servers → Brightwing installs them → users experience quality servers → publishers see install metrics → publishers care about their PatchworkMCP score.

---

## 2. Target Users

### 2.1 Primary: Developer-users of AI coding tools
People who use Claude Desktop, Cursor, Windsurf, VS Code with Copilot, or similar tools and want to try MCP servers without the hassle of manual config editing. Comfortable with installing desktop apps but don't want to hand-edit JSON across five different files.

### 2.2 Secondary: MCP server publishers
Developers who build MCP servers and want an easy distribution path. They embed "Install with Brightwing" badges in their READMEs, which drives traffic through PatchworkMCP and provides install metrics.

### 2.3 Tertiary: Enterprise teams evaluating MCP servers
Teams that want a controlled way to roll out approved MCP servers to developers. (Future: managed profiles, allow-lists.)

---

## 3. Technology Stack

### 3.1 Framework
**Tauri v2** — Rust backend + web frontend rendered in the system webview.

**Rationale:** Small binary (~10-15MB), native performance for filesystem operations, professional feel for a trust-oriented product. The Rust layer handles file I/O, process detection, and deep link registration. The frontend handles all UI rendering.

### 3.2 Frontend
**React + TypeScript + Vite** — Standard Tauri v2 React template.

**Styling:** Tailwind CSS for rapid development. Keep the design clean, utility-focused, and consistent with PatchworkMCP's visual identity.

**Rationale:** React is the most well-supported frontend in the Tauri ecosystem. TypeScript gives type safety in the UI layer where most code lives. Vite is the default bundler recommended by Tauri.

### 3.3 Local Storage
**SQLite via `tauri-plugin-sql`** — Tracks installation state, user preferences, cached server metadata.

### 3.4 Backend API
**PatchworkMCP API (Django REST)** — The existing PatchworkMCP Django backend, extended with new endpoints for Brightwing. Authentication via the same user accounts used on patchworkmcp.com.

### 3.5 Key Tauri Plugins
| Plugin | Purpose |
|---|---|
| `tauri-plugin-deep-link` | Handle `brightwing://` protocol URLs |
| `tauri-plugin-single-instance` | Prevent multiple app instances |
| `tauri-plugin-sql` | SQLite for local state |
| `tauri-plugin-shell` | Detect installed tools via PATH checks |
| `tauri-plugin-fs` | Read/write config files with proper permissions |
| `tauri-plugin-http` | API calls to PatchworkMCP |
| `tauri-plugin-updater` | Auto-update the app |
| `tauri-plugin-notification` | Toast notifications for install/uninstall status |

---

## 4. Core Features (MVP)

### 4.1 Tool Scanner

**What it does:** Detects which MCP-capable AI tools are installed on the user's machine.

**Detection strategy per tool:**

| Tool | Detection Method | Config Path (macOS) | Config Path (Windows) |
|---|---|---|---|
| Claude Desktop | Check for config directory | `~/Library/Application Support/Claude/` | `%APPDATA%\Claude\` |
| Cursor | Check for config directory | `~/.cursor/` | `%USERPROFILE%\.cursor\` |
| Windsurf | Check for config directory | `~/.codeium/windsurf/` | `%USERPROFILE%\.codeium\windsurf\` |
| VS Code | Check for settings directory | `~/Library/Application Support/Code/` | `%APPDATA%\Code\` |
| Claude Code | Check PATH for `claude` binary | `which claude` | `where claude` |
| Gemini CLI | Check for config directory or PATH | `~/.gemini/` | `%USERPROFILE%\.gemini\` |
| Zed | Check for config directory | `~/Library/Application Support/Zed/` | N/A (macOS/Linux only) |
| Kiro | Check for config directory | `~/.kiro/` | `%USERPROFILE%\.kiro\` |

**Behavior:**
- Runs on app startup and caches results.
- Displays detected tools with green checkmarks in the UI.
- User can manually trigger a re-scan.
- User can manually add/hide tools if detection is wrong.

**Rust implementation:** A Tauri command (`scan_tools`) that checks filesystem paths and PATH, returns a `Vec<DetectedTool>` struct to the frontend.

### 4.2 Config Writers

**What it does:** Reads an existing config file for a given tool, merges in a new MCP server entry (or removes one), and writes the file back without disturbing other entries.

**Format translation table:**

| Tool | Top-Level Key | Remote URL Key | Extra Fields | Notes |
|---|---|---|---|---|
| Claude Desktop | `mcpServers` | N/A (UI connectors) | — | Standard format |
| Cursor | `mcpServers` | SSE/HTTP in same format | — | Global + project-scoped |
| Windsurf | `mcpServers` | Same format | — | Different path from Cursor |
| VS Code | `servers` | `type` + `url` | Requires `type: "stdio"` | Different key name! |
| Claude Code | `mcpServers` | `type: "http"` | — | Via CLI: `claude mcp add-json` |
| Gemini CLI | `mcpServers` | `httpUrl` | — | Different remote key! |
| Zed | `context_servers` | Not supported | `source: "custom"` | Different key + schema |
| Kiro | `mcpServers` | `url` | `autoApprove`, `disabledTools` | Env var expansion |

**Core operation (pseudocode):**
```
fn install_server(tool: Tool, server: ServerConfig) -> Result<()> {
    let config_path = tool.config_path();
    let existing = read_json_or_empty(config_path)?;
    let key = tool.servers_key();  // "mcpServers" or "servers" or "context_servers"

    let mut servers = existing.get(key).unwrap_or_default();
    let entry = tool.format_entry(server);  // Transform to tool-specific format
    servers.insert(server.config_key, entry);

    let mut output = existing.clone();
    output[key] = servers;
    write_json(config_path, output)?;

    // Record in local DB
    db.record_installation(server.uuid, tool.id, server.config_key)?;

    Ok(())
}
```

**Safety measures:**
- Always read before writing (never overwrite the whole file).
- Create a `.brightwing-backup` copy of the config file before first modification.
- Validate JSON before writing (don't write malformed output).
- Handle the case where the config file doesn't exist yet (create with just the new entry).
- Handle the case where the config file is not valid JSON (alert the user, don't touch it).
- Use file locking to prevent race conditions if the AI tool is also writing.

**Claude Code special handling:** Claude Code doesn't use a simple JSON file. Instead, Brightwing shells out to `claude mcp add-json <name> '<json>'` with the appropriate `--scope` flag. This means Claude Code must be on PATH.

### 4.3 Deep Link Protocol Handler

**What it does:** Registers `brightwing://` as a custom URL protocol so the PatchworkMCP website can launch the app with a specific server pre-selected.

**URL format:**
```
brightwing://install?server={uuid}
brightwing://install?server={uuid}&tool={tool_id}
brightwing://uninstall?server={uuid}
brightwing://view?server={uuid}
```

**Implementation:**
- Register via `tauri-plugin-deep-link` with scheme `brightwing`.
- Combine with `tauri-plugin-single-instance` so clicking a link activates the existing window rather than spawning a new instance.
- On macOS: handled via `open-url` event (single instance is enforced by the OS).
- On Windows/Linux: handled via `second-instance` event with URL parsed from `process.argv`.

**Flow:**
1. User clicks "Install via Brightwing" on patchworkmcp.com.
2. Browser opens `brightwing://install?server={uuid}`.
3. OS routes to Brightwing app.
4. App fetches server metadata from PatchworkMCP API (if not cached).
5. App shows installation dialog: server details + checkboxes for each detected tool.
6. User checks the tools they want and clicks "Install."
7. Config writers do their thing.
8. App shows success/restart-needed status.

### 4.4 Server Installation UI

**What it does:** The main interface for installing a specific server into one or more tools.

**Layout (when triggered by deep link or search):**
```
┌──────────────────────────────────────────────────┐
│  [Server Icon]  unifi-network-mcp          A+ 95 │
│  MCP server for UniFi network application        │
│  by sirkirby · ★ 183 · Python                    │
│  ───────────────────────────────────────────────  │
│                                                   │
│  Install into:                                    │
│  ┌─────────────────────────────────────────────┐  │
│  │ ✅ Claude Desktop     [Installed ✓] [Remove]│  │
│  │ ☐  Cursor             [Not installed]       │  │
│  │ ✅ Windsurf           [Not installed]       │  │
│  │ ☐  VS Code            [Not installed]       │  │
│  │ ━━ Zed                [Incompatible ⚠]      │  │
│  └─────────────────────────────────────────────┘  │
│                                                   │
│  Configuration:                                   │
│  ┌─────────────────────────────────────────────┐  │
│  │ UNIFI_HOST:  [________________________]     │  │
│  │ UNIFI_USER:  [________________________]     │  │
│  │ UNIFI_PASS:  [________________________] 🔒  │  │
│  └─────────────────────────────────────────────┘  │
│                                                   │
│  [Install Selected]                    [Cancel]   │
│                                                   │
│  ⚠ Restart Claude Desktop to activate changes.   │
└──────────────────────────────────────────────────┘
```

**States per tool:**
- **Not installed** — checkbox available
- **Installed (by Brightwing)** — green checkmark, "Remove" button
- **Installed (externally)** — yellow indicator, "detected existing config with same server key"
- **Incompatible** — grayed out with reason (e.g., "Zed doesn't support remote servers")

### 4.5 Dashboard (Main View)

**What it does:** The home screen when the app is opened directly (not via deep link).

**Layout:**
```
┌──────────────────────────────────────────────────┐
│  Brightwing MCP Loader                    [⚙]    │
│  ───────────────────────────────────────────────  │
│                                                   │
│  My Tools:  Claude Desktop · Cursor · Windsurf   │
│             VS Code · Claude Code                 │
│             [Rescan]                              │
│  ───────────────────────────────────────────────  │
│                                                   │
│  Installed Servers (7):                           │
│  ┌─────────────────────────────────────────────┐  │
│  │ github-mcp-server     A  │ CD CU WS VC     │  │
│  │ filesystem            B  │ CD    WS         │  │
│  │ brave-search           B  │ CD CU            │  │
│  │ ...                       │                  │  │
│  └─────────────────────────────────────────────┘  │
│                                                   │
│  ★ Favorites (synced from patchworkmcp.com):      │
│  ┌─────────────────────────────────────────────┐  │
│  │ [Server tiles from favorites list]           │  │
│  └─────────────────────────────────────────────┘  │
│                                                   │
│  🔍 Search PatchworkMCP...  [________________]    │
│                                                   │
│  Recommended:                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │ [Curated recommendation tiles]               │  │
│  └─────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

**Key behaviors:**
- The "Installed Servers" grid is built by scanning all detected tools' config files and cross-referencing with Brightwing's local DB.
- Tool abbreviations (CD, CU, WS, VC, CC, GC, ZD, KR) as compact indicators of where each server is installed.
- Clicking a server row opens the Server Installation UI (section 4.4) in context.
- Favorites are synced from the user's PatchworkMCP account via API.
- Recommendations are fetched from a PatchworkMCP API endpoint (editorially curated or algorithmically generated).
- Search queries the PatchworkMCP API and shows results inline.

### 4.6 Authentication

**What it does:** Connects Brightwing to the user's PatchworkMCP account for favorites sync, install tracking, and personalized recommendations.

**Implementation:**
- OAuth 2.0 PKCE flow against PatchworkMCP's Django backend.
- Opens system browser for login (not an embedded webview — better security practice).
- Stores refresh token securely via Tauri's secure storage (OS keychain).
- Falls back to anonymous/offline mode if not logged in (can still install from deep links with cached data, just no favorites or recommendations).
- Login state is reported to the PatchworkMCP API so the website knows to show "Install via Brightwing" buttons.

---

## 5. Data Model

### 5.1 PatchworkMCP API (New Endpoints Needed)

**`GET /api/brightwing/server/{uuid}/install-config/`**
Returns installation metadata for a specific server.
```json
{
    "uuid": "3d984130-0a88-4a4b-96bf-1e86033ed46c",
    "name": "unifi-network-mcp",
    "display_name": "UniFi Network MCP",
    "description": "MCP server implementation for the UniFi network application",
    "publisher": "sirkirby",
    "grade": "A+",
    "score": 95,
    "language": "python",
    "github_url": "https://github.com/sirkirby/unifi-network-mcp",
    "server_type": "local",
    "transport": "stdio",
    "install_config": {
        "command": "uvx",
        "args": ["unifi-network-mcp"],
        "env": {
            "UNIFI_HOST": {"required": true, "sensitive": false, "description": "UniFi controller hostname"},
            "UNIFI_USERNAME": {"required": true, "sensitive": false, "description": "Admin username"},
            "UNIFI_PASSWORD": {"required": true, "sensitive": true, "description": "Admin password"},
            "UNIFI_PORT": {"required": false, "sensitive": false, "description": "Port (default: 443)", "default": "443"},
            "UNIFI_SITE": {"required": false, "sensitive": false, "description": "Site name (default: default)", "default": "default"}
        },
        "config_key": "unifi-network-mcp"
    },
    "compatibility": {
        "claude_desktop": true,
        "cursor": true,
        "windsurf": true,
        "vscode": true,
        "claude_code": true,
        "gemini_cli": true,
        "zed": true,
        "kiro": true,
        "chatgpt": false
    },
    "install_notes": "Requires a UniFi controller accessible from your machine."
}
```

**`GET /api/brightwing/favorites/`**
Returns the user's favorited servers (requires auth).

**`GET /api/brightwing/recommendations/`**
Returns editorially curated or personalized server recommendations.

**`POST /api/brightwing/installs/`**
Reports an installation event (server UUID, tool, timestamp). Used for install metrics visible to publishers.

**`GET /api/brightwing/search/?q={query}`**
Searches the server catalog. Returns lightweight results with install configs.

### 5.2 Local SQLite Schema

```sql
-- Detected AI tools on this machine
CREATE TABLE detected_tools (
    id TEXT PRIMARY KEY,            -- e.g., "claude_desktop"
    display_name TEXT NOT NULL,
    config_path TEXT,               -- Resolved absolute path to config file
    detected_at TIMESTAMP,
    last_verified TIMESTAMP,
    is_hidden BOOLEAN DEFAULT FALSE -- User can hide tools they don't use
);

-- Servers installed by Brightwing
CREATE TABLE installations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_uuid TEXT NOT NULL,      -- PatchworkMCP UUID
    server_name TEXT NOT NULL,
    tool_id TEXT NOT NULL,          -- FK to detected_tools.id
    config_key TEXT NOT NULL,       -- The key used in the JSON config
    installed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    config_snapshot TEXT,           -- The JSON entry we wrote (for verification)
    UNIQUE(server_uuid, tool_id)
);

-- Cached server metadata from PatchworkMCP API
CREATE TABLE server_cache (
    uuid TEXT PRIMARY KEY,
    name TEXT,
    display_name TEXT,
    grade TEXT,
    score INTEGER,
    language TEXT,
    install_config_json TEXT,       -- Full install_config blob
    compatibility_json TEXT,
    fetched_at TIMESTAMP
);

-- Config file backups
CREATE TABLE config_backups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_id TEXT NOT NULL,
    config_path TEXT NOT NULL,
    backup_content TEXT NOT NULL,   -- Full file contents before modification
    backed_up_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- User preferences
CREATE TABLE preferences (
    key TEXT PRIMARY KEY,
    value TEXT
);
```

---

## 6. Project Structure

```
brightwing/
├── src/                            # React frontend (TypeScript)
│   ├── App.tsx                     # Root component + routing
│   ├── main.tsx                    # Entry point
│   ├── components/
│   │   ├── Dashboard.tsx           # Main view with installed servers grid
│   │   ├── ServerInstall.tsx       # Installation dialog for a specific server
│   │   ├── ToolList.tsx            # Detected tools display
│   │   ├── ServerCard.tsx          # Server tile (name, grade, indicators)
│   │   ├── ConfigForm.tsx          # Dynamic form for env vars / API keys
│   │   ├── SearchBar.tsx           # Search PatchworkMCP catalog
│   │   └── StatusToast.tsx         # Success/error notifications
│   ├── hooks/
│   │   ├── useTools.ts             # Hook for detected tools state
│   │   ├── useInstallations.ts     # Hook for installation state
│   │   └── useApi.ts               # Hook for PatchworkMCP API calls
│   ├── lib/
│   │   ├── tauri.ts                # Tauri command invocations (typed)
│   │   ├── types.ts                # TypeScript type definitions
│   │   └── constants.ts            # Tool IDs, config paths, etc.
│   └── styles/
│       └── index.css               # Tailwind imports + custom styles
│
├── src-tauri/                      # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json             # Tauri configuration
│   ├── capabilities/               # Tauri v2 permission capabilities
│   │   └── default.json
│   ├── src/
│   │   ├── main.rs                 # Entry point, plugin registration
│   │   ├── lib.rs                  # Tauri command exports
│   │   ├── tools/
│   │   │   ├── mod.rs              # Tool scanner module
│   │   │   ├── scanner.rs          # Detect installed AI tools
│   │   │   └── definitions.rs      # Static tool definitions (paths, formats)
│   │   ├── config/
│   │   │   ├── mod.rs              # Config writer module
│   │   │   ├── reader.rs           # Read and parse config files
│   │   │   ├── writer.rs           # Merge and write config files
│   │   │   ├── formats.rs          # Per-tool format transformations
│   │   │   └── backup.rs           # Config file backup/restore
│   │   ├── db/
│   │   │   ├── mod.rs              # SQLite operations
│   │   │   ├── migrations.rs       # Schema setup
│   │   │   └── queries.rs          # CRUD operations
│   │   └── deeplink/
│   │       └── mod.rs              # Deep link URL parsing
│   └── icons/                      # App icons for all platforms
│
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.js
└── README.md
```

---

## 7. Tauri Commands (Rust → Frontend Interface)

These are the `#[tauri::command]` functions exposed to the React frontend.

```rust
// Tool scanning
#[tauri::command]
fn scan_tools() -> Result<Vec<DetectedTool>, String>

// Config operations
#[tauri::command]
fn install_server(tool_id: String, server_config: ServerInstallConfig) -> Result<InstallResult, String>

#[tauri::command]
fn uninstall_server(tool_id: String, server_uuid: String) -> Result<(), String>

#[tauri::command]
fn get_installed_servers() -> Result<Vec<Installation>, String>

#[tauri::command]
fn read_tool_config(tool_id: String) -> Result<serde_json::Value, String>

// Backup/restore
#[tauri::command]
fn backup_config(tool_id: String) -> Result<String, String>

#[tauri::command]
fn restore_config(tool_id: String, backup_id: i64) -> Result<(), String>

// Deep link
#[tauri::command]
fn get_pending_deep_link() -> Result<Option<DeepLinkAction>, String>

#[tauri::command]
fn clear_pending_deep_link() -> Result<(), String>
```

**TypeScript invocations (frontend side):**
```typescript
import { invoke } from '@tauri-apps/api/core';

const tools = await invoke<DetectedTool[]>('scan_tools');
const result = await invoke<InstallResult>('install_server', {
    toolId: 'claude_desktop',
    serverConfig: { /* ... */ }
});
```

---

## 8. PatchworkMCP Website Integration

### 8.1 "Install via Brightwing" Button

On each server detail page (e.g., `/scoreboard/server/{uuid}/`), add a button that:

1. Checks if the user is logged in and has `has_brightwing` flag set on their profile.
2. If yes: shows "Install via Brightwing" button linking to `brightwing://install?server={uuid}`.
3. If no/unknown: shows "Install via Brightwing" with the try-and-fallback pattern:
   - The link attempts `brightwing://install?server={uuid}`.
   - A 1.5-second timeout checks if the page still has focus.
   - If still focused (app didn't open), shows a modal: "Get Brightwing — the easy MCP installer" with download links.

### 8.2 Django Model Changes

```python
# On the User profile model (or a related model)
class UserProfile(models.Model):
    # ... existing fields ...
    has_brightwing = models.BooleanField(default=False)
    brightwing_last_seen = models.DateTimeField(null=True, blank=True)
```

### 8.3 Install Config Data Model

```python
class ServerInstallConfig(models.Model):
    server = models.OneToOneField(MCPServer, on_delete=models.CASCADE, related_name='install_config')
    command = models.CharField(max_length=255)                 # e.g., "npx", "uvx", "node"
    args = models.JSONField(default=list)                      # e.g., ["-y", "package-name"]
    env_schema = models.JSONField(default=dict)                # Env var definitions with metadata
    config_key = models.CharField(max_length=255)              # Key name in mcpServers config
    transport = models.CharField(max_length=20, default='stdio')  # stdio, http, sse
    compatibility = models.JSONField(default=dict)             # Per-tool compatibility flags
    install_notes = models.TextField(blank=True)
    source = models.CharField(max_length=50, default='manual') # manual, auto_extracted, publisher
    verified = models.BooleanField(default=False)              # Publisher-verified config
    created_at = models.DateTimeField(auto_now_add=True)
    updated_at = models.DateTimeField(auto_now=True)
```

### 8.4 Publisher Badge

Publishers can embed an install badge in their READMEs:

```markdown
[![Install with Brightwing](https://patchworkmcp.com/brightwing/badge/{uuid}.svg)](brightwing://install?server={uuid})
```

The badge SVG shows "Install with Brightwing" plus the server's grade.

---

## 9. Installation Config Pipeline

The biggest data challenge is getting accurate installation configs for 24,000+ servers. This is a phased approach:

### Phase 1: Manual curation (MVP launch)
- Manually create `ServerInstallConfig` entries for the top 100 most-popular servers.
- Focus on servers that are commonly mentioned in MCP tutorials and docs.
- These become the "Featured" / "Recommended" servers in Brightwing.

### Phase 2: Publisher-provided configs (Post-launch)
- When a publisher claims their server on PatchworkMCP, prompt them to provide install config.
- Provide a form that generates the config JSON and validates it.
- Publisher-provided configs get a "Verified" badge.

### Phase 3: Auto-extraction from READMEs (Ongoing)
- During the scoring pipeline, parse README.md files for JSON config blocks.
- Look for patterns like `"mcpServers":` or `"command": "npx"` in code fences.
- Extract and store as `source='auto_extracted'` with `verified=False`.
- Surface these in Brightwing with a "Community-extracted config — verify before use" warning.

### Phase 4: `server.json` standard adoption
- Monitor adoption of the MCP registry `server.json` standard.
- If a repo contains a `server.json`, parse it for install metadata.
- This is the cleanest long-term solution but adoption is still early.

---

## 10. Scope and Limitations

### 10.1 MVP Scope (v0.1)

**In scope:**
- Tool detection: Claude Desktop, Cursor, Windsurf, VS Code, Claude Code
- Config writing: All five above tools
- Deep link handler: `brightwing://install` and `brightwing://view`
- Dashboard: Installed servers grid, search, favorites (if logged in)
- Authentication: OAuth to PatchworkMCP
- Local state: SQLite tracking of installations
- Config backup: Before first modification of any config file
- macOS and Windows builds

**Out of scope for v0.1:**
- ChatGPT (remote-only, no config file to write)
- JetBrains IDEs (GUI-only config, no file to write)
- Zed (different format, stdio-only, limited adoption)
- Gemini CLI (add in v0.2)
- Kiro (add in v0.2)
- Linux builds (add in v0.2)
- MCPB bundle generation
- Enterprise features (managed profiles, allow-lists)
- Automatic restart of AI tools after config changes
- Config file watching / real-time sync

### 10.2 Known Limitations

**Cannot install into ChatGPT:** ChatGPT has no local config file. For remote servers, Brightwing can provide a "Copy URL" action and instructions. For local servers, ChatGPT is simply not a supported target.

**Cannot install into JetBrains:** JetBrains IDEs manage MCP config through their internal GUI. Workaround: copy config to clipboard + show instructions, or recommend "Import from Claude" in JetBrains settings.

**Restart required:** Most AI tools need a full restart to pick up config changes. Brightwing shows a notification but does not force-restart other apps.

**Plaintext secrets:** Most tools store API keys in plaintext JSON. This is not Brightwing's fault but it should be surfaced to users. Brightwing itself does not store secrets — it writes them to the target tool's config file.

**Concurrent modification:** If a user is editing their config file while Brightwing writes to it, data could be lost. Mitigation: file locking where possible, backup before writing.

---

## 11. Distribution and Updates

### 11.1 Packaging
- **macOS:** `.dmg` disk image (universal binary for Intel + Apple Silicon)
- **Windows:** `.msi` installer (or `.exe` via NSIS)
- Built via `cargo tauri build` in CI (GitHub Actions)

### 11.2 Code Signing
- **macOS:** Apple Developer ID certificate + notarization (required to avoid Gatekeeper warnings)
- **Windows:** Code signing certificate (EV recommended to avoid SmartScreen warnings)
- Note: Code signing certificates have cost implications. Can launch without them initially (with warnings) and add later.

### 11.3 Auto-Updates
- Via `tauri-plugin-updater` pointing to a release endpoint on brightwingsystems.com or GitHub Releases.
- Check for updates on app launch (non-blocking).
- Show update-available indicator in the UI.

### 11.4 Download Page
- Hosted at `brightwingsystems.com/brightwing/` or `patchworkmcp.com/brightwing/`
- Detects OS and offers the appropriate download.
- Also linked from every "Install via Brightwing" fallback modal on the scoreboard.

---

## 12. Monetization Alignment

### Free Tier
- Install servers from deep links (no limit)
- Dashboard with installed servers view
- Tool detection
- Search PatchworkMCP catalog
- Up to 5 favorited servers synced from patchworkmcp.com

### Pro Tier (future)
- Unlimited favorites sync
- Installation profiles (e.g., "Work" vs "Personal" — different servers for different contexts)
- Bulk install/uninstall
- Config backup history and restore
- "Install to all tools" one-click
- Export/import config bundles
- Priority support

### Publisher Tier (future, via PatchworkMCP)
- Install metrics dashboard (how many Brightwing installs of their server)
- "Install with Brightwing" badge generator
- Verified install config badge on scoreboard
- Featured placement in Brightwing recommendations

---

## 13. Development Phases

### Phase 1: Foundation (Weeks 1-2)
- [ ] Scaffold Tauri v2 project with React + TypeScript + Vite
- [ ] Implement tool scanner (Rust side)
- [ ] Implement config reader/writer for Claude Desktop (simplest format)
- [ ] Implement config reader/writer for Cursor
- [ ] Build basic Dashboard UI showing detected tools
- [ ] Build basic "Install Server" form (hardcoded test server)
- [ ] Set up SQLite local state

### Phase 2: Core Features (Weeks 3-4)
- [ ] Add config writers for Windsurf, VS Code, Claude Code
- [ ] Implement deep link protocol handler
- [ ] Build PatchworkMCP API endpoints (install-config, search, favorites)
- [ ] Connect frontend to PatchworkMCP API
- [ ] Build Server Installation UI with dynamic env var form
- [ ] Implement install/uninstall flow end-to-end
- [ ] Add config file backup system

### Phase 3: Integration (Weeks 5-6)
- [ ] Implement OAuth login flow
- [ ] Favorites sync
- [ ] Search integration
- [ ] Recommendations feed
- [ ] Add "Install via Brightwing" button to PatchworkMCP server detail pages
- [ ] Publisher install badge endpoint

### Phase 4: Polish and Distribution (Weeks 7-8)
- [ ] Error handling and edge cases
- [ ] Loading states, empty states, error states
- [ ] macOS and Windows packaging
- [ ] Auto-update infrastructure
- [ ] Download page
- [ ] Manual curation of top 100 server install configs
- [ ] Beta testing

---

## 14. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| AI tools change config file paths in updates | Medium | High | Version-detect tools and maintain path mappings per version. Monitor changelogs. |
| JSON config format changes across tools | Low | High | Abstract format logic per tool. Add format version detection. |
| Code signing costs delay distribution | Medium | Medium | Launch with unsigned builds for beta (accept the UX warnings). Budget for certs before public launch. |
| Install config data is sparse for most servers | High | Medium | Prioritize top 100 servers manually. Build auto-extraction pipeline. Make it easy for publishers to submit configs. |
| Tauri plugin compatibility issues | Medium | Medium | Pin plugin versions. Test on both macOS and Windows in CI. |
| Users run Brightwing and AI tool simultaneously, causing config write conflicts | Medium | Low | File locking, backup before write, and clear messaging about restart requirements. |

---

## 15. Success Metrics

### Launch (Week 8)
- Brightwing installable on macOS and Windows
- 50+ servers with verified install configs
- Deep link flow working from patchworkmcp.com
- 5 AI tools supported (Claude Desktop, Cursor, Windsurf, VS Code, Claude Code)

### Month 1
- 500+ Brightwing installs
- 100+ server installation events tracked
- 3+ publishers using "Install with Brightwing" badges

### Month 3
- 2,000+ Brightwing installs
- 500+ servers with install configs (via publisher submissions + auto-extraction)
- Brightwing mentioned in MCP community discussions as a recommended installation method

---

## Appendix A: Reference — Config File Paths by OS

### macOS
| Tool | Path |
|---|---|
| Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Cursor (global) | `~/.cursor/mcp.json` |
| Cursor (project) | `.cursor/mcp.json` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` |
| VS Code (user) | `~/Library/Application Support/Code/User/settings.json` |
| VS Code (workspace) | `.vscode/mcp.json` |
| Gemini CLI (user) | `~/.gemini/settings.json` |
| Gemini CLI (project) | `.gemini/settings.json` |
| Zed | `~/.config/zed/settings.json` |
| Kiro (global) | `~/.kiro/settings/mcp.json` |
| Kiro (workspace) | `.kiro/settings/mcp.json` |
| Q Developer (global) | `~/.aws/amazonq/default.json` |

### Windows
| Tool | Path |
|---|---|
| Claude Desktop | `%APPDATA%\Claude\claude_desktop_config.json` |
| Cursor (global) | `%USERPROFILE%\.cursor\mcp.json` |
| Windsurf | `%USERPROFILE%\.codeium\windsurf\mcp_config.json` |
| VS Code (user) | `%APPDATA%\Code\User\settings.json` |
| Gemini CLI (user) | `%USERPROFILE%\.gemini\settings.json` |
| Kiro (global) | `%USERPROFILE%\.kiro\settings\mcp.json` |
| Q Developer (global) | `%USERPROFILE%\.aws\amazonq\default.json` |

---

## Appendix B: Format Translation Examples

### Same server installed into each tool

**Server:** `brave-search` (npx -y @modelcontextprotocol/server-brave-search, needs BRAVE_API_KEY)

**Claude Desktop / Cursor / Windsurf / Kiro:**
```json
{
  "mcpServers": {
    "brave-search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "user-provided-key"
      }
    }
  }
}
```

**VS Code:**
```json
{
  "servers": {
    "brave-search": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "user-provided-key"
      }
    }
  }
}
```

**Claude Code (via shell command):**
```bash
claude mcp add-json brave-search '{"command":"npx","args":["-y","@modelcontextprotocol/server-brave-search"],"env":{"BRAVE_API_KEY":"user-provided-key"}}' --scope user
```

**Gemini CLI:**
```json
{
  "mcpServers": {
    "brave-search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "user-provided-key"
      }
    }
  }
}
```

**Zed:**
```json
{
  "context_servers": {
    "brave-search": {
      "source": "custom",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "user-provided-key"
      }
    }
  }
}
```

Note: All tools place this inside their own config file at their own path. Brightwing's config writer handles the key name, any extra fields, and the file location transparently.
