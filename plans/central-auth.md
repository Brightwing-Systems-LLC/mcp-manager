# Central Auth Proxy: Brightwing as the MCP Authentication Layer

## Vision

Transform the Brightwing MCP Manager from a **config file editor** into the **central nervous system for MCP on your machine**. Brightwing handles all MCP server authentication — OAuth flows, API key storage, token refresh — and exposes authenticated servers to local AI tools as simple stdio MCP proxies. No API keys in plaintext JSON files. No OAuth flows per-app. Authenticate once in Brightwing, use everywhere.

```
TODAY (Config File Editor):
  Claude Desktop → (OAuth/API key) → MCP Server X (remote)
  Cursor         → (OAuth/API key) → MCP Server X (remote)
  Claude Code    → (OAuth/API key) → MCP Server X (remote)
  (each app manages its own auth, user configures keys N times)

PROPOSED (Central Auth Proxy):
  Claude Desktop → stdio → brightwing-proxy (local)  ─┐
  Cursor         → stdio → brightwing-proxy (local)   ├→ Brightwing Auth Layer → MCP Server X
  Claude Code    → stdio → brightwing-proxy (local)  ─┘   (OAuth tokens, API keys,
  VS Code        → stdio → brightwing-proxy (local)  ─┘    token refresh — all managed)
```

---

## Strategic Context

### Why This Matters for the Ecosystem

The MCP security landscape is fragmented. API keys sit in plaintext JSON files scattered across `~/.cursor/mcp.json`, `~/Library/Application Support/Claude/claude_desktop_config.json`, `~/.gemini/settings.json`, and more. Every tool re-implements OAuth (or doesn't). Users configure the same credentials N times for M tools.

This creates three problems Brightwing is positioned to solve:

1. **Security:** Plaintext secrets in config files are a liability. One leaked config backup exposes everything.
2. **Friction:** Adding the same MCP server to 5 tools means entering the same API key 5 times, or doing OAuth 5 times.
3. **Lifecycle:** When an OAuth token expires, each tool handles refresh differently (or doesn't). API key rotation requires editing every config file.

### Competitive Positioning

The competitive landscape splits into two camps, neither of which addresses the individual developer's auth problem:

| Category | Players | Gap |
|----------|---------|-----|
| **Enterprise gateways** | MintMCP, Composio, Kong, Traefik Hub, TrueFoundry | Cloud-deployed, RBAC/SOC 2 focused. Overkill for individual developers. |
| **Developer proxies** | mcp-proxy, MetaMCP, FastMCP | CLI building blocks. No GUI, no credential management, no quality signal. |
| **Docker MCP Gateway** | Docker Desktop | Container-centric. Requires Docker Desktop. Good secrets management but heavy. |
| **maxtheman/mcpManager** | Solo project | Similar Tauri approach but no quality scoring, no auth proxy, weekend project. |

**Brightwing's unique position:** Quality-scored server discovery (24K+ servers via Scoreboard) + one-click install across 8+ tools + centralized auth proxy + encrypted credential vault. Nobody else combines all four.

### The Flywheel

```
mcpscoreboard.com (SEO magnet, 24K server pages)
    → MCP Manager desktop app (daily-use tool on developer machines)
        → Auth proxy makes it sticky (always running, managing tokens)
            → PatchworkMCP.com (paid SaaS for server publishers)
```

The auth proxy is the stickiness layer. Without it, the MCP Manager is opened occasionally to add/remove servers. With it, Brightwing is a background service that every MCP interaction flows through. That's the difference between a utility and a platform.

---

## Architecture Overview

### Component Map

```
┌─────────────────────────────────────────────────────────────┐
│  Brightwing MCP Manager (Tauri v2 GUI)                      │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐ │
│  │ Server       │ │ OAuth Login  │ │ API Key Entry        │ │
│  │ Discovery &  │ │ Flow (opens  │ │ (manual or from      │ │
│  │ Install      │ │ browser,     │ │ Scoreboard schema)   │ │
│  │              │ │ handles      │ │                      │ │
│  │              │ │ callback)    │ │                      │ │
│  └──────┬───────┘ └──────┬───────┘ └──────────┬───────────┘ │
│         │                │                     │             │
│         ▼                ▼                     ▼             │
│  ┌──────────────────────────────────────────────────────────┐│
│  │  Credential Store (IOTA Stronghold, encrypted at rest)   ││
│  │  - OAuth tokens (access + refresh + expiry)              ││
│  │  - API keys                                              ││
│  │  - Server connection metadata                            ││
│  └──────────────────────────────┬───────────────────────────┘│
│                                 │                            │
│  ┌──────────────────────────────┴───────────────────────────┐│
│  │  Auth Service (Rust, runs in-process or as daemon)       ││
│  │  - Token refresh lifecycle                               ││
│  │  - Credential dispensing via local IPC                    ││
│  │  - Proxy process management                              ││
│  └──────────────────────────────┬───────────────────────────┘│
└─────────────────────────────────┼────────────────────────────┘
                                  │
            ┌─────────────────────┼──────────────────────┐
            │                     │                      │
            ▼                     ▼                      ▼
   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
   │ brightwing-proxy │  │ brightwing-proxy │  │ brightwing-proxy │
   │ (server-a)       │  │ (server-b)       │  │ (server-c)       │
   │                  │  │                  │  │                  │
   │ stdin ←→ JSON-RPC│  │ stdin ←→ JSON-RPC│  │ stdin ←→ JSON-RPC│
   │         ↕        │  │         ↕        │  │         ↕        │
   │ HTTPS + auth     │  │ HTTPS + auth     │  │ stdio passthru   │
   │ headers          │  │ headers          │  │ + env injection   │
   └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘
            │                     │                      │
            ▼                     ▼                      ▼
   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
   │ Remote MCP      │  │ Remote MCP      │  │ Local MCP       │
   │ Server A        │  │ Server B        │  │ Server C        │
   │ (GitHub, Slack)  │  │ (Linear, Jira)  │  │ (filesystem)    │
   └─────────────────┘  └─────────────────┘  └─────────────────┘
```

### What Each AI Tool Sees

Instead of complex configs with API keys, each tool gets a simple stdio entry:

```json
{
  "mcpServers": {
    "bw-github": {
      "command": "brightwing-proxy",
      "args": ["--server", "github-mcp"]
    },
    "bw-linear": {
      "command": "brightwing-proxy",
      "args": ["--server", "linear-mcp"]
    }
  }
}
```

No env vars. No API keys. No OAuth URLs. Just a local binary and a server identifier.

---

## Component Design

### Component 1: `brightwing-proxy` Binary

A small, standalone Rust binary that acts as a transparent MCP proxy. Each AI tool spawns one instance per proxied server.

**Responsibilities:**
- Read JSON-RPC messages from stdin (from the AI tool)
- Fetch auth credentials from the Brightwing credential store
- Forward requests to the upstream MCP server with proper authentication
- Relay responses back to stdout
- Handle transport translation (stdio ↔ HTTP/SSE, stdio ↔ stdio+env)

**Binary characteristics:**
- Single static binary, no runtime dependencies
- Fast startup (<50ms) — AI tools spawn this frequently
- Small memory footprint (~5-10MB RSS)
- Built from the same Rust workspace as the Tauri app

**CLI interface:**
```bash
brightwing-proxy --server <server-id> [--socket <path>] [--verbose]

# Arguments:
#   --server    Server identifier (matches key in Brightwing's credential store)
#   --socket    Override IPC socket path (default: platform-specific)
#   --verbose   Enable debug logging to stderr
```

**Proxy modes (determined by upstream server config):**

| Upstream Transport | Proxy Behavior |
|-------------------|----------------|
| **Streamable HTTP** | stdio ↔ HTTP. Proxy adds `Authorization: Bearer <token>` header. |
| **SSE** | stdio ↔ SSE. Proxy establishes SSE connection with auth, translates to JSON-RPC on stdio. |
| **stdio (with secrets)** | stdio ↔ stdio. Proxy spawns the upstream command with secrets injected as env vars. Passes JSON-RPC through. |

**Credential retrieval:**
The proxy fetches credentials from the Brightwing auth service via a Unix domain socket (macOS/Linux) or named pipe (Windows).

```
brightwing-proxy                    Brightwing Auth Service
     │                                      │
     │──── GET_CREDENTIALS(server-id) ─────→│
     │                                      │ (looks up in Stronghold,
     │                                      │  refreshes token if needed)
     │←─── {type, token, url, env, ...} ────│
     │                                      │
     │──── (forward MCP traffic) ──────────→│ (upstream MCP server)
```

**IPC protocol (simple JSON over Unix socket):**
```json
// Request
{"action": "get_credentials", "server_id": "github-mcp"}

// Response (OAuth)
{"type": "oauth", "access_token": "gho_xxx...", "url": "https://github-mcp.example.com/mcp"}

// Response (API key)
{"type": "api_key", "env": {"GITHUB_TOKEN": "ghp_xxx..."}, "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"]}

// Response (error — needs re-auth)
{"type": "error", "code": "auth_expired", "message": "OAuth token expired. Please re-authenticate in Brightwing."}
```

**Error handling:**
- If the auth service is not running → proxy writes a JSON-RPC error response to stdout and exits with a clear message to stderr
- If credentials are expired and refresh fails → proxy returns an MCP error response indicating re-auth is needed
- If upstream server is unreachable → standard MCP error response

**Source location:** `src-tauri/src/bin/brightwing_proxy.rs` (separate binary target in the same Cargo workspace)

### Component 2: Auth Service (IPC Daemon)

Runs as part of the Brightwing app (or as a lightweight background daemon) and serves credential requests from proxy instances.

**Architecture decision: In-process vs. standalone daemon**

| Approach | Pros | Cons |
|----------|------|------|
| **In-process (Tauri app must be running)** | Simple, no daemon management, vault access is direct | Proxy doesn't work if user closes the GUI |
| **Standalone daemon (auto-launched)** | Works headlessly, survives GUI close | More complex, daemon lifecycle management |
| **Hybrid: daemon extracted on first run** | Best of both, daemon is lightweight | Two binaries to ship and manage |

**Recommended: Hybrid approach.**
- The Tauri GUI app manages credentials and handles OAuth flows (browser-based)
- On first proxy setup, Brightwing installs a lightweight background daemon (`brightwing-authd`) that:
  - Reads from the same Stronghold vault
  - Listens on a Unix socket / named pipe
  - Handles token refresh autonomously
  - Auto-starts on login (launchd on macOS, Task Scheduler on Windows, systemd user unit on Linux)
- The daemon is tiny (~3MB) — it only does credential storage access and token refresh

**Daemon IPC socket paths:**
- macOS: `~/Library/Application Support/com.brightwing.mcp-manager/authd.sock`
- Linux: `$XDG_RUNTIME_DIR/brightwing-authd.sock` or `/tmp/brightwing-authd.sock`
- Windows: `\\.\pipe\brightwing-authd`

**Daemon lifecycle:**
```
┌─────────────────────────────────────────────────────────────┐
│  System Login                                                │
│      │                                                       │
│      ▼                                                       │
│  brightwing-authd starts (launchd/systemd/Task Scheduler)    │
│      │                                                       │
│      ├── Opens Stronghold vault (encrypted at rest)          │
│      ├── Binds IPC socket                                    │
│      ├── Starts token refresh scheduler                      │
│      │                                                       │
│      │   ┌───── brightwing-proxy (spawned by Claude Desktop) │
│      │   │      GET_CREDENTIALS("github-mcp")                │
│      │   │                                                   │
│      │◄──┘      → Returns cached/refreshed credentials       │
│      │                                                       │
│      │   ┌───── brightwing-proxy (spawned by Cursor)         │
│      │   │      GET_CREDENTIALS("github-mcp")                │
│      │   │                                                   │
│      │◄──┘      → Returns same cached credentials            │
│      │                                                       │
│      │   (Token approaching expiry)                          │
│      │   → Refresh OAuth token automatically                 │
│      │   → Update Stronghold                                 │
│      │   → Next proxy request gets fresh token               │
│      │                                                       │
│      │   (Refresh fails — user needs to re-auth)             │
│      │   → Mark server as "needs_reauth" in SQLite           │
│      │   → Send desktop notification via notify-rust         │
│      │   → Next proxy request gets auth_expired error        │
│      │   → User opens Brightwing GUI, re-authenticates       │
│      │                                                       │
│  System Shutdown                                             │
│      │                                                       │
│      ▼                                                       │
│  brightwing-authd exits cleanly                              │
└─────────────────────────────────────────────────────────────┘
```

### Component 3: OAuth Flow Handler

Handles the OAuth 2.1 authorization code flow with PKCE, entirely within the Tauri GUI app.

**Flow:**
```
1. User clicks "Connect" on an OAuth-requiring MCP server in Brightwing UI
2. Brightwing generates PKCE code verifier + challenge
3. Opens system browser to authorization URL:
   https://auth.example.com/authorize?
     client_id=...
     redirect_uri=http://localhost:{port}/callback
     response_type=code
     code_challenge=...
     code_challenge_method=S256
     scope=...
4. Brightwing starts a temporary localhost HTTP server on a random port
5. User authenticates in browser, grants consent
6. Browser redirects to http://localhost:{port}/callback?code=...
7. Brightwing exchanges code for tokens (access_token + refresh_token)
8. Tokens stored in Stronghold vault
9. Localhost server shuts down
10. UI updates to show "Connected ✓"
```

**OAuth metadata discovery:**
Many MCP servers support OAuth 2.0 Authorization Server Metadata (RFC 8414). The flow should:
1. Try `GET {server_url}/.well-known/oauth-authorization-server` first
2. Fall back to manual configuration if discovery fails
3. Cache discovered metadata in SQLite

**OAuth data model (stored in Stronghold):**
```
Vault key: oauth:{server_id}
Value (JSON):
{
  "access_token": "...",
  "refresh_token": "...",
  "token_type": "Bearer",
  "expires_at": "2026-03-10T14:30:00Z",
  "scope": "read write",
  "authorization_server": "https://auth.example.com",
  "token_endpoint": "https://auth.example.com/token",
  "client_id": "brightwing-mcp-manager"
}
```

**OAuth metadata (stored in SQLite, not secrets):**
```sql
CREATE TABLE oauth_server_meta (
    server_id TEXT PRIMARY KEY,
    authorization_endpoint TEXT NOT NULL,
    token_endpoint TEXT NOT NULL,
    registration_endpoint TEXT,       -- For dynamic client registration
    client_id TEXT,                    -- May be dynamic or pre-registered
    scopes TEXT,                      -- Space-separated scope list
    auth_method TEXT DEFAULT 'pkce',  -- 'pkce', 'client_secret', 'none'
    discovered_at TEXT,
    updated_at TEXT DEFAULT (datetime('now'))
);
```

**Dynamic Client Registration (RFC 7591):**
Some MCP servers support dynamic client registration. Brightwing should:
1. Check if the server's metadata includes a `registration_endpoint`
2. If so, register itself as a client automatically
3. Store the obtained `client_id` and `client_secret` in the vault
4. This eliminates the need for pre-registered client IDs per server

### Component 4: Credential Store (Enhanced Vault)

Builds on the existing `plans/auth.md` vault design. The credential store now handles three types of credentials:

| Type | Storage Key Pattern | Contents |
|------|-------------------|----------|
| **API Keys** | `secret:{server_id}:{env_var}` | Single environment variable value |
| **OAuth Tokens** | `oauth:{server_id}` | Access token, refresh token, expiry, metadata |
| **Server Connection** | `connection:{server_id}` | URL, transport type, command/args (for stdio upstream) |

**Enhanced schema (SQLite metadata):**
```sql
-- Extends the api_key_meta table from auth.md plan
-- Adds proxy-specific tracking

CREATE TABLE proxy_servers (
    server_id TEXT PRIMARY KEY,           -- "github-mcp"
    display_name TEXT NOT NULL,           -- "GitHub MCP Server"
    upstream_transport TEXT NOT NULL,     -- "http", "sse", "stdio"
    upstream_url TEXT,                    -- For http/sse: "https://github-mcp.example.com/mcp"
    upstream_command TEXT,                -- For stdio: "npx"
    upstream_args TEXT,                   -- For stdio: JSON array '["@modelcontextprotocol/server-github"]'
    auth_type TEXT NOT NULL,             -- "oauth", "api_key", "none"
    auth_status TEXT DEFAULT 'pending',  -- "pending", "connected", "expired", "error"
    proxy_enabled INTEGER DEFAULT 1,     -- Whether proxy is active
    scoreboard_uuid TEXT,                -- Link to Scoreboard server if applicable
    last_used_at TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Track which tools have this proxy installed
CREATE TABLE proxy_tool_installs (
    server_id TEXT NOT NULL,
    tool_id TEXT NOT NULL,
    installed_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (server_id, tool_id),
    FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id)
);
```

### Component 5: Proxy Config Writer (Modified Install Flow)

When a user enables a proxied server for a tool, instead of writing the upstream server's config, Brightwing writes a proxy config:

**Before (direct install):**
```json
{
  "mcpServers": {
    "github-mcp": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_TOKEN": "ghp_plaintext_secret_in_config"
      }
    }
  }
}
```

**After (proxy install):**
```json
{
  "mcpServers": {
    "bw-github-mcp": {
      "command": "brightwing-proxy",
      "args": ["--server", "github-mcp"]
    }
  }
}
```

**Config writer changes:**
- New function: `write_proxy_config(tool_id, server_id)` — writes the proxy entry instead of the direct config
- The `bw-` prefix makes it clear which servers are Brightwing-managed
- For CLI tools (Claude Code): `claude mcp add-json bw-github-mcp '{"command":"brightwing-proxy","args":["--server","github-mcp"]}'`
- The proxy binary must be on PATH — installer adds it to a well-known location:
  - macOS: `~/.local/bin/brightwing-proxy` (or `/usr/local/bin/`)
  - Windows: `%LOCALAPPDATA%\Brightwing\bin\brightwing-proxy.exe`
  - Linux: `~/.local/bin/brightwing-proxy`

---

## User Experience Flow

### Flow 1: Adding an OAuth MCP Server

```
┌─────────────────────────────────────────────────────────────┐
│  1. User searches for "GitHub" in Brightwing                │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ 🔍 github                                              │  │
│  │                                                        │  │
│  │  github-mcp-server               A+ 95                │  │
│  │  by modelcontextprotocol                               │  │
│  │  ★ 1,234 installs                                     │  │
│  │  Auth: OAuth 🔒                                        │  │
│  │                                                        │  │
│  │  [Add to Brightwing]                                   │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  2. OAuth authentication flow                                │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Connect to GitHub MCP Server                           │  │
│  │                                                        │  │
│  │ This server requires OAuth authentication.             │  │
│  │ You'll be redirected to GitHub to authorize access.    │  │
│  │                                                        │  │
│  │ Brightwing will securely store your credentials and    │  │
│  │ handle token refresh automatically.                    │  │
│  │                                                        │  │
│  │ [Connect with GitHub]                      [Cancel]    │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  → Opens browser → GitHub login → Consent → Callback        │
│  → Tokens stored in encrypted vault                          │
│  → UI shows "Connected ✓"                                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  3. Choose which tools get the proxy                         │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ github-mcp-server                     Connected ✓      │  │
│  │                                                        │  │
│  │ Enable for:                                            │  │
│  │ ☑ Claude Desktop                                       │  │
│  │ ☑ Cursor                                               │  │
│  │ ☑ Claude Code                                          │  │
│  │ ☐ VS Code                                              │  │
│  │ ☐ Windsurf                                             │  │
│  │                                                        │  │
│  │ Each tool will receive a local proxy connection.       │  │
│  │ No API keys are written to config files.               │  │
│  │                                                        │  │
│  │ [Enable Selected]                         [Cancel]     │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  → Writes proxy config to each tool's config file            │
│  → Shows restart banner for GUI tools                        │
└─────────────────────────────────────────────────────────────┘
```

### Flow 2: Adding an API Key MCP Server

```
┌─────────────────────────────────────────────────────────────┐
│  1. User installs from Scoreboard (as today)                 │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ brave-search                            A  89          │  │
│  │ Auth: API Key 🔑                                       │  │
│  │                                                        │  │
│  │ BRAVE_API_KEY: [________________________]              │  │
│  │ Get your key at: https://api.search.brave.com          │  │
│  │                                                        │  │
│  │ ☑ Use Brightwing Proxy (recommended)                   │  │
│  │   Credentials stored securely, not in config files     │  │
│  │                                                        │  │
│  │ ☐ Direct install (legacy)                              │  │
│  │   API key written to each tool's config file           │  │
│  │                                                        │  │
│  │ [Install]                                 [Cancel]     │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Flow 3: Dashboard with Proxy Indicators

```
┌─────────────────────────────────────────────────────────────┐
│  Dashboard                                                   │
│  ───────────────────────────────────────────────────────────  │
│                                                              │
│  Server               │ Auth  │ CD │ CU │ CC │ VC │ WS     │
│  ─────────────────────┼───────┼────┼────┼────┼────┼────     │
│  🔒 bw-github-mcp     │ ✓ OAuth│ ☑  │ ☑  │ ☑  │ ☐  │ ☐     │
│  🔑 bw-brave-search   │ ✓ Key │ ☑  │ ☑  │ ☐  │ ☑  │ ☐     │
│  🔒 bw-linear         │ ⚠ Exp │ ☑  │ ☐  │ ☑  │ ☐  │ ☐     │
│     filesystem         │ None  │ ☑  │ ☑  │ ☑  │ ☑  │ ☑     │
│  ─────────────────────┼───────┼────┼────┼────┼────┼────     │
│                                                              │
│  🔒 = Proxied (OAuth)   🔑 = Proxied (API Key)              │
│  ⚠ = Needs re-authentication                                │
│  (unlabeled) = Direct install (no proxy)                     │
│                                                              │
│  Legend: Proxied servers route through Brightwing.            │
│  Credentials never appear in tool config files.              │
└─────────────────────────────────────────────────────────────┘
```

---

## Data Model

### SQLite Schema (New Tables)

```sql
-- Servers managed through the Brightwing proxy
CREATE TABLE proxy_servers (
    server_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    upstream_transport TEXT NOT NULL,       -- "http", "sse", "stdio"
    upstream_url TEXT,                      -- For remote: full URL
    upstream_command TEXT,                  -- For stdio: binary/script to run
    upstream_args TEXT,                     -- For stdio: JSON array of args
    auth_type TEXT NOT NULL,               -- "oauth", "api_key", "none"
    auth_status TEXT DEFAULT 'pending',    -- "pending", "connected", "expired", "error"
    proxy_enabled INTEGER DEFAULT 1,
    scoreboard_uuid TEXT,
    last_used_at TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- OAuth server metadata (non-secret)
CREATE TABLE oauth_server_meta (
    server_id TEXT PRIMARY KEY,
    authorization_endpoint TEXT NOT NULL,
    token_endpoint TEXT NOT NULL,
    registration_endpoint TEXT,
    client_id TEXT,
    scopes TEXT,
    auth_method TEXT DEFAULT 'pkce',
    discovered_at TEXT,
    updated_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id)
);

-- Which tools have a proxy config installed
CREATE TABLE proxy_tool_installs (
    server_id TEXT NOT NULL,
    tool_id TEXT NOT NULL,
    config_key TEXT NOT NULL,              -- "bw-github-mcp" (what's in the config file)
    installed_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (server_id, tool_id),
    FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id)
);

-- Daemon state tracking
CREATE TABLE authd_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT DEFAULT (datetime('now'))
);
-- Keys: "last_started", "pid", "socket_path", "version"
```

### Stronghold Vault Key Patterns

```
secret:{server_id}:{env_var}           → API key values (unchanged from auth.md)
oauth:{server_id}                       → OAuth token bundle (access, refresh, expiry)
oauth_client:{server_id}                → Dynamic client registration credentials
connection:{server_id}                  → Server connection metadata
```

---

## Tauri Commands (New)

### Proxy Management
```rust
/// Register a new proxy server (after auth is complete).
#[tauri::command]
fn register_proxy_server(
    server_id: String,
    display_name: String,
    upstream_transport: String,          // "http", "sse", "stdio"
    upstream_url: Option<String>,
    upstream_command: Option<String>,
    upstream_args: Option<Vec<String>>,
    auth_type: String,                   // "oauth", "api_key", "none"
    scoreboard_uuid: Option<String>,
    db: State<Database>,
) -> Result<(), String>

/// List all registered proxy servers with their auth status.
#[tauri::command]
fn list_proxy_servers(db: State<Database>) -> Result<Vec<ProxyServer>, String>

/// Install proxy config into a specific tool.
#[tauri::command]
fn install_proxy_to_tool(
    server_id: String,
    tool_id: String,
    db: State<Database>,
) -> Result<InstallResult, String>

/// Remove proxy config from a specific tool.
#[tauri::command]
fn remove_proxy_from_tool(
    server_id: String,
    tool_id: String,
    db: State<Database>,
) -> Result<InstallResult, String>

/// Unregister a proxy server entirely (removes from all tools + vault).
#[tauri::command]
fn unregister_proxy_server(
    server_id: String,
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<(), String>

/// Get auth status for a proxy server.
#[tauri::command]
fn get_proxy_auth_status(
    server_id: String,
    db: State<Database>,
) -> Result<AuthStatus, String>
```

### OAuth Flow
```rust
/// Initiate OAuth flow for a server. Returns the authorization URL.
#[tauri::command]
async fn start_oauth_flow(
    server_id: String,
    server_url: String,
    db: State<Database>,
) -> Result<OAuthFlowInfo, String>
// OAuthFlowInfo { auth_url: String, state: String, port: u16 }

/// Called by the OAuth callback handler when the browser redirects back.
/// Exchanges the auth code for tokens and stores them.
#[tauri::command]
async fn complete_oauth_flow(
    server_id: String,
    auth_code: String,
    state: String,
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<(), String>

/// Force-refresh an OAuth token (for testing or manual recovery).
#[tauri::command]
async fn refresh_oauth_token(
    server_id: String,
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<(), String>

/// Discover OAuth metadata from a server URL.
#[tauri::command]
async fn discover_oauth_metadata(
    server_url: String,
) -> Result<OAuthMetadata, String>
```

### Daemon Management
```rust
/// Start the auth daemon if not already running.
#[tauri::command]
fn start_auth_daemon(db: State<Database>) -> Result<DaemonStatus, String>

/// Stop the auth daemon.
#[tauri::command]
fn stop_auth_daemon(db: State<Database>) -> Result<(), String>

/// Get daemon status (running, pid, uptime).
#[tauri::command]
fn get_daemon_status(db: State<Database>) -> Result<DaemonStatus, String>

/// Install daemon for auto-start on login.
#[tauri::command]
fn install_daemon_autostart() -> Result<(), String>

/// Remove daemon from auto-start.
#[tauri::command]
fn uninstall_daemon_autostart() -> Result<(), String>
```

---

## TypeScript Layer

```typescript
// src/lib/types.ts — New types

export interface ProxyServer {
  server_id: string;
  display_name: string;
  upstream_transport: "http" | "sse" | "stdio";
  upstream_url: string | null;
  auth_type: "oauth" | "api_key" | "none";
  auth_status: "pending" | "connected" | "expired" | "error";
  proxy_enabled: boolean;
  scoreboard_uuid: string | null;
  last_used_at: string | null;
  tool_installs: string[];  // tool IDs where proxy is installed
}

export interface OAuthFlowInfo {
  auth_url: string;
  state: string;
  port: number;
}

export interface DaemonStatus {
  running: boolean;
  pid: number | null;
  uptime_seconds: number | null;
  socket_path: string;
  version: string;
}

export interface AuthStatus {
  type: "oauth" | "api_key" | "none";
  status: "pending" | "connected" | "expired" | "error";
  expires_at: string | null;          // For OAuth tokens
  error_message: string | null;       // If status is "error"
  last_refreshed_at: string | null;
}
```

---

## Proxy Binary Path & Distribution

### Installation Location

The `brightwing-proxy` binary must be discoverable by AI tools, which means it must be on PATH or at a known absolute path.

**Strategy: Install to a well-known location and add to PATH**

| Platform | Binary Path | PATH Addition |
|----------|------------|---------------|
| macOS | `~/.local/bin/brightwing-proxy` | `~/.local/bin` (add to shell profile if needed) |
| Windows | `%LOCALAPPDATA%\Brightwing\bin\brightwing-proxy.exe` | Add to User PATH env var |
| Linux | `~/.local/bin/brightwing-proxy` | `~/.local/bin` (typically already on PATH) |

**Alternative: Use absolute paths in configs**
Instead of relying on PATH, write absolute paths in tool configs:
```json
{
  "mcpServers": {
    "bw-github": {
      "command": "/Users/keyton/.local/bin/brightwing-proxy",
      "args": ["--server", "github-mcp"]
    }
  }
}
```
This is more reliable but less portable. Use this as a fallback if PATH detection fails.

**Binary extraction:**
The Tauri app bundles the proxy binary as a sidecar or resource. On first run (or after update), it extracts/copies the binary to the install location and sets execute permissions.

### Updating the Proxy Binary

When Brightwing updates, the proxy binary must also update:
1. New proxy binary is bundled with the Tauri update
2. On app launch after update, compare versions
3. If proxy binary is outdated, copy new version to install location
4. Running proxy instances continue with old version until their process exits
5. New proxy instances pick up the new binary automatically

---

## Handling Stdio Upstream Servers

Not all MCP servers are remote. For stdio servers that need API keys (e.g., `npx @modelcontextprotocol/server-github` with `GITHUB_TOKEN`), the proxy operates differently:

```
AI Tool → stdio → brightwing-proxy → stdio → upstream MCP server
                       │
                       │ (injects env vars from vault)
                       │
                       └→ spawns: GITHUB_TOKEN=ghp_xxx npx -y @modelcontextprotocol/server-github
```

**The proxy acts as a process supervisor:**
1. Fetches credentials from auth service
2. Spawns the upstream command with secrets as environment variables
3. Pipes stdin/stdout between the AI tool and the upstream process
4. Monitors the upstream process for crashes and restarts if needed

This means even for `npx`/`uvx` servers, the API key never appears in any config file. The proxy binary is the only process that ever holds the decrypted secret, and only in memory.

---

## Migration Path: Direct Install → Proxy

Users with existing direct installs should be able to migrate smoothly:

### Migration Flow
```
┌─────────────────────────────────────────────────────────────┐
│  Brightwing detects existing direct installs with env vars   │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Upgrade to Secure Proxy                                │  │
│  │                                                        │  │
│  │ 3 servers have API keys in plaintext config files:     │  │
│  │                                                        │  │
│  │ ☑ github-mcp (GITHUB_TOKEN in 3 tools)                │  │
│  │ ☑ brave-search (BRAVE_API_KEY in 2 tools)             │  │
│  │ ☐ filesystem (no secrets, no proxy needed)             │  │
│  │                                                        │  │
│  │ Migrating will:                                        │  │
│  │ 1. Import API keys into Brightwing's encrypted vault   │  │
│  │ 2. Replace direct configs with proxy configs           │  │
│  │ 3. Remove plaintext keys from tool config files        │  │
│  │                                                        │  │
│  │ [Migrate Selected]          [Skip for now]             │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Migration Steps (Per Server)
1. Read current config from the tool's config file
2. Extract env vars (secrets)
3. Store secrets in Stronghold vault
4. Register as a proxy server in SQLite
5. Replace config entry with proxy config
6. Verify proxy connectivity
7. If verification fails, rollback to original config

---

## Coexistence: Proxy + Direct Install

Not all servers need proxying. The system should support both modes:

| Server Type | Mode | Rationale |
|------------|------|-----------|
| Remote + OAuth | Proxy (required) | Only Brightwing handles the OAuth flow |
| Remote + API Key | Proxy (recommended) | Keeps secrets out of config files |
| Local + API Key | Proxy (recommended) | Same benefit, env injection |
| Local + No Auth | Direct install | No benefit to proxying, just adds overhead |

**UI indicator:**
In the dashboard grid, proxied servers are visually distinct from direct installs. Users can choose per-server whether to use the proxy or direct install.

---

## Security Model

### Threat Model

| Threat | Mitigation |
|--------|-----------|
| **Config file exposure** (backup, git commit, screenshot) | Proxy configs contain no secrets. Only `brightwing-proxy --server <id>`. |
| **Vault compromise** (disk theft, malware) | Stronghold encryption with Argon2-derived key. OS keychain integration. |
| **IPC eavesdropping** (local attacker reads Unix socket) | Socket file permissions set to `0600` (owner-only). PID verification on connection. |
| **Proxy binary tampering** | Binary checksum verified by daemon on startup. Code signing on distributed builds. |
| **Token theft from memory** | Tokens held in memory only during active request. Process isolation per proxy instance. |
| **Expired token abuse** | Refresh tokens stored encrypted. Access tokens have short TTL. Automatic refresh with immediate vault update. |
| **Rogue MCP server** | Brightwing can integrate Scoreboard quality scores as a trust signal before proxying. |

### IPC Security
- Unix socket with `0600` permissions (only the user can connect)
- Optional: Verify connecting PID is a known `brightwing-proxy` binary
- All IPC traffic is local-only (no network exposure)
- Consider adding a per-session nonce that the daemon generates and the proxy must present

---

## Implementation Phases

### Phase 1: Foundation — Proxy Binary & IPC (Weeks 1-3)

**Goal:** A working `brightwing-proxy` binary that can proxy a single HTTP MCP server with a hardcoded API key.

1. **Create `brightwing-proxy` binary target** in `src-tauri/Cargo.toml`
   - Separate `[[bin]]` target in the same workspace
   - Dependencies: `tokio`, `serde_json`, `reqwest`, `tokio-tungstenite` (for SSE)

2. **Implement stdio ↔ HTTP proxy core**
   - JSON-RPC message parser for stdin
   - HTTP client for forwarding to upstream
   - Response relay back to stdout
   - MCP protocol compliance (initialize, ping, tools/list, tools/call, etc.)

3. **Implement IPC client in proxy**
   - Unix domain socket client (macOS/Linux)
   - Named pipe client (Windows)
   - `GET_CREDENTIALS` request/response

4. **Implement IPC server (minimal, in-process)**
   - Start as part of Tauri app for now (daemon comes later)
   - Serve credentials from Stronghold
   - Handle concurrent proxy connections

5. **Binary extraction and PATH setup**
   - Bundle proxy binary as Tauri sidecar
   - Extract to `~/.local/bin/` on first run
   - Verify PATH accessibility

6. **Basic integration test**
   - Mock MCP server (HTTP)
   - Proxy binary connects via IPC, gets credentials, proxies traffic
   - Verify end-to-end message flow

**Files created:**
- `src-tauri/src/bin/brightwing_proxy.rs`
- `src-tauri/src/proxy/mod.rs` (shared proxy types)
- `src-tauri/src/proxy/ipc_server.rs`
- `src-tauri/src/proxy/ipc_client.rs`
- `src-tauri/src/proxy/transport.rs` (stdio ↔ HTTP translation)

### Phase 2: Credential Management (Weeks 3-5)

**Goal:** Full vault integration with API key and OAuth token storage. Builds on `plans/auth.md` Phase 1-2.

1. **Implement Stronghold integration** (from auth.md plan)
   - `tauri-plugin-stronghold` setup
   - Key storage/retrieval API
   - `api_key_meta` and `proxy_servers` SQLite tables

2. **API Key flow for proxy servers**
   - UI for entering API keys during proxy server setup
   - Store in Stronghold, register in `proxy_servers`
   - Serve via IPC to proxy instances

3. **Proxy config writer**
   - New install mode: "proxy" vs "direct"
   - Write `brightwing-proxy` configs to tool config files
   - Handle all config formats (JSON, TOML, CLI)

4. **API Keys panel** (from auth.md plan)
   - View, edit, delete stored keys
   - Eye toggle for reveal
   - Export/import

5. **Dashboard integration**
   - Proxy status indicators in the grid
   - Auth status badges (connected, expired, pending)

**Files created/modified:**
- `src-tauri/src/vault.rs` (Stronghold wrapper — from auth.md)
- `src/components/ApiKeys.tsx` (from auth.md)
- `src/components/ProxySetup.tsx` (new — proxy server add flow)
- `src-tauri/src/config/writer.rs` (modified — proxy config mode)
- `src/components/Dashboard.tsx` (modified — proxy indicators)

### Phase 3: OAuth Integration (Weeks 5-8)

**Goal:** Full OAuth 2.1 PKCE flow for remote MCP servers.

1. **OAuth metadata discovery**
   - `GET /.well-known/oauth-authorization-server` probing
   - Fallback to manual configuration
   - `oauth_server_meta` SQLite table

2. **Authorization flow**
   - PKCE code verifier/challenge generation
   - Temporary localhost HTTP server for callback
   - Browser launch for authorization
   - Code-to-token exchange
   - Token storage in Stronghold

3. **Dynamic Client Registration (RFC 7591)**
   - Auto-register Brightwing with servers that support it
   - Store client credentials in vault

4. **Token refresh lifecycle**
   - Background timer for proactive refresh (before expiry)
   - Reactive refresh on 401 response from upstream
   - Update vault and notify active proxies

5. **Re-authentication flow**
   - Detect expired/revoked tokens
   - Desktop notification prompting user to re-auth
   - Seamless re-auth in Brightwing GUI

6. **OAuth UI**
   - "Connect" button for OAuth servers
   - Auth status display (connected, expires in X hours, expired)
   - "Disconnect" / "Re-authenticate" actions

**Files created:**
- `src-tauri/src/oauth/mod.rs`
- `src-tauri/src/oauth/discovery.rs` (metadata discovery)
- `src-tauri/src/oauth/flow.rs` (PKCE flow)
- `src-tauri/src/oauth/refresh.rs` (token lifecycle)
- `src-tauri/src/oauth/registration.rs` (dynamic client reg)
- `src/components/OAuthConnect.tsx`

### Phase 4: Background Daemon (Weeks 8-10)

**Goal:** Standalone background daemon so proxies work even when the GUI is closed.

1. **Extract auth service into standalone binary**
   - `brightwing-authd` — minimal Rust binary
   - Stronghold vault access
   - IPC server (Unix socket / named pipe)
   - Token refresh scheduler

2. **Daemon lifecycle management**
   - Auto-start on login:
     - macOS: `~/Library/LaunchAgents/com.brightwing.authd.plist`
     - Windows: Task Scheduler entry
     - Linux: `~/.config/systemd/user/brightwing-authd.service`
   - Graceful shutdown on system shutdown
   - Health check endpoint on IPC socket

3. **GUI ↔ Daemon coordination**
   - GUI detects running daemon, delegates IPC serving
   - GUI can restart/stop daemon
   - GUI writes to vault, daemon picks up changes

4. **Daemon status UI**
   - Status indicator in Brightwing UI (daemon running/stopped)
   - Start/stop controls
   - Auto-start toggle
   - Log viewer

**Files created:**
- `src-tauri/src/bin/brightwing_authd.rs`
- `src-tauri/src/daemon/mod.rs`
- `src-tauri/src/daemon/lifecycle.rs`
- `src-tauri/src/daemon/scheduler.rs` (token refresh timer)
- `resources/com.brightwing.authd.plist` (macOS)
- `resources/brightwing-authd.service` (Linux)
- `src/components/DaemonStatus.tsx`

### Phase 5: stdio Upstream Proxy (Weeks 10-11)

**Goal:** Proxy stdio MCP servers (e.g., `npx` commands) with credential injection.

1. **Process supervisor in proxy binary**
   - Spawn upstream command with injected env vars
   - Pipe stdin/stdout bidirectionally
   - Monitor for crashes, clean up child process

2. **Env var injection**
   - Fetch secrets from auth service
   - Set as environment variables on child process
   - Clear from proxy's own environment after spawn

3. **Process lifecycle**
   - Handle upstream process exit (report error via MCP)
   - Handle proxy process exit (clean up child)
   - Signal forwarding (SIGTERM, SIGINT)

**Files modified:**
- `src-tauri/src/bin/brightwing_proxy.rs` (add stdio upstream mode)
- `src-tauri/src/proxy/transport.rs` (add stdio ↔ stdio mode)

### Phase 6: Migration & Polish (Weeks 11-13)

**Goal:** Migration from direct installs, UX polish, documentation.

1. **Migration assistant**
   - Scan existing tool configs for servers with env vars
   - Offer to import into vault and replace with proxy configs
   - Rollback capability

2. **Scoreboard integration**
   - Use Scoreboard's install config to pre-fill upstream transport/URL/command
   - Auth type detection from Scoreboard metadata
   - One-click "Add + Authenticate + Install to All Tools" flow

3. **Proxy health monitoring**
   - Periodic connectivity checks to upstream servers
   - Status page showing proxy health per server
   - Latency metrics

4. **Error recovery**
   - Daemon crash recovery (auto-restart via launchd/systemd)
   - Proxy crash recovery (AI tools re-spawn automatically)
   - Vault corruption detection and recovery from backup

5. **Documentation**
   - User guide: adding servers, OAuth flow, troubleshooting
   - Architecture docs for contributors
   - FAQ: "Why does Brightwing need a background service?"

---

## Cargo Workspace Structure

```toml
# src-tauri/Cargo.toml

[workspace]
members = [".", "crates/proxy-common"]

[[bin]]
name = "brightwing-proxy"
path = "src/bin/brightwing_proxy.rs"

[[bin]]
name = "brightwing-authd"
path = "src/bin/brightwing_authd.rs"

[dependencies]
# ... existing deps ...
tauri-plugin-stronghold = "2"
tokio = { version = "1", features = ["full"] }

# Shared between main app, proxy, and daemon
proxy-common = { path = "crates/proxy-common" }

[profile.dev.package.scrypt]
opt-level = 3  # Prevent slow debug builds with Stronghold
```

```
src-tauri/
├── src/
│   ├── main.rs                    # Tauri app entry (unchanged)
│   ├── lib.rs                     # Tauri commands (extended)
│   ├── config/                    # Config reader/writer (modified)
│   ├── db/                        # SQLite (extended with proxy tables)
│   ├── tools/                     # Tool scanner (unchanged)
│   ├── deeplink/                  # Deep links (unchanged)
│   ├── vault.rs                   # Stronghold wrapper (new)
│   ├── oauth/                     # OAuth flows (new)
│   │   ├── mod.rs
│   │   ├── discovery.rs
│   │   ├── flow.rs
│   │   ├── refresh.rs
│   │   └── registration.rs
│   ├── proxy/                     # Proxy infrastructure (new)
│   │   ├── mod.rs
│   │   ├── ipc_server.rs
│   │   ├── ipc_client.rs
│   │   └── transport.rs
│   ├── daemon/                    # Daemon management (new)
│   │   ├── mod.rs
│   │   ├── lifecycle.rs
│   │   └── scheduler.rs
│   └── bin/
│       ├── brightwing_proxy.rs    # Proxy binary (new)
│       └── brightwing_authd.rs    # Auth daemon binary (new)
│
├── crates/
│   └── proxy-common/              # Shared types between components
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── ipc.rs             # IPC protocol types
│           └── credentials.rs     # Credential types
│
└── resources/
    ├── com.brightwing.authd.plist          # macOS launchd config
    ├── brightwing-authd.service            # Linux systemd user unit
    └── brightwing-authd-task.xml           # Windows Task Scheduler
```

---

## Technology Choices

### Why Rust for Everything (Not Python or Node.js)

| Factor | Rust | Python | Node.js |
|--------|------|--------|---------|
| **Startup time** | <50ms | 200-500ms | 100-300ms |
| **Memory per proxy** | ~5MB | ~30MB | ~25MB |
| **Runtime dependency** | None (static binary) | Python interpreter | Node.js runtime |
| **Already in stack** | Yes (Tauri backend) | No | Yes (frontend tooling) |
| **Stronghold access** | Native (same crate) | FFI required | FFI required |
| **Cross-platform** | Excellent | Good | Good |

Rust is the clear choice: zero runtime dependency (critical for a binary that AI tools spawn), tiny footprint (users may have 5+ proxy instances running), and native access to the Stronghold vault that the Tauri app already uses.

### Key Rust Crates

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime for proxy and daemon |
| `reqwest` | HTTP client for upstream MCP servers |
| `tokio-tungstenite` | WebSocket client for SSE transport |
| `serde_json` | JSON-RPC parsing |
| `tauri-plugin-stronghold` | Encrypted credential storage |
| `interprocess` | Cross-platform IPC (Unix sockets + named pipes) |
| `notify-rust` | Desktop notifications (for re-auth prompts) |
| `tracing` | Structured logging for proxy and daemon |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| **Daemon complexity** — managing a background service adds significant engineering and support burden | High | High | Start with in-process IPC (Phase 1-3), only extract daemon in Phase 4. Validate the proxy concept works before committing to daemon architecture. |
| **PATH issues** — proxy binary not found by AI tools, especially on macOS where GUI apps have limited PATH | High | High | Use absolute paths in config files as primary strategy. Only fall back to PATH-based resolution when needed. |
| **OAuth server diversity** — each MCP server's OAuth implementation has quirks | Medium | High | Start with well-documented servers (GitHub, Google, Slack). Build a compatibility database. Log issues for community contribution. |
| **Token refresh races** — multiple proxy instances refreshing the same token simultaneously | Medium | Medium | Token refresh is only done by the daemon/auth service, never by proxy instances directly. Mutex around refresh operations. |
| **Tauri sidecar bundling** — packaging multiple binaries with Tauri has platform-specific gotchas | Medium | Medium | Test thoroughly on all platforms. Fall back to resource extraction if sidecar approach has issues. |
| **User confusion** — "Why do I need a background service?" | Medium | Low | Clear onboarding UI explaining the benefit. Make it optional — users can still do direct installs. |
| **Docker MCP Gateway competition** — Docker adds similar proxy features | Low | Medium | Docker requires Docker Desktop (heavy). Brightwing is lightweight. Different market segment. |
| **Stronghold deprecation in Tauri v3** | Low | Medium | Abstract vault access behind a trait. Can swap implementations when Tauri v3 ships. |

---

## Success Metrics

### Phase 1 Launch (Foundation)
- brightwing-proxy binary successfully proxies HTTP MCP servers
- IPC credential retrieval works on macOS, Windows, Linux
- End-to-end test: Claude Desktop → proxy → authenticated upstream server

### Phase 3 Launch (OAuth)
- At least 5 OAuth-based MCP servers tested and working
- Token refresh operates correctly for 7+ day sessions
- Zero plaintext secrets in any tool config file for proxied servers

### Phase 4 Launch (Daemon)
- Daemon survives GUI close, system sleep/wake, and process crashes
- Auto-start works on all three platforms
- <100ms latency overhead vs direct connection

### Phase 6 Launch (Full)
- Migration assistant tested with configs containing 10+ servers
- User documentation complete
- Scoreboard integration: one-click "Add + Auth + Install" flow

### 3 Months Post-Launch
- 50%+ of Brightwing users have at least one proxied server
- Average of 3+ proxied servers per active user
- Zero reported credential leaks from config files
- Net Promoter Score improvement vs pre-proxy baseline

---

## Relationship to Existing Plans

### `plans/auth.md` (API Key Vault & OAuth Detection)

The central auth proxy plan **supersedes and extends** the auth.md plan:

| auth.md Feature | Status in Central Auth |
|----------------|----------------------|
| Stronghold vault for API keys | **Kept as-is.** Foundation for credential storage. |
| `api_key_meta` SQLite table | **Kept as-is.** Extended with `proxy_servers` table. |
| API Keys panel UI | **Kept as-is.** Enhanced with proxy status indicators. |
| Key capture during install | **Modified.** Now defaults to proxy mode instead of injecting into tool configs. |
| OAuth detection heuristic | **Replaced.** OAuth is now handled directly, not just detected. |
| OAuth "guidance" (tell user to auth in each tool) | **Replaced.** Brightwing handles OAuth centrally. No per-tool auth needed. |
| Vault export/import | **Kept as-is.** Now includes OAuth tokens as well as API keys. |
| Encrypted config backups | **Kept as-is.** Still important for rollback. |

**Implementation order:** Start with auth.md Phases 1-2 (vault foundation, key capture), then build the proxy on top. The vault is a prerequisite for the proxy.

---

## Open Questions

1. **Proxy naming convention:** Should proxied servers use a `bw-` prefix (e.g., `bw-github-mcp`) or keep the original name? Prefix makes proxied servers obvious but may confuse users who reference server names in prompts.

2. **Connection multiplexing:** If 5 tools all connect to the same upstream server via 5 proxy instances, that's 5 separate upstream connections. Should the daemon multiplex these into a single upstream connection? (Probably not for v1 — adds complexity, and most servers handle multiple connections fine.)

3. **Proxy vs. direct for local servers:** Should Brightwing offer proxy mode for local servers that don't need auth? There's a small latency overhead but it enables centralized logging and monitoring. (Probably opt-in only.)

4. **Scoreboard auth metadata:** Should the Scoreboard API provide OAuth endpoint URLs and client IDs as part of install configs? This would make adding OAuth servers zero-config. (Yes, this should be a future Scoreboard enhancement.)

5. **MCP protocol version pinning:** Should the proxy advertise a specific MCP protocol version to the AI tool, independent of what the upstream server supports? This could help with compatibility across tools. (Probably yes — the proxy can normalize protocol differences.)
