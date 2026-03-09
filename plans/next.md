# Brightwing MCP Manager: Consolidated Development Plan

## Vision

Transform the Brightwing MCP Manager from a **config file editor** into the **central nervous system for MCP on your machine**. Three interconnected capabilities:

1. **Central Auth Proxy** — Brightwing handles all MCP server authentication (OAuth flows, API key storage, token refresh) and exposes authenticated servers to local AI tools as simple stdio MCP proxies. Authenticate once, use everywhere.
2. **CLI Shim (`bw`)** — A fast Rust binary that exposes any MCP server as a shell command for terminal-native agents (Claude Code, Codex, Gemini CLI). Zero context injection cost.
3. **Tool Filtering** — The proxy intercepts `tools/list` responses and returns only user-enabled tools. Context-efficient access for MCP-only clients (Claude Desktop, Cursor, VS Code).

```
TODAY (Config File Editor):
  Claude Desktop → (OAuth/API key) → MCP Server X (remote)
  Cursor         → (OAuth/API key) → MCP Server X (remote)
  Claude Code    → (OAuth/API key) → MCP Server X (remote)
  (each app manages its own auth, user configures keys N times)

PROPOSED (Central Platform):
  Claude Desktop → stdio → brightwing-proxy (filtered tools) ─┐
  Cursor         → stdio → brightwing-proxy (filtered tools)  ├→ Brightwing Auth Layer → MCP Server X
  Claude Code    → `bw github search_repos --query "mcp"`     │   (OAuth tokens, API keys,
  Codex          → `bw jira list_issues --project ENG`        ─┘    token refresh — all managed)
```

---

## Strategic Context

### Why This Matters for the Ecosystem

The MCP security landscape is fragmented. API keys sit in plaintext JSON files scattered across `~/.cursor/mcp.json`, `~/Library/Application Support/Claude/claude_desktop_config.json`, `~/.gemini/settings.json`, and more. Every tool re-implements OAuth (or doesn't). Users configure the same credentials N times for M tools.

Three problems Brightwing solves:

1. **Security:** Plaintext secrets in config files are a liability. One leaked config backup exposes everything.
2. **Friction:** Adding the same MCP server to 5 tools means entering the same API key 5 times, or doing OAuth 5 times.
3. **Context Bloat:** MCP injects full tool schemas into the agent's context. GitHub's 93 tools burn ~55,000 tokens before the agent asks a single question. Four servers stacked together can burn 150K+ tokens on plumbing. Benchmarks show MCP uses 380% more tokens than equivalent CLI tools for the same tasks.

### Competitive Positioning

| Category | Players | Gap |
|----------|---------|-----|
| **Enterprise gateways** | MintMCP, Composio, Kong, Traefik Hub, TrueFoundry | Cloud-deployed, RBAC/SOC 2 focused. Overkill for individual developers. |
| **Developer proxies** | mcp-proxy, MetaMCP, FastMCP | CLI building blocks. No GUI, no credential management, no quality signal. |
| **Docker MCP Gateway** | Docker Desktop | Container-centric. Requires Docker Desktop. Good secrets management but heavy. |

**Brightwing's unique position:** Quality-scored server discovery (24K+ servers via Scoreboard) + one-click install across 8+ tools + centralized auth proxy + encrypted credential vault + zero-context CLI access. Nobody else combines all five.

### The Flywheel

```
mcpscoreboard.com (SEO magnet, 24K server pages)
    → MCP Manager desktop app (daily-use tool on developer machines)
        → Auth proxy makes it sticky (always running, managing tokens)
            → CLI shim + tool filtering make it indispensable
                → PatchworkMCP.com (paid SaaS for server publishers)
```

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
│  │  Auth Service (Rust, standalone daemon from day 1)        ││
│  │  - Token refresh lifecycle                               ││
│  │  - Credential dispensing via local IPC                    ││
│  │  - Tool schema caching + token estimation                ││
│  │  - Tool filter management                                ││
│  │  - Proxy process management                              ││
│  └──────────────────────────────┬───────────────────────────┘│
└─────────────────────────────────┼────────────────────────────┘
                                  │
            ┌─────────────────────┼──────────────────────┐
            │                     │                      │
            ▼                     ▼                      ▼
   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
   │ brightwing-proxy │  │ brightwing-proxy │  │ bw CLI shim     │
   │ (server-a)       │  │ (server-b)       │  │                 │
   │                  │  │                  │  │ bw github       │
   │ stdin ←→ JSON-RPC│  │ stdin ←→ JSON-RPC│  │   search_repos  │
   │ + tool filtering │  │ + tool filtering │  │   --query "mcp" │
   │         ↕        │  │         ↕        │  │                 │
   │ HTTPS + auth     │  │ stdio passthru   │  │ IPC → daemon    │
   │ headers          │  │ + env injection   │  │ → upstream call │
   └────────┬─────────┘  └────────┬─────────┘  └────────┬────────┘
            │                     │                      │
            ▼                     ▼                      ▼
   ┌─────────────────┐  ┌─────────────────┐  (same upstream servers
   │ Remote MCP      │  │ Local MCP       │   via daemon connection
   │ Server A        │  │ Server C        │   pooling)
   │ (GitHub, Slack)  │  │ (filesystem)    │
   └─────────────────┘  └─────────────────┘
```

### What Each AI Tool Sees

**MCP-only clients** (Claude Desktop, Cursor, VS Code) — simple stdio proxy with filtered tools:
```json
{
  "mcpServers": {
    "bw-github": {
      "command": "brightwing-proxy",
      "args": ["--server", "github-mcp"]
    }
  }
}
```

**Shell-capable agents** (Claude Code, Codex, Gemini CLI) — direct CLI invocation:
```bash
bw github search_repos --query "mcp server" --per_page 5
```

Both hit the same daemon, same credential vault, same upstream connections. Two interfaces optimized for two types of clients.

**Why both interfaces?** The stdio proxy is essential — Claude Desktop, Cursor, VS Code Copilot, and Windsurf can only reach external tools through the MCP protocol. They have no shell access. The `bw` CLI shim doesn't replace the proxy; it adds a second interface for agents that live in a terminal. Shell-capable agents don't need MCP's schema injection — they already discover tools via `--help` the way they use `git`, `gh`, `aws`, and `kubectl`. The `bw` binary is agent-agnostic: any agent that can call `exec()` gets access. No integration code, no SDK, no protocol negotiation.

---

## Design Decisions

Decisions made during plan review, prior to implementation:

| # | Decision | Rationale |
|---|----------|-----------|
| **D1** | **Daemon from day 1** — no in-process IPC phase. Build `brightwing-authd` as a standalone daemon from Phase 1. | No interim release planned. In-process IPC would be throwaway work and risks a messy extraction later. |
| **D2** | **`VaultBackend` trait from Phase 1** — abstract Stronghold behind a trait immediately. | Stronghold may be deprecated in Tauri v3. Trait also enables in-memory test impl (`HashMap`-backed) so all vault-dependent code is unit-testable without Tauri runtime. |
| **D3** | **Standalone binaries, not Tauri sidecars.** Build `brightwing-proxy`, `bw`, and `brightwing-authd` as independent `cargo build` targets. Tauri app copies them to `~/.local/bin/` on first run / update. | Sidecars have Tauri-specific naming conventions (`binary-aarch64-apple-darwin`), signing requirements, and config complexity. Standalone is simpler to build, test, and reference from AI tool configs. |
| **D4** | **Config files use absolute paths** as primary strategy. PATH as fallback. | Avoids platform-specific PATH detection issues. `"/Users/keyton/.local/bin/brightwing-proxy"` always works. |
| **D5** | **Mock MCP server built first** in Phase 1, before proxy code. Supports both HTTP and stdio transports. | Every integration test depends on it. Building it first unblocks all downstream testing. |
| **D6** | **Headless OAuth test harness** — automated tests POST directly to the localhost callback, skipping the browser redirect. | Covers the full token-exchange flow. Browser redirect is OS-level, not our code. Manual testing covers the browser portion. |
| **D7** | **IPC protocol versioning** — every IPC handshake includes a `version` field. Daemon rejects incompatible proxy/shim versions with a clear error. | Prevents silent breakage when binaries update at different times. Dashboard flags version mismatches. |
| **D8** | **No graceful degradation** — if daemon is down, proxy fails loudly. No local credential cache. | Credential cache creates a second attack surface. Daemon reliability (auto-restart via launchd/systemd) is the mitigation. Dashboard shows daemon status clearly. |
| **D9** | **Curated profiles are a separate track**, not gating v1 launch. | The code to apply a profile is just `set_tool_filter_bulk`. Scoreboard API integration and community submission are distribution concerns, not core architecture. |
| **D10** | **`bw` arg parsing and output formatting are daemon-independent** — build and test these units early, wire to IPC later. | Reduces Phase 5 scope and gets test coverage on CLI ergonomics sooner. |

---

## Component Design

### Component 1: `brightwing-proxy` Binary

A small, standalone Rust binary that acts as a transparent MCP proxy with tool filtering. Each AI tool spawns one instance per proxied server.

**Responsibilities:**
- Read JSON-RPC messages from stdin (from the AI tool)
- Fetch auth credentials from the Brightwing auth service via IPC
- Intercept `tools/list` responses and apply user-configured tool filters
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
```

**Proxy modes (determined by upstream server config):**

| Upstream Transport | Proxy Behavior |
|-------------------|----------------|
| **Streamable HTTP** | stdio ↔ HTTP. Proxy adds `Authorization: Bearer <token>` header. |
| **SSE** | stdio ↔ SSE. Proxy establishes SSE connection with auth, translates to JSON-RPC on stdio. |
| **stdio (with secrets)** | stdio ↔ stdio. Proxy spawns the upstream command with secrets injected as env vars. Passes JSON-RPC through. |

**Credential retrieval via IPC:**
```
brightwing-proxy                    Brightwing Auth Service
     │                                      │
     │──── GET_CREDENTIALS(server-id) ─────→│
     │                                      │ (looks up in Stronghold,
     │                                      │  refreshes token if needed)
     │←─── {type, token, url, env, ...} ────│
     │                                      │
     │──── GET_TOOL_FILTER(server-id) ─────→│
     │                                      │
     │←─── {enabled_tools: [...]} ─────────│
     │                                      │
     │──── (forward MCP traffic) ──────────→│ (upstream MCP server)
```

**Tool filter interception:**
```rust
async fn relay_response(
    response: JsonRpcResponse,
    filter: &ToolFilter,
) -> JsonRpcResponse {
    if is_tools_list_response(&response) {
        apply_tool_filter(response, filter)
    } else {
        response
    }
}

fn apply_tool_filter(
    mut response: JsonRpcResponse,
    filter: &ToolFilter,
) -> JsonRpcResponse {
    if let Some(tools) = response.result.get_mut("tools") {
        if let Some(arr) = tools.as_array_mut() {
            arr.retain(|tool| {
                let name = tool.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                filter.is_enabled(name)
            });
        }
    }
    response
}
```

**IPC protocol (JSON over Unix socket / named pipe):**
```json
// Request
{"action": "get_credentials", "server_id": "github-mcp"}

// Response (OAuth)
{"type": "oauth", "access_token": "gho_xxx...", "url": "https://github-mcp.example.com/mcp"}

// Response (API key)
{"type": "api_key", "env": {"GITHUB_TOKEN": "ghp_xxx..."}, "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"]}

// Response (error)
{"type": "error", "code": "auth_expired", "message": "OAuth token expired. Please re-authenticate in Brightwing."}
```

**Filter retrieval and live updates:**
```jsonc
// Proxy fetches filter on startup
{ "action": "get_tool_filter", "server_id": "github" }

// Response
{
  "type": "tool_filter",
  "server_id": "github",
  "enabled_tools": ["search_repos", "get_repo", "list_issues"],
  "total_tools": 93,
  "token_estimate_filtered": 2200,
  "token_estimate_full": 55000
}

// Daemon pushes invalidation when user toggles tools in UI
{ "type": "filter_changed", "server_id": "github" }
```

**Error handling:**
- Auth service not running → JSON-RPC error to stdout, clear message to stderr
- Credentials expired and refresh fails → MCP error response indicating re-auth needed
- Upstream server unreachable → standard MCP error response

**Source location:** `src-tauri/src/bin/brightwing_proxy.rs`

### Component 2: `bw` CLI Shim Binary

A single, fast Rust binary installed to PATH. Exposes any MCP server managed by Brightwing as a standard shell command. The broker daemon handles connection pooling, auth, and request routing — the shim is just a thin client.

```
CURRENT: Shell-capable agents pay the MCP context tax
  Claude Code → stdio MCP → schema injection (55K tokens) → tool call → response

PROPOSED: Shell-capable agents call tools directly via CLI
  Claude Code → `bw github search_repos --query "mcp"` → response
  (0 tokens of schema injection. Agent reads --help only when needed.)
```

**CLI interface:**
```
USAGE:
    bw <COMMAND>

COMMANDS:
    list                     List available servers and tools
    <server> <tool>          Call a tool on a server
    help                     Show this help

GLOBAL FLAGS:
    --json                   Output raw JSON instead of formatted text
    --verbose                Print timing and debug info to stderr
    --socket <path>          Override IPC socket path

EXAMPLES:
    bw list                              # All servers and their tools
    bw list github                       # Tools on the github server
    bw github search_repos --help        # Usage for a specific tool
    bw github search_repos --query "mcp" # Call a tool
    bw jira list_issues --project ENG --status open
    bw linear list_teams --json          # Raw JSON output for piping
```

**`bw list` output (human-readable with token budget):**
```
$ bw list

Brightwing Tool Broker — 4 servers connected

  github          5 tools    OAuth ✓     tokens: ~2,200
  jira            12 tools   OAuth ✓     tokens: ~4,800
  brave-search    3 tools    API Key ✓   tokens: ~900
  linear          8 tools    OAuth ✓     tokens: ~3,100

Total MCP context cost if loaded as servers: ~11,000 tokens
Context cost via bw: 0 tokens

Run `bw <server>` to see tools, `bw <server> <tool> --help` for usage.
```

**`bw <server> <tool> --help` output (agent-optimized):**
```
$ bw github search_repos --help

Search GitHub repositories

USAGE:
    bw github search_repos --query <text> [OPTIONS]

REQUIRED:
    --query <text>       Search query string

OPTIONS:
    --per_page <int>     Results per page (default: 10, max: 100)
    --sort <text>        Sort by: stars, forks, updated (default: stars)
    --order <text>       Sort order: asc, desc (default: desc)

EXAMPLE:
    bw github search_repos --query "mcp server" --per_page 5 --sort stars
```

**Request flow:**
```
Developer or Agent → bw binary → Parse args → IPC call_tool → Auth Daemon → Upstream MCP → response → stdout
```

**Binary characteristics:**

| Property | Target |
|----------|--------|
| Startup time | <5ms (no runtime, no connection pool — just parse args and send one IPC message) |
| Binary size | ~2-3MB (static Rust, no TLS needed — IPC is local) |
| Memory | ~1MB RSS (parse, send, print, exit) |
| Dependencies | Zero runtime. Single static binary. |
| Platforms | macOS (aarch64 + x86_64), Windows (x86_64), Linux (x86_64 + aarch64) |

**Core implementation:** `src-tauri/src/bin/bw.rs` — single file, ~300 lines. All complexity lives in the daemon.

```rust
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Parse into: list | <server> <tool> [--params] | help
    // Connect to daemon IPC socket
    // Send request, print response, exit
}

fn parse_params(args: &[&str]) -> HashMap<String, serde_json::Value> {
    // --key value → numeric coercion (int/bool first, fallback string)
    // --flag (no value) → boolean true
}
```

**Output formatting:**
- Default: raw text content from MCP tool response
- `--json`: full structured JSON response
- Debug/error info always to stderr (safe for piping)

**Source location:** `src-tauri/src/bin/bw.rs`

### Component 3: Auth Service (IPC Daemon) — `brightwing-authd`

Standalone background daemon built from Phase 1 (Decision D1). Serves credential requests, tool schema queries, and filter configurations to proxy and CLI shim instances.

**Architecture: Daemon-first (no in-process IPC phase)**
- The daemon (`brightwing-authd`) is the single source of truth for credentials and tool state
- The Tauri GUI app manages credentials and handles OAuth flows (browser-based), writing to the shared vault
- The daemon reads from the same vault and serves proxy/shim instances via IPC
- Key responsibilities:
  - Listens on a Unix socket / named pipe
  - Reads credentials via `VaultBackend` trait (Decision D2) — Stronghold in production, `HashMap` in tests
  - Handles token refresh autonomously
  - Caches tool schemas and serves them to `bw` shim and proxy
  - Manages tool filter state
  - Auto-starts on login (launchd on macOS, Task Scheduler on Windows, systemd user unit on Linux)
- The daemon is tiny (~3MB) — credential storage, token refresh, tool schema cache, filter management
- IPC protocol includes a `version` field for compatibility checking (Decision D7)

**Daemon IPC socket paths:**
- macOS: `~/Library/Application Support/com.brightwing.mcp-manager/authd.sock`
- Linux: `$XDG_RUNTIME_DIR/brightwing-authd.sock` or `/tmp/brightwing-authd.sock`
- Windows: `\\.\pipe\brightwing-authd`

**Daemon lifecycle:**
```
System Login
    │
    ▼
brightwing-authd starts (launchd/systemd/Task Scheduler)
    │
    ├── Opens Stronghold vault (encrypted at rest)
    ├── Binds IPC socket
    ├── Caches tool schemas from connected upstream servers
    ├── Loads tool filter config from SQLite
    ├── Starts token refresh scheduler
    │
    │   ┌── brightwing-proxy → GET_CREDENTIALS / GET_TOOL_FILTER
    │   ├── bw shim → LIST_SERVERS / CALL_TOOL / GET_TOOL_SCHEMA
    │   └── Both return cached/refreshed data
    │
    │   (Token approaching expiry)
    │   → Refresh OAuth token automatically
    │   → Update Stronghold
    │   → Next request gets fresh token
    │
    │   (Refresh fails)
    │   → Mark server as "needs_reauth" in SQLite
    │   → Send desktop notification
    │   → Next proxy request gets auth_expired error
    │
System Shutdown → brightwing-authd exits cleanly
```

**Tool schema cache (serves both `bw` and tool filtering):**
```rust
struct ConnectedServer {
    // ... connection/auth fields ...
    tool_schemas: Vec<ToolSchema>,      // Cached from tools/list response
    token_estimate: u32,                // Estimated tokens for full schema
    tools_fetched_at: Instant,          // When we last refreshed
}

struct ToolSchema {
    name: String,
    description: String,
    input_schema: serde_json::Value,    // Raw JSON Schema from MCP
    parameters: Vec<ParameterInfo>,     // Parsed for CLI --help display
}

struct ParameterInfo {
    name: String,
    param_type: String,                 // "string", "int", "bool", "array"
    description: String,
    required: bool,
    default: Option<String>,
}
```

**Token estimation:**
```rust
fn estimate_tokens(tools: &[serde_json::Value]) -> u32 {
    let serialized = serde_json::to_string(tools).unwrap_or_default();
    (serialized.len() as u32) / 4  // ~4 chars per token for JSON schema
}
```

**Source locations:**
- `src-tauri/src/bin/brightwing_authd.rs`
- `src-tauri/src/daemon/mod.rs`
- `src-tauri/src/daemon/lifecycle.rs`
- `src-tauri/src/daemon/scheduler.rs`

### Component 4: OAuth Flow Handler

Handles the OAuth 2.1 authorization code flow with PKCE, entirely within the Tauri GUI app.

**Flow:**
1. User clicks "Connect" on an OAuth-requiring MCP server
2. Brightwing generates PKCE code verifier + challenge
3. Opens system browser to authorization URL
4. Starts a temporary localhost HTTP server on a random port for callback
5. User authenticates in browser, grants consent
6. Browser redirects to `http://localhost:{port}/callback?code=...`
7. Brightwing exchanges code for tokens
8. Tokens stored in Stronghold vault
9. Localhost server shuts down
10. UI updates to show "Connected ✓"

**OAuth metadata discovery:**
1. Try `GET {server_url}/.well-known/oauth-authorization-server` first
2. Fall back to manual configuration if discovery fails
3. Cache discovered metadata in SQLite

**Dynamic Client Registration (RFC 7591):**
1. Check if server's metadata includes a `registration_endpoint`
2. If so, register Brightwing as a client automatically
3. Store obtained `client_id` and `client_secret` in vault

**Source locations:**
- `src-tauri/src/oauth/mod.rs`
- `src-tauri/src/oauth/discovery.rs`
- `src-tauri/src/oauth/flow.rs`
- `src-tauri/src/oauth/refresh.rs`
- `src-tauri/src/oauth/registration.rs`

### Component 5: Credential Store (Enhanced Vault)

**Abstracted behind `VaultBackend` trait (Decision D2):**
```rust
#[async_trait]
trait VaultBackend: Send + Sync {
    async fn store(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>>;
}

// Production: StrongholdBackend (wraps tauri-plugin-stronghold)
// Tests: InMemoryVaultBackend (wraps HashMap<String, Vec<u8>>)
```

| Type | Storage Key Pattern | Contents |
|------|-------------------|----------|
| **API Keys** | `secret:{server_id}:{env_var}` | Single environment variable value |
| **OAuth Tokens** | `oauth:{server_id}` | Access token, refresh token, expiry, metadata |
| **OAuth Client Creds** | `oauth_client:{server_id}` | Dynamic client registration credentials |
| **Server Connection** | `connection:{server_id}` | URL, transport type, command/args |

### Component 6: Proxy Config Writer (Modified Install Flow)

When a user enables a proxied server for a tool, Brightwing writes a proxy config instead of the upstream server's config:

**Before (direct install):**
```json
{
  "mcpServers": {
    "github-mcp": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "ghp_plaintext_secret_in_config" }
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

No env vars. No API keys. No OAuth URLs. Just a local binary and a server identifier.

---

## Data Model

### SQLite Schema

```sql
-- Servers managed through the Brightwing proxy
CREATE TABLE proxy_servers (
    server_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    upstream_transport TEXT NOT NULL,       -- "http", "sse", "stdio"
    upstream_url TEXT,
    upstream_command TEXT,
    upstream_args TEXT,                     -- JSON array
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
    config_key TEXT NOT NULL,              -- "bw-github-mcp"
    installed_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (server_id, tool_id),
    FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id)
);

-- Tool filtering: which tools are visible per server
CREATE TABLE proxy_tool_filter (
    server_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (server_id, tool_name),
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

### Tool Filter Population

When the daemon first connects to an upstream server and caches its `tools/list` response, it populates `proxy_tool_filter` with one row per tool, all defaulting to `enabled = 1`.

- **New servers work immediately** with all tools visible
- **Users opt in to filtering** by unchecking tools they don't need
- **Upstream changes reconciled:** new tools → insert enabled; removed tools → delete row; existing → preserve user choice

---

## IPC Protocol (Complete)

All IPC messages shared between proxy, `bw` shim, and daemon via `proxy-common` crate.

### Connection Handshake (Decision D7)

Every client (proxy or `bw` shim) sends a handshake on connect:
```jsonc
// Client → Daemon
{ "action": "handshake", "client": "brightwing-proxy", "version": "1.0.0" }

// Daemon → Client (compatible)
{ "type": "handshake_ok", "daemon_version": "1.0.0" }

// Daemon → Client (incompatible)
{ "type": "handshake_error", "daemon_version": "1.2.0", "min_client_version": "1.1.0",
  "message": "Brightwing proxy v1.0.0 is incompatible with daemon v1.2.0. Please update binaries from the Brightwing app." }
```

The dashboard shows daemon version, binary versions on disk, and flags mismatches.

### Messages

```jsonc
// === Credential Management ===

// Request: Get credentials for a server (proxy uses this)
{ "action": "get_credentials", "server_id": "github-mcp" }
// Response (OAuth)
{ "type": "oauth", "access_token": "gho_xxx...", "url": "https://..." }
// Response (API key)
{ "type": "api_key", "env": {"GITHUB_TOKEN": "ghp_xxx..."}, "command": "npx", "args": [...] }
// Response (error)
{ "type": "error", "code": "auth_expired", "message": "..." }

// === Tool Filtering ===

// Request: Get tool filter for a server (proxy uses this)
{ "action": "get_tool_filter", "server_id": "github" }
// Response
{
  "type": "tool_filter",
  "server_id": "github",
  "enabled_tools": ["search_repos", "get_repo", "list_issues"],
  "total_tools": 93,
  "token_estimate_filtered": 2200,
  "token_estimate_full": 55000
}

// Request: Get full tool list with filter status (Manager UI uses this)
{ "action": "get_tools_with_filter", "server_id": "github" }
// Response
{
  "type": "tools_with_filter",
  "server_id": "github",
  "tools": [
    { "name": "search_repos", "description": "Search GitHub repositories", "enabled": true },
    { "name": "create_pull_request", "description": "Create a new PR", "enabled": false }
  ],
  "enabled_count": 5,
  "total_count": 93,
  "token_estimate_filtered": 2200,
  "token_estimate_full": 55000
}

// Request: Update tool filter (single tool)
{ "action": "set_tool_filter", "server_id": "github", "tool_name": "create_pull_request", "enabled": true }
// Response
{ "type": "filter_updated", "server_id": "github", "enabled_count": 6 }

// Request: Bulk update tool filter
{
  "action": "set_tool_filter_bulk",
  "server_id": "github",
  "tools": { "search_repos": true, "get_repo": true, "list_issues": true },
  "disable_unlisted": true
}
// Response
{ "type": "filter_updated", "server_id": "github", "enabled_count": 3 }

// Daemon → Proxy push (filter invalidation)
{ "type": "filter_changed", "server_id": "github" }

// === CLI Shim Messages ===

// Request: List all servers with tool counts (bw list)
{ "action": "list_servers" }
// Response
{
  "type": "server_list",
  "servers": [{
    "server_id": "github",
    "display_name": "GitHub MCP Server",
    "tool_count": 5,
    "auth_type": "oauth",
    "auth_status": "connected",
    "token_estimate": 2200
  }]
}

// Request: List tools for a specific server (bw list <server>)
{ "action": "list_tools", "server_id": "github" }
// Response
{
  "type": "tool_list",
  "server_id": "github",
  "tools": [{
    "name": "search_repos",
    "description": "Search GitHub repositories",
    "parameters": [
      { "name": "query", "param_type": "string", "description": "Search query", "required": true, "default": null }
    ]
  }]
}

// Request: Get schema for a single tool (bw <server> <tool> --help)
{ "action": "get_tool_schema", "server_id": "github", "tool_name": "search_repos" }
// Response
{
  "type": "tool_schema",
  "name": "search_repos",
  "description": "Search GitHub repositories",
  "parameters": [/* same format */]
}

// Request: Call a tool (both proxy and bw shim use this)
{
  "action": "call_tool",
  "server_id": "github",
  "tool_name": "search_repos",
  "arguments": { "query": "mcp server", "per_page": 5 }
}
// Response
{
  "type": "tool_result",
  "content": [{ "type": "text", "text": "..." }],
  "is_error": false,
  "latency_ms": 342
}
```

---

## Tauri Commands

### Proxy Management
```rust
#[tauri::command]
fn register_proxy_server(
    server_id: String, display_name: String,
    upstream_transport: String, upstream_url: Option<String>,
    upstream_command: Option<String>, upstream_args: Option<Vec<String>>,
    auth_type: String, scoreboard_uuid: Option<String>,
    db: State<Database>,
) -> Result<(), String>

#[tauri::command]
fn list_proxy_servers(db: State<Database>) -> Result<Vec<ProxyServer>, String>

#[tauri::command]
fn install_proxy_to_tool(server_id: String, tool_id: String, db: State<Database>) -> Result<InstallResult, String>

#[tauri::command]
fn remove_proxy_from_tool(server_id: String, tool_id: String, db: State<Database>) -> Result<InstallResult, String>

#[tauri::command]
fn unregister_proxy_server(server_id: String, db: State<Database>, stronghold: State<Stronghold>) -> Result<(), String>

#[tauri::command]
fn get_proxy_auth_status(server_id: String, db: State<Database>) -> Result<AuthStatus, String>
```

### OAuth Flow
```rust
#[tauri::command]
async fn start_oauth_flow(server_id: String, server_url: String, db: State<Database>) -> Result<OAuthFlowInfo, String>

#[tauri::command]
async fn complete_oauth_flow(server_id: String, auth_code: String, state: String, db: State<Database>, stronghold: State<Stronghold>) -> Result<(), String>

#[tauri::command]
async fn refresh_oauth_token(server_id: String, db: State<Database>, stronghold: State<Stronghold>) -> Result<(), String>

#[tauri::command]
async fn discover_oauth_metadata(server_url: String) -> Result<OAuthMetadata, String>
```

### Daemon Management
```rust
#[tauri::command]
fn start_auth_daemon(db: State<Database>) -> Result<DaemonStatus, String>

#[tauri::command]
fn stop_auth_daemon(db: State<Database>) -> Result<(), String>

#[tauri::command]
fn get_daemon_status(db: State<Database>) -> Result<DaemonStatus, String>

#[tauri::command]
fn install_daemon_autostart() -> Result<(), String>

#[tauri::command]
fn uninstall_daemon_autostart() -> Result<(), String>
```

### Tool Filtering
```rust
#[tauri::command]
fn get_server_tools(server_id: String, db: State<Database>) -> Result<Vec<ToolWithFilter>, String>

#[tauri::command]
fn set_tool_enabled(server_id: String, tool_name: String, enabled: bool, db: State<Database>) -> Result<FilterSummary, String>

#[tauri::command]
fn set_tool_filter_bulk(server_id: String, enabled_tools: Vec<String>, disable_unlisted: bool, db: State<Database>) -> Result<FilterSummary, String>

#[tauri::command]
fn reset_tool_filter(server_id: String, db: State<Database>) -> Result<FilterSummary, String>
```

---

## TypeScript Types

```typescript
// src/lib/types.ts

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
  tool_installs: string[];
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
  expires_at: string | null;
  error_message: string | null;
  last_refreshed_at: string | null;
}

export interface ToolWithFilter {
  name: string;
  description: string;
  enabled: boolean;
  parameter_count: number;
  token_estimate: number;
}

export interface FilterSummary {
  server_id: string;
  enabled_count: number;
  total_count: number;
  token_estimate_filtered: number;
  token_estimate_full: number;
}
```

---

## User Experience Flows

### Flow 1: Adding an OAuth MCP Server

1. User searches for "GitHub" in Brightwing
2. Clicks "Add to Brightwing" → OAuth flow dialog
3. "Connect with GitHub" → browser opens → login → consent → callback
4. Tokens stored in encrypted vault → UI shows "Connected ✓"
5. Choose which tools get the proxy (Claude Desktop, Cursor, Claude Code, etc.)
6. Proxy config written to each selected tool's config file
7. Restart banner shown for GUI tools

### Flow 2: Adding an API Key MCP Server

1. User installs from Scoreboard (as today)
2. Enters API key in secure input field
3. Option: "Use Brightwing Proxy (recommended)" vs "Direct install (legacy)"
4. If proxy: key stored in vault, proxy config written (no plaintext keys in configs)

### Flow 3: Dashboard with Proxy + Filter Indicators

```
┌─────────────────────────────────────────────────────────────┐
│  Dashboard                                                   │
│                                                              │
│  MCP Context Budget: ~6,200 tokens  (unfiltered: ~98,000)   │
│  ░░░▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓   │
│                                                              │
│  Server               │ Tools    │ Tokens  │ Auth  │ CD │..│
│  ─────────────────────┼──────────┼─────────┼───────┼────┼──│
│  🔒 bw-github          │ 5 of 93  │ ~2,200  │ ✓ OAuth│ ☑ │  │
│  🔒 bw-jira            │ 8 of 40  │ ~2,800  │ ✓ OAuth│ ☑ │  │
│  🔑 bw-brave-search    │ 3 of 3   │ ~900    │ ✓ Key │ ☑ │  │
│     filesystem          │ 4 of 4   │ ~300    │ None  │ ☑ │  │
│                                                              │
│  Click any server to manage its tool filter.                │
└─────────────────────────────────────────────────────────────┘
```

### Flow 4: Tool Filter Panel

Click a server → slide-out panel showing all tools with checkboxes:
- Per-tool token estimate
- Visual token cost bar (filtered vs. full)
- Enable All / Disable All / Apply Profile buttons
- Search filter for servers with dozens of tools
- Changes apply immediately — no restart required
- Daemon pushes invalidation to active proxy instances

---

## Agent Integration (CLI Shim)

### Claude Code

When the user enables the `bw` shim for Claude Code, the Manager appends to `CLAUDE.md`:

```markdown
## Brightwing Tool Broker

This project has access to external tools via the `bw` command.
Run `bw list` to see available servers and tools.
Run `bw <server> <tool> --help` for usage details.

Prefer `bw` commands over MCP servers for context efficiency.

Quick reference:
- `bw github search_repos --query "topic:mcp-server" --per_page 10`
- `bw jira list_issues --project ENG --status open`
- `bw linear list_teams --json`
```

### Codex / Gemini CLI

Same pattern. Codex reads from `~/.codex/instructions.md`, Gemini CLI reads from `GEMINI.md`. The Manager knows the right file for each tool and injects the documentation.

---

## Curated Tool Profiles

Community-maintained profiles that override raw MCP schemas with tighter descriptions and a curated tool subset. One profile serves both interfaces:
- Populates the MCP proxy tool filter (for Claude Desktop)
- Powers `bw --help` output (for Claude Code)

```jsonc
// ~/.brightwing/profiles/github.json
{
  "server_id": "github",
  "profile_version": "1.0.0",
  "source": "scoreboard",
  "curated_tools": [
    {
      "name": "search_repos",
      "cli_description": "Search GitHub repositories",
      "parameters": {
        "query":    { "description": "Search query string", "required": true },
        "per_page": { "description": "Results per page", "default": "10" }
      }
    }
  ],
  "hidden_tools": ["create_or_update_file", "push_files", "fork_repository"],
  "safety": {
    "read_only_default": true,
    "destructive_tools": ["delete_repository", "delete_branch"],
    "require_confirmation": ["create_issue", "create_pull_request"]
  },
  "token_estimate_raw": 55000,
  "token_estimate_curated": 2200
}
```

### Relationship Between Tool Filtering and Curated Profiles

| Feature | Tool Filtering | Curated Profiles |
|---------|---------------|-----------------|
| **Purpose** | User controls which tools MCP clients see | Tighter descriptions + preset tool selection for `bw` CLI |
| **Scope** | Per-user, per-server | Community-maintained, distributed via Scoreboard |
| **Storage** | `proxy_tool_filter` SQLite table | `~/.brightwing/profiles/*.json` |
| **Applies to** | stdio MCP proxy (Claude Desktop, Cursor, etc.) | `bw` CLI shim AND can be applied to MCP proxy filter |

**Profiles can seed filters.** When a user clicks "Apply Profile" in the tool filter panel, it reads the curated profile's `curated_tools` list and bulk-updates the filter table via `set_tool_filter_bulk` with `disable_unlisted: true`.

**Token estimates are shared.** Both systems use the same token estimation logic from the daemon's tool schema cache.

**One profile, two interfaces.** A curated GitHub profile that selects 5 tools is useful in both directions: it populates the MCP proxy filter (for Claude Desktop) and the `bw --help` output (for Claude Code). Users apply it once, both interfaces benefit.

**Distribution:** Profiles fetched from Scoreboard API (`GET patchworkmcp.com/api/cli-profiles/{server_id}/`). Community-contributed and Brightwing-reviewed.

**Auto-generation fallback:** For servers without curated profiles, an LLM pass can compress raw schemas into CLI-friendly descriptions. Result cached as auto-generated profile.

---

## Security Model

| Threat | Mitigation |
|--------|-----------|
| **Config file exposure** | Proxy configs contain no secrets. Only `brightwing-proxy --server <id>`. |
| **Vault compromise** | Stronghold encryption with Argon2-derived key. OS keychain integration. |
| **IPC eavesdropping** | Socket file permissions `0600` (owner-only). PID verification on connection. |
| **Proxy binary tampering** | Binary checksum verified by daemon on startup. Code signing on distributed builds. |
| **Token theft from memory** | Tokens held in memory only during active request. Process isolation per proxy instance. |
| **Expired token abuse** | Refresh tokens stored encrypted. Short TTL access tokens. Automatic refresh with immediate vault update. |
| **Rogue MCP server** | Scoreboard quality scores as trust signal before proxying. |

---

## Proxy & CLI Binary Distribution

### Standalone Builds (Decision D3)

All binaries are built independently via `cargo build --release --bin <name>`. No Tauri sidecar mechanism. The Tauri app copies built binaries to install locations on first run / update.

### Installation Locations

| Platform | Binary Paths | PATH Addition |
|----------|-------------|---------------|
| macOS | `~/.local/bin/brightwing-proxy`, `~/.local/bin/bw`, `~/.local/bin/brightwing-authd` | `~/.local/bin` (add to shell profile if needed) |
| Windows | `%LOCALAPPDATA%\Brightwing\bin\brightwing-proxy.exe`, `%LOCALAPPDATA%\Brightwing\bin\bw.exe`, `%LOCALAPPDATA%\Brightwing\bin\brightwing-authd.exe` | Add to User PATH env var |
| Linux | `~/.local/bin/brightwing-proxy`, `~/.local/bin/bw`, `~/.local/bin/brightwing-authd` | `~/.local/bin` (typically already on PATH) |

### Config File Strategy (Decision D4)

**Primary:** Absolute paths in config files. This is the default and most reliable approach:
```json
{ "command": "/Users/keyton/.local/bin/brightwing-proxy", "args": ["--server", "github-mcp"] }
```

**Fallback:** Short binary name (relies on PATH):
```json
{ "command": "brightwing-proxy", "args": ["--server", "github-mcp"] }
```

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

[[bin]]
name = "bw"
path = "src/bin/bw.rs"

[dependencies]
tauri-plugin-stronghold = "2"
tokio = { version = "1", features = ["full"] }
proxy-common = { path = "crates/proxy-common" }
```

```
src-tauri/
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config/                    # Config reader/writer (modified)
│   ├── db/                        # SQLite (extended with proxy + filter tables)
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
│       ├── brightwing_authd.rs    # Auth daemon binary (new)
│       └── bw.rs                  # CLI shim binary (new)
│
├── crates/
│   └── proxy-common/              # Shared types between all binaries
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── ipc.rs             # IPC protocol types (all messages)
│           └── credentials.rs     # Credential types
│
└── resources/
    ├── com.brightwing.authd.plist          # macOS launchd config
    ├── brightwing-authd.service            # Linux systemd user unit
    └── brightwing-authd-task.xml           # Windows Task Scheduler
```

---

## Technology Choices

### Why Rust for Everything

| Factor | Rust | Python | Node.js |
|--------|------|--------|---------|
| **Startup time** | <50ms (proxy), <5ms (bw) | 200-500ms | 100-300ms |
| **Memory per proxy** | ~5MB | ~30MB | ~25MB |
| **Runtime dependency** | None (static binary) | Python interpreter | Node.js runtime |
| **Already in stack** | Yes (Tauri backend) | No | Yes (frontend tooling) |
| **Stronghold access** | Native (same crate) | FFI required | FFI required |

Sub-5ms startup for `bw` is critical. Agents may call it hundreds of times per session. 5+ proxy instances running simultaneously need minimal footprint.

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

## Scoreboard Integration

### Context Efficiency Scoring Dimension

Add to the existing Scoreboard scoring:

| Token count | Score |
|------------|-------|
| <1,000 | 100 |
| 1,000-5,000 | 85 |
| 5,000-15,000 | 70 |
| 15,000-30,000 | 50 |
| 30,000-55,000 | 30 |
| >55,000 | 10 |

### "Available via `bw`" Badge

Servers with curated profiles get a badge on their Scoreboard page.

### Context Budget Calculator

Interactive tool on the Scoreboard website showing total MCP context cost vs Brightwing `bw` CLI cost.

---

## Phased Implementation Plan

### Phase 1: Foundation — Mock Server, Daemon, Proxy & IPC (Weeks 1-4) ✅ COMPLETE

**Goal:** A working `brightwing-proxy` binary that proxies an HTTP MCP server, getting credentials from a running `brightwing-authd` daemon. Mock MCP server built first to unblock all testing.

**Deliverables (in order):**
1. ✅ **Mock MCP server** (Decision D5 — build first)
   - Reusable test fixture in `tests/mock_mcp_server.rs`
   - HTTP transport mode: `wiremock`-based, handles `initialize`, `tools/list`, `tools/call`
   - stdio transport mode: spawnable child process for stdio proxy testing
   - Configurable tool set and canned responses
2. ✅ **`proxy-common` crate** — shared types for all binaries
   - IPC protocol types with `version` field (Decision D7)
   - Credential types
   - `VaultBackend` trait + `InMemoryVaultBackend` (Decision D2)
   - Serde round-trip tests for all types
3. ✅ **`brightwing-authd` daemon** (Decision D1 — daemon from day 1)
   - Listens on Unix socket (macOS/Linux) / named pipe (Windows)
   - `VaultBackend` trait for credential access (Stronghold impl deferred to Phase 2, in-memory for now)
   - Serves `get_credentials` requests
   - Handshake with version checking
   - Handles concurrent connections
4. ✅ **`brightwing-proxy` binary**
   - JSON-RPC message parser for stdin
   - IPC client connecting to daemon
   - HTTP client for forwarding to upstream with auth headers
   - Response relay back to stdout
   - MCP protocol compliance (initialize, ping, tools/list, tools/call)
5. ✅ **`bw` CLI arg parsing and output formatting** (Decision D10 — pull forward)
   - `parse_params()` for `--key value` CLI args with type coercion
   - Output formatting: text default, `--json` structured
   - `--help` generation from tool schemas
   - All daemon-independent, fully unit-testable

**Files created:**
- `tests/mock_mcp_server.rs`
- `src-tauri/crates/proxy-common/Cargo.toml`
- `src-tauri/crates/proxy-common/src/lib.rs`
- `src-tauri/crates/proxy-common/src/ipc.rs`
- `src-tauri/crates/proxy-common/src/credentials.rs`
- `src-tauri/crates/proxy-common/src/vault.rs` (trait + in-memory impl)
- `src-tauri/src/bin/brightwing_authd.rs`
- `src-tauri/src/bin/brightwing_proxy.rs`
- `src-tauri/src/bin/bw.rs` (arg parsing + formatting only, IPC wiring in Phase 5)
- `src-tauri/src/daemon/mod.rs`
- `src-tauri/src/daemon/ipc_server.rs`
- `src-tauri/src/proxy/mod.rs`
- `src-tauri/src/proxy/ipc_client.rs`
- `src-tauri/src/proxy/transport.rs`

**Testing — Phase 1:** ✅ All automated tests passing.

| Test Type | Status | What | Location |
|-----------|--------|------|----------|
| **Unit: IPC serialization** | ✅ | 35+ round-trip tests for all request/response types | `proxy-common/src/ipc.rs` |
| **Unit: VaultBackend** | ✅ | 10 tests: store/retrieve/delete/list_keys/overwrite/binary data | `proxy-common/src/vault.rs` |
| **Unit: Credentials** | ✅ | 5 tests: OAuth/ApiKey/None round-trips, error codes | `proxy-common/src/credentials.rs` |
| **Unit: `bw` arg parsing** | ✅ | 11 tests: parse_params, help, type coercion, boolean flags, positional args | `src/bin/bw.rs` |
| **Unit: `bw` output formatting** | ✅ | 7 tests: text blocks, error results, server list, tool help | `src/bin/bw.rs` |
| **Integration: daemon ↔ proxy** | ✅ | 5 tests: OAuth/ApiKey auth, tools/list, tools/call, initialize forwarding | `tests/proxy_integration.rs` |
| **Integration: daemon IPC** | ✅ | 7 tests: handshake, credentials, concurrent clients, malformed requests | `tests/daemon_ipc_test.rs` |
| **Integration: mock MCP server** | ✅ | 9 tests: initialize, tools/list, tools/call, auth, recording, keep-alive | `tests/mock_mcp_server.rs` |

### Phase 2: Credential Management & Tool Filtering (Weeks 4-7) ✅ COMPLETE

**Goal:** Stronghold `VaultBackend` impl, API key storage, tool filtering in the proxy, dashboard enhancements, binary distribution.

**Deliverables:**
1. ✅ **Stronghold `VaultBackend` implementation** (Decision D2)
   - `StrongholdBackend` in `crates/proxy-common/src/stronghold_vault.rs` behind `stronghold` feature flag
   - Uses SHA-256 hashed passphrase → 32-byte key for `KeyProvider`
   - 8 feature-gated tests (persistence, wrong passphrase rejection, etc.)
   - Daemon uses `StrongholdBackend` in production, `InMemoryVaultBackend` in tests
2. ✅ **Binary distribution** (Decisions D3, D4)
   - Tauri commands: `distribute_binaries`, `get_binary_versions`
   - Copies `brightwing-proxy`, `brightwing-authd`, `bw` to `~/.local/bin/`
   - Config files use absolute paths as primary strategy
3. ✅ **API Key flow for proxy servers**
   - `proxy_api_keys` SQLite table (env vars stored as JSON, CASCADE delete)
   - DB CRUD: `store_api_key`, `get_api_key`, `delete_api_key`, `get_all_api_keys`
   - Tauri commands bridging GUI ↔ SQLite
   - Daemon serves keys to proxy instances via IPC (`StoreCredentials`/`GetCredentials`)
4. ✅ **Proxy config writer**
   - `install_proxy_server()` in `config/writer.rs` — writes `brightwing-proxy --server <id>` as stdio
   - `proxy_binary_path()` with fallback chain: `~/.local/bin` → PATH → default
   - Handles all config formats (JSON, TOML, CLI)
5. ✅ **Tool schema caching in daemon**
   - `proxy_tool_cache` SQLite table + `cache_tool_schema` query (auto-creates filter entries)
   - Token estimation utility in `crates/proxy-common/src/tokens.rs` (~4 chars/token heuristic)
   - Tauri commands: `cache_tool_schema`, `get_cached_tools`
6. ✅ **Tool filter interception in proxy**
   - `apply_tool_filter()` in `brightwing_proxy.rs` with 3 unit tests
   - Proxy fetches filter from daemon on each `tools/list`, strips disabled tools
   - Empty filter = pass-through (all tools returned)
7. ✅ **Tool filter IPC handlers**
   - IPC messages: `GetToolFilter`, `SetToolFilter`, `SetToolFilterBulk` (+ responses)
   - `proxy_tool_filter` SQLite table with per-server, per-tool enabled/disabled + token estimate
   - Tauri commands: `get_tool_filter`, `set_tool_filter`, `set_tool_filter_bulk`
8. ✅ **Tool Filter Panel UI** (`src/components/ToolFilterPanel.tsx`)
   - Per-tool checkboxes with token estimates
   - Token cost progress bar (enabled/total tokens)
   - Enable All / Disable All buttons
   - Search/filter tools by name
   - Optimistic UI updates in Zustand store
9. ✅ **Dashboard enhancements** (`src/components/Dashboard.tsx`)
   - Proxy servers summary section with auth type badges (OAuth/API Key/None)
   - Token budget bars per proxy server (enabled tools + token counts)
   - "Manage →" link to Proxy view
10. ✅ **API Keys panel UI** (`src/components/ApiKeysPanel.tsx`)
    - Lists all proxy servers with `api_key` auth type
    - "Configured" / "Not Set" status badges
    - Dynamic env var entry form (auto-adds rows, X to remove)
    - Password-masked values with Reveal/Hide toggle
    - Edit, Save, Remove, Cancel actions
    - Updated timestamp display
11. ✅ **Proxy Servers management UI** (`src/components/ProxyServers.tsx`)
    - List / Add / Filter sub-views
    - Registration form: server ID, display name, auth type, upstream URL
    - Expandable server cards with per-tool install/uninstall buttons
    - Tool Filter and Remove actions
12. ✅ **Navigation & routing**
    - "Proxy" nav item (shield icon) and "API Keys" nav item (key icon)
    - `proxy` and `api-keys` views wired into App.tsx

**Files created:**
- `crates/proxy-common/src/stronghold_vault.rs` — StrongholdBackend (feature-gated)
- `crates/proxy-common/src/tokens.rs` — token estimation
- `src/components/ToolFilterPanel.tsx`
- `src/components/ProxyServers.tsx`
- `src/components/ApiKeysPanel.tsx`

**Files modified:**
- `crates/proxy-common/Cargo.toml` — stronghold feature + deps
- `crates/proxy-common/src/lib.rs` — new module exports
- `crates/proxy-common/src/ipc.rs` — 6 new request variants, 1 new response variant, 8+ new tests
- `src-tauri/src/db/migrations.rs` — 5 new SQLite tables
- `src-tauri/src/db/queries.rs` — `ProxyServer`, `ProxyApiKey`, `ToolFilterEntry`, `CachedTool` types + ~20 query methods
- `src-tauri/src/config/writer.rs` — proxy install mode
- `src-tauri/src/lib.rs` — 18 new Tauri commands
- `src-tauri/src/bin/brightwing_authd.rs` — handlers for all new IPC messages
- `src-tauri/src/bin/brightwing_proxy.rs` — `apply_tool_filter()` + 3 unit tests
- `src/lib/types.ts` — `ProxyServer`, `ToolFilterEntry`, `CachedTool`, `ProxyApiKey` types, `View` union
- `src/lib/tauri.ts` — 18 new Tauri bindings
- `src/store.ts` — proxy server + tool filter state
- `src/components/Dashboard.tsx` — proxy summary section
- `src/components/Navigation.tsx` — Proxy + API Keys nav items
- `src/App.tsx` — new view routing + initial data load

**Testing — Phase 2:**

| Test Type | Status | What | Location |
|-----------|--------|------|----------|
| **Unit: StrongholdBackend** | ✅ | 8 tests: store/retrieve, persistence, wrong passphrase, overwrite, delete, list keys | `crates/proxy-common/src/stronghold_vault.rs` (feature-gated) |
| **Unit: tool filter logic** | ✅ | `apply_tool_filter()`: retains enabled, empty filter removes all, no-result passthrough | `src/bin/brightwing_proxy.rs` (3 tests) |
| **Unit: token estimation** | ✅ | `estimate_tool_tokens()` with sample schemas | `crates/proxy-common/src/tokens.rs` |
| **Unit: IPC messages** | ✅ | Round-trip tests for all new request/response variants | `crates/proxy-common/src/ipc.rs` (8+ new tests) |
| **Integration: filter round-trip** | ✅ | Seed filter → proxy fetches → `tools/list` omits disabled tools | `tests/proxy_integration.rs` |
| **Integration: filter update** | ✅ | Change filter via IPC → next proxy request reflects update | `test_proxy_filter_update_reflected_in_next_request` |
| **Integration: filter edge cases** | ✅ | Single tool of many, no filter = pass-through | `test_proxy_filter_single_tool_out_of_many`, `test_proxy_no_filter_returns_all_tools` |
| **Integration: credential store → use** | ✅ | GUI stores via IPC → proxy retrieves → forwards with auth → mock validates | `test_credential_store_then_proxy_uses` |
| **Integration: credential overwrite** | ✅ | Overwrite credential → proxy uses latest (mock rejects old token) | `test_credential_overwrite_then_proxy_uses_latest` |
| **Integration: credential CRUD** | ✅ | Store, overwrite, delete, verify gone | `daemon_ipc_test.rs` (3 tests) |
| **Integration: filter toggle** | ✅ | Bulk set → disable → re-enable → verify | `daemon_ipc_test.rs` (3 tests) |

**Test counts:** 69 fast tests passing (no Stronghold feature) + 8 Stronghold tests passing (feature-gated).

**Cumulative test counts through Phase 4:** 85 fast tests passing (50 proxy-common + 19 proxy integration + 14 oauth integration + 2 new IPC Ping/Pong) + 8 Stronghold tests (feature-gated).

### Phase 3: OAuth Integration (Weeks 7-10) ✅ COMPLETE

**Goal:** Full OAuth 2.1 PKCE flow for remote MCP servers.

**Deliverables:**
1. ✅ OAuth metadata discovery
   - `GET /.well-known/oauth-authorization-server` probing
   - Metadata caching in `oauth_server_meta` SQLite table
   - Fallback: uses cached metadata on subsequent flows
2. ✅ Authorization flow
   - PKCE code verifier/challenge generation (S256)
   - Temporary localhost HTTP server on `127.0.0.1:0` for callback
   - Browser launch for authorization via `@tauri-apps/plugin-shell`
   - Code-to-token exchange with PKCE verification
   - Token storage in `oauth_token_sets` SQLite table
3. ✅ Dynamic Client Registration (RFC 7591)
   - Auto-register Brightwing with servers that support it
   - Client secret forwarded in token exchange when provided
4. ✅ Token refresh lifecycle
   - `refresh_token` grant type exchange
   - Preserves old refresh token if server doesn't re-issue
   - Proactive refresh via GUI "Refresh Token" button
   - Reactive refresh on 401: proxy detects 401, re-fetches credentials from daemon
   - Desktop notification on token expiry via `tauri-plugin-notification`
5. ✅ Re-authentication flow
   - Detect expired/revoked tokens (`get_status` checks `expires_at`)
   - Desktop notification prompting user to re-auth
   - "Re-authenticate" button in GUI for expired tokens
   - "Disconnect" action clears tokens + metadata
6. ✅ OAuth UI (`OAuthConnect.tsx`)
   - "Connect with OAuth" button with spinner during browser wait
   - Status indicator (connected/disconnected/expired/error with colored dot)
   - Expiry timestamp display
   - "Refresh Token" / "Re-authenticate" / "Disconnect" actions
   - Auto-polling (2s) for callback completion during flow

**Files created:**
- `src-tauri/src/oauth/mod.rs` — Module declarations
- `src-tauri/src/oauth/types.rs` — OAuthServerMetadata, OAuthTokenSet, ClientRegistration, OAuthFlowState, OAuthFlowInfo, OAuthStatus, OAuthError
- `src-tauri/src/oauth/pkce.rs` — PKCE verifier/challenge/state generation
- `src-tauri/src/oauth/discovery.rs` — `.well-known/oauth-authorization-server` discovery
- `src-tauri/src/oauth/registration.rs` — RFC 7591 dynamic client registration
- `src-tauri/src/oauth/callback.rs` — Localhost callback server (TcpListener, 5min timeout)
- `src-tauri/src/oauth/exchange.rs` — Authorization code → token exchange
- `src-tauri/src/oauth/refresh.rs` — Token refresh (`refresh_token` grant)
- `src-tauri/src/oauth/flow.rs` — Flow orchestration (start_flow, complete_flow, get_status, disconnect)
- `src/components/OAuthConnect.tsx` — OAuth UI component
- `src-tauri/tests/oauth_integration.rs` — 13 integration tests with mock OAuth server

**Files modified:**
- `src-tauri/src/lib.rs` — Added `mod oauth`, 6 Tauri commands, `OAuthFlowStates` managed state, made `db` and `oauth` pub
- `src-tauri/src/db/mod.rs` — Added `new_in_memory()` for tests
- `src-tauri/src/db/migrations.rs` — Added `oauth_server_meta` and `oauth_token_sets` tables
- `src-tauri/src/db/queries.rs` — Added OAuth metadata + token CRUD methods
- `src-tauri/src/bin/brightwing_proxy.rs` — Added 401 detection with re-auth retry loop
- `src-tauri/src/bin/brightwing_authd.rs` — Added `RefreshOAuthToken` IPC handler + background refresh scheduler
- `src-tauri/Cargo.toml` — Added `rand`, `sha2`, `base64` dependencies
- `src/lib/types.ts` — Added `OAuthFlowInfo`, `OAuthStatus` interfaces
- `src/lib/tauri.ts` — Added 5 OAuth bindings
- `src/components/ProxyServers.tsx` — Integrated `OAuthConnect` for OAuth-type servers

**Testing — Phase 3:**

| Test | Location | Count |
|------|----------|-------|
| Unit: PKCE generation (length, format, SHA-256, uniqueness) | `oauth/pkce.rs` | 6 |
| Unit: metadata discovery (invalid URL, full/minimal deserialize) | `oauth/discovery.rs` | 3 |
| Unit: token exchange (full, minimal, error, missing token) | `oauth/exchange.rs` | 4 |
| Unit: token refresh (no refresh token) | `oauth/refresh.rs` | 1 |
| Unit: dynamic client registration (with/without secret) | `oauth/registration.rs` | 2 |
| Unit: callback server (normal, URL-encoded, error, missing code, live) | `oauth/callback.rs` | 5 |
| Integration: full flow with dynamic registration | `tests/oauth_integration.rs` | 1 |
| Integration: flow with provided client_id | `tests/oauth_integration.rs` | 1 |
| Integration: complete with direct code (deep link) | `tests/oauth_integration.rs` | 1 |
| Integration: status disconnected/expired | `tests/oauth_integration.rs` | 2 |
| Integration: disconnect clears tokens | `tests/oauth_integration.rs` | 1 |
| Integration: token refresh cycle | `tests/oauth_integration.rs` | 1 |
| Integration: refresh preserves old refresh token | `tests/oauth_integration.rs` | 1 |
| Integration: no registration + no client_id → error | `tests/oauth_integration.rs` | 1 |
| Integration: complete without callback → error | `tests/oauth_integration.rs` | 1 |
| Integration: invalid state → error | `tests/oauth_integration.rs` | 1 |
| Integration: metadata caching | `tests/oauth_integration.rs` | 1 |
| Integration: client secret forwarded | `tests/oauth_integration.rs` | 1 |
| Integration: proxy 401 retry with refreshed token | `tests/oauth_integration.rs` | 1 |
| **Total Phase 3 tests** | | **36** |

### Phase 4: Daemon Hardening & Lifecycle (Weeks 10-12) ✅ COMPLETE

**Goal:** Production-harden the daemon (built in Phase 1). Auto-start on login, crash recovery, GUI coordination. The daemon already exists and handles IPC + credentials — this phase makes it robust for always-on operation.

**Deliverables:**
1. ✅ IPC Ping/Pong health check
   - Added `Ping` to `IpcRequest` and `Pong { uptime_secs, daemon_version }` to `IpcResponse`
   - Daemon tracks `started_at: Instant` and responds with uptime
2. ✅ Daemon lifecycle management
   - PID file management (`authd.pid`) — write on start, check for stale, remove on shutdown
   - Duplicate instance detection via PID file + `kill -0`
   - Graceful shutdown on SIGTERM and ctrl-c (removes socket + PID file)
   - Auto-start on login:
     - macOS: launchd plist at `~/Library/LaunchAgents/com.brightwing.authd.plist`
     - Linux: systemd user unit at `~/.config/systemd/user/brightwing-authd.service`
3. ✅ GUI ↔ Daemon coordination
   - `daemon_status` command — reads PID file, pings daemon for uptime/version
   - `start_daemon` command — spawns daemon as detached process
   - `stop_daemon` command — sends SIGTERM to daemon PID
   - `is_autostart_enabled` / `set_autostart` commands — manage launchd/systemd configs
4. ✅ Daemon status UI (`DaemonStatus.tsx`)
   - Status indicator (running/stopped) with green/gray dot
   - PID, uptime, and version display
   - Start/Stop/Refresh buttons
   - Auto-start toggle switch ("Start on login")
   - Auto-refresh every 10 seconds
   - Integrated into ProxyServers view
5. Token refresh scheduler deferred to Phase 5 (requires daemon direct DB access)

**Files created/modified:**
- `src-tauri/crates/proxy-common/src/ipc.rs` (modified — Ping/Pong messages + tests)
- `src-tauri/src/bin/brightwing_authd.rs` (modified — Ping handler, started_at, PID file, graceful shutdown)
- `src-tauri/src/lib.rs` (modified — daemon lifecycle Tauri commands)
- `src/components/DaemonStatus.tsx` (new — daemon status UI)
- `src/components/ProxyServers.tsx` (modified — integrated DaemonStatus)
- `src/lib/types.ts` (modified — DaemonStatusInfo type)
- `src/lib/tauri.ts` (modified — daemon lifecycle bindings)
- `src/store.ts` (modified — daemon state management)
- `src-tauri/Cargo.toml` (modified — added libc dependency)

**Testing — Phase 4:**

| Test Type | What to Test | How |
|-----------|-------------|-----|
| **Unit: IPC Ping/Pong** | Ping request and Pong response round-trip serialization | `proxy-common/src/ipc.rs` tests (2 new tests) |
| **Unit: all variants** | All IPC request variants parse from JSON including Ping | `proxy-common/src/ipc.rs` test updated |
| **Integration: daemon lifecycle** | Start daemon → verify IPC socket created → ping → get credentials → stop daemon → verify socket removed | `tests/daemon_integration.rs` (future) |
| **Manual: GUI close** | Close Brightwing GUI → verify daemon still running → verify proxy still works → open GUI → verify daemon status shows running | Manual checklist |

### Phase 5: CLI Shim Wiring & stdio Upstream Proxy (Weeks 12-14)

**Goal:** Wire the `bw` CLI binary (arg parsing from Phase 1) to the daemon via IPC. Proxy handles stdio upstream servers with credential injection. Agent documentation injection.

**Deliverables:**
1. Wire `bw` binary to daemon via IPC
   - IPC client (reuse `proxy-common`)
   - Connect arg parsing (Phase 1) to `list_servers`, `list_tools`, `get_tool_schema`, `call_tool` IPC messages
2. Add CLI shim IPC handlers to daemon
   - `list_servers`, `list_tools`, `get_tool_schema`
   - `call_tool` (direct tool invocation through daemon)
3. Process supervisor in proxy binary (stdio upstream mode)
   - Spawn upstream command with injected env vars from vault
   - Pipe stdin/stdout bidirectionally
   - Monitor for crashes, clean up child process
   - Signal forwarding (SIGTERM, SIGINT)
4. Agent documentation injection
   - Detect `CLAUDE.md`, `GEMINI.md`, `~/.codex/instructions.md`
   - Generate and inject `bw` documentation block
   - UI toggle per agent: "Inject bw docs into [Claude Code ✓] [Codex ☐]"
   - Re-generate docs when servers are added/removed

**Files modified:**
- `src-tauri/src/bin/bw.rs` (modified — add IPC wiring to existing arg parsing)
- `src-tauri/src/bin/brightwing_proxy.rs` (modified — stdio upstream mode)
- `src-tauri/src/proxy/transport.rs` (modified — stdio ↔ stdio mode)
- `src-tauri/src/daemon/mod.rs` (modified — CLI shim IPC handlers)

**Testing — Phase 5:**

| Test Type | What to Test | How |
|-----------|-------------|-----|
| **Unit: stdio process supervisor** | Spawns child, pipes stdin/stdout, handles child exit, forwards signals | `#[cfg(test)]` with a mock child process (simple echo binary) |
| **Integration: bw list** | `bw list` returns all connected servers. `bw list github` returns tools for a specific server. | `tests/bw_integration.rs` with running daemon + mock upstream |
| **Integration: bw call** | `bw github search_repos --query "test"` returns results from mock upstream | Integration test |
| **Integration: bw --help** | `bw github search_repos --help` displays parameter info from cached schema | Integration test |
| **Integration: stdio proxy** | Proxy spawns mock stdio server with env vars injected. Verify env vars present in child. Verify MCP messages pass through. | `tests/stdio_proxy.rs` |
| **Integration: agent doc injection** | Toggle "inject bw docs" in UI → verify CLAUDE.md updated with correct content → add/remove server → verify docs regenerated | Integration test or manual |
| **E2E: bw with real server** | `bw brave-search web_search --query "test"` returns real results via authenticated proxy | E2E test (requires API key) |
| **Manual: Claude Code + bw** | Add `bw` docs to CLAUDE.md → start Claude Code → ask it to "search GitHub for mcp servers" → verify it uses `bw` command | Manual checklist |
| **Performance: latency** | Measure `bw list` latency (<50ms), `bw <tool>` overhead (<200ms excluding upstream) | `time` command benchmarks |
| **Performance: rapid invocation** | Call `bw list` 100 times in a loop. Verify no resource leaks, consistent latency. | Shell script benchmark |

### Phase 6: Migration & Polish (Weeks 14-16)

**Goal:** Migration from direct installs, proxy health monitoring, error recovery, documentation.

**Deliverables:**
1. Migration assistant
   - Scan existing tool configs for servers with env vars
   - Offer to import into vault and replace with proxy configs
   - Rollback capability on verification failure
2. Token budget display (enhanced)
   - `bw list` shows total MCP cost vs bw cost
   - Manager UI shows per-server and total budget
3. Proxy health monitoring
   - Periodic connectivity checks to upstream servers
   - Status page showing proxy health per server
   - Latency metrics
4. Error recovery
   - Vault corruption detection and recovery from backup
5. Documentation
   - User guide: adding servers, OAuth flow, troubleshooting
   - Architecture docs for contributors
   - FAQ: "Why does Brightwing need a background service?"

**Files created/modified:**
- `src/components/MigrationAssistant.tsx` (new)
- `src-tauri/src/migration.rs` (new)

**Testing — Phase 6:**

| Test Type | What to Test | How |
|-----------|-------------|-----|
| **Unit: migration detection** | Correctly identifies configs with env vars that could be migrated. Ignores configs without secrets. | `#[cfg(test)]` in `migration.rs` with sample config files |
| **Unit: migration rollback** | If proxy verification fails after migration, original config is restored exactly | `#[cfg(test)]` with mock config write/restore |
| **Integration: full migration** | Start with 3 direct-installed servers → run migration → verify vault has keys → verify proxy configs in place → verify tools work | `tests/migration_integration.rs` |
| **Manual: migration UX** | Walk through migration UI with real existing configs. Verify import, verify proxy works, verify rollback. | Manual checklist |
| **Manual: cross-platform** | Full end-to-end test of all features on macOS, Windows, Linux | Manual test matrix |

### Phase 7: Curated Profiles & Scoreboard Integration (Separate Track, Decision D9)

**Goal:** Community-maintained tool profiles and Scoreboard integration. Does not gate v1 launch.

**Deliverables:**
1. Curated tool profiles
   - Profile JSON format and local storage (`~/.brightwing/profiles/`)
   - Scoreboard API endpoint for profile distribution
   - Manager checks for profiles on server add and periodically
   - `bw list` and `bw --help` prefer curated descriptions when available
   - "Apply Profile" button in ToolFilterPanel
   - Five initial profiles: GitHub, Jira, Linear, Slack, Brave Search
2. Scoreboard integration
   - Context Efficiency scoring dimension
   - "Available via bw" badge on server pages
   - Context Budget Calculator widget
   - Profile submission flow for community contributions
   - Use Scoreboard install config to pre-fill upstream transport/URL/command
   - One-click "Add + Authenticate + Install to All Tools" flow

**Files created:**
- `src-tauri/src/profiles.rs` (new)

**Testing — Phase 7:**

| Test Type | What to Test | How |
|-----------|-------------|-----|
| **Unit: profile parsing** | Curated profile JSON parses correctly. Falls back to raw schema when no profile exists. | `#[cfg(test)]` in `profiles.rs` |
| **Unit: profile application** | Applying a profile correctly updates `proxy_tool_filter` via `set_tool_filter_bulk` | Unit test |
| **Integration: profile distribution** | Mock Scoreboard API → request profile → verify cached locally → verify `bw --help` uses curated descriptions | Integration test |
| **Manual: Scoreboard integration** | Verify Context Efficiency score displays correctly. Verify "Available via bw" badge. Verify Budget Calculator. | Manual checklist on staging |

---

## Test Infrastructure

### Automated Test Stack

| Layer | Tool | Purpose |
|-------|------|---------|
| **Rust unit tests** | `cargo test` | Core logic: IPC types, parsing, filter logic, OAuth, vault, migration |
| **Rust integration tests** | `cargo test --test '*'` + `wiremock`/`httpmock` | Multi-component flows: proxy ↔ daemon ↔ mock upstream |
| **Binary tests** | `assert_cmd` + `predicates` crates | Verify binary startup, arg parsing, error messages, exit codes |
| **React component tests** | Vitest + React Testing Library | UI components: ToolFilterPanel, Dashboard, OAuthConnect, DaemonStatus |
| **E2E tests** | Playwright or Tauri's WebDriver + real MCP servers | Full GUI flows: install server → configure → verify in tool |

### Mock MCP Server (Built First — Phase 1, Decision D5)

A reusable mock MCP server for all integration testing. Built as the first deliverable in Phase 1 to unblock everything downstream.

```rust
// tests/mock_mcp_server.rs
//
// Two transport modes:
// 1. HTTP mode: wiremock-based server with configurable endpoints
//    - POST /mcp → JSON-RPC dispatch
//    - Handles: initialize, tools/list, tools/call, ping
//    - Configurable tool set and canned responses
//    - Returns realistic JSON-RPC responses with proper IDs
//
// 2. stdio mode: spawnable child binary
//    - Reads JSON-RPC from stdin, writes to stdout
//    - Same configurable tool set
//    - Used for testing stdio upstream proxy mode
//
// Builder pattern for test setup:
//   MockMcpServer::builder()
//       .with_tools(vec![mock_search_repos(), mock_list_issues()])
//       .with_auth_requirement("Bearer")
//       .build_http().await  // or .build_stdio()
```

### Headless OAuth Test Harness (Phase 3, Decision D6)

Automated OAuth flow testing without a real browser:

```rust
// tests/oauth_test_harness.rs
//
// 1. Start mock OAuth server (wiremock) with:
//    - /.well-known/oauth-authorization-server metadata
//    - /authorize endpoint (not actually used in headless mode)
//    - /token endpoint for code exchange and refresh
//    - Optional: /register for dynamic client registration
//
// 2. Start the OAuth flow programmatically (get auth URL + callback port)
// 3. Skip browser — POST directly to localhost:{callback_port}/callback?code=mock_code&state=...
// 4. Verify token exchange against mock OAuth server
// 5. Verify tokens stored in VaultBackend
```

### CI Pipeline

```yaml
# Runs on every PR (all platforms)
test:
  - cargo fmt --check
  - cargo clippy -- -D warnings
  - cargo test                      # Unit + integration tests
  - cargo test --test '*'           # Integration tests only
  - npm run test                    # React component tests
  - cargo build --release --bin brightwing-proxy
  - cargo build --release --bin bw
  - cargo build --release --bin brightwing-authd
  # Binary smoke tests
  - ./target/release/bw --help
  - ./target/release/brightwing-proxy --help

# Platform-specific daemon lifecycle tests (Phase 4)
daemon-lifecycle:
  matrix:
    os: [macos-latest, windows-latest, ubuntu-latest]
  steps:
    - cargo build --release --bin brightwing-authd
    - cargo build --release --bin brightwing-proxy
    # Test auto-start install/uninstall
    # Test daemon start/stop/restart
    # Test crash recovery (kill -9 → verify restart)
    # Test proxy reconnection after daemon restart
```

### Manual Test Checklists

Each phase includes manual test checklists stored in `tests/manual/`:

```
tests/manual/
├── phase-1-foundation.md
├── phase-2-credentials-filtering.md
├── phase-3-oauth.md
├── phase-4-daemon-lifecycle.md
├── phase-5-cli-shim-stdio.md
├── phase-6-migration-polish.md
├── phase-7-profiles-scoreboard.md
└── cross-platform-matrix.md
```

Each checklist includes:
- Prerequisites (servers/accounts needed)
- Step-by-step instructions
- Expected results
- Platform-specific notes (macOS, Windows, Linux)
- Pass/fail checkboxes

---

## Edge Cases

### Tool Filtering
- **Tool list changes upstream:** Daemon reconciles on reconnect. New tools default enabled. Removed tools cleaned up. Existing filter choices preserved.
- **Multiple AI tools, different filters:** v1: one filter per server, applied uniformly. Per-tool-per-client filtering deferred.
- **Agent calls filtered tool:** Shouldn't happen (tool not in `tools/list`). If it does, upstream handles the error. Proxy doesn't block `tools/call`.
- **Zero tools enabled:** Valid state. Proxy returns empty tools list. Dashboard shows "0 of N tools enabled."

### CLI Shim
- **Agents don't discover `bw`:** Documentation injection into CLAUDE.md etc. is essential. Agents also discover CLIs on PATH via exploration.
- **Parameter type mismatches:** `bw` does automatic type coercion (int/bool first, fallback string).
- **Complex nested parameters:** `--json-input '{"nested": {"key": "val"}}'` flag for complex cases.
- **Daemon not running:** Clear error message, no credential cache fallback (D8). Daemon reliability via OS service manager is the mitigation.

### Proxy
- **PATH issues:** Use absolute paths in config files as primary strategy. PATH as fallback.
- **OAuth server diversity:** Start with well-documented servers. Build compatibility database.
- **Token refresh races:** Refresh only done by daemon, never by proxy. Mutex around refresh.

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| **Daemon complexity** — managing a background service adds engineering and support burden | High | High | Build daemon from Phase 1 (D1). Avoids in-process→daemon refactoring. Auto-restart via OS service managers. |
| **PATH issues** — proxy binary not found by AI tools, especially on macOS | High | High | Absolute paths in config files as primary strategy (D4). PATH as fallback. |
| **Agents don't discover `bw`** — agent doesn't know the tool exists | High | High | Documentation injection into CLAUDE.md etc. is essential, not optional. |
| **OAuth server diversity** — each server's OAuth implementation has quirks | Medium | High | Start with well-documented servers (GitHub, Google, Slack). Build compatibility database. |
| **Token refresh races** — multiple proxy instances refreshing same token | Medium | Medium | Refresh only in daemon, never in proxy. Mutex around refresh. |
| **Parameter type mismatches** — agent passes wrong types to `bw` | Medium | Medium | Automatic type coercion based on JSON Schema. |
| **Binary distribution** — installing standalone binaries to `~/.local/bin/` | Low | Medium | Standalone builds (D3) are simpler than sidecars. Absolute paths (D4) avoid PATH issues. |
| **User confusion** — "Why do I need a background service?" | Medium | Low | Clear onboarding UI. Make it optional — users can still do direct installs. |
| **Docker MCP Gateway competition** | Low | Medium | Docker requires Docker Desktop (heavy). Brightwing is lightweight. Different segment. |
| **Stronghold deprecation in Tauri v3** | Low | Medium | `VaultBackend` trait from Phase 1 (D2). Swap implementations when needed. |
| **MCP protocol changes** | Low | Medium | Daemon abstracts the protocol. `bw` and proxy only talk to daemon. |

---

## Success Metrics

### Phase 1 Launch ✅
- ✅ Mock MCP server supports HTTP transport, used by all integration tests (9 mock tests + 5 proxy integration tests)
- ✅ `brightwing-authd` daemon starts, listens on IPC socket, serves credentials
- ✅ `brightwing-proxy` successfully proxies HTTP MCP servers via daemon
- ✅ IPC handshake with version checking works
- ✅ `bw` arg parsing and output formatting unit tests passing (24 tests)
- ✅ `VaultBackend` trait with `InMemoryVaultBackend` fully tested (10 tests)
- ✅ All Phase 1 automated tests passing

### Phase 2 Launch ✅
- ✅ `StrongholdBackend` implements `VaultBackend` trait (8 feature-gated tests)
- ✅ Binary distribution commands: `distribute_binaries`, `get_binary_versions`
- ✅ Tool filter panel with token budget bar, per-tool checkboxes, search, enable/disable all
- ✅ Filter round-trip tested: update via IPC → proxy reflects change on next request
- ✅ Token estimation utility in `proxy-common/src/tokens.rs`
- ✅ Dashboard shows proxy server summary with auth badges and token budget bars
- ✅ API Keys panel: CRUD for env var credentials, reveal/hide, status badges
- ✅ Proxy server management: register, install to tools, tool filter, remove
- ✅ All Phase 2 automated tests passing (69 fast + 8 Stronghold feature-gated)

### Phase 3 Launch ✅
- ✅ Headless OAuth test harness covers full token-exchange flow (14 integration tests)
- ✅ Token refresh lifecycle implemented (proactive + reactive on 401)
- ✅ Zero plaintext secrets in any tool config file for proxied servers
- ✅ Desktop notifications on token expiry
- ✅ 36 new Phase 3 tests (21 unit + 14 integration + 1 proxy 401 retry)

### Phase 4 Launch ✅
- ✅ Daemon PID file management, duplicate detection, graceful shutdown
- ✅ IPC Ping/Pong health check with uptime tracking
- ✅ GUI start/stop daemon, status display with uptime/PID/version
- ✅ Auto-start support: macOS (launchd) and Linux (systemd)
- ✅ DaemonStatus.tsx with auto-refresh, start/stop controls, auto-start toggle
- ✅ 85 cumulative tests passing

### Phase 5 Launch
- `bw list` returns all connected servers with tool counts
- `bw <server> <tool> --help` displays parameter info
- `bw <server> <tool>` executes and returns results
- Latency: <50ms for `bw list`, <200ms overhead for `bw <tool>` (plus upstream latency)
- stdio upstream proxy works for `npx`/`uvx` servers with credential injection

### Phase 6 Launch (v1 Release Gate)
- Migration assistant tested with configs containing 10+ servers
- Cross-platform end-to-end tests passing

### Phase 7 (Post-v1, Separate Track)
- Curated profiles working for 5+ servers
- Scoreboard integration: one-click "Add + Auth + Install" flow

### 1 Month Post-Launch
- 50%+ of Brightwing users have at least one proxied server
- 40%+ of users with proxied servers have customized at least one tool filter
- 30%+ of users have used `bw` at least once
- Average `bw` invocation latency <100ms (excluding upstream)

### 3 Months Post-Launch
- Average of 3+ proxied servers per active user
- `bw` used more frequently than stdio proxy by Claude Code / Codex users
- Token budget feature cited in user feedback
- Context Efficiency score on Scoreboard driving awareness
- Community-contributed profiles for 10+ servers
- Zero reported credential leaks from config files

---

## Open Questions

1. **Proxy naming convention:** Should proxied servers use a `bw-` prefix (e.g., `bw-github-mcp`) or keep the original name? Prefix makes proxied servers obvious but may confuse users who reference server names in prompts.

2. **Connection multiplexing:** If 5 tools all connect to the same upstream via 5 proxy instances, that's 5 separate upstream connections. Should the daemon multiplex these? (Probably not for v1.)

3. **Proxy for local no-auth servers:** Offer proxy mode for local servers without auth? Small latency overhead but enables centralized logging/monitoring. (Opt-in only.)

4. **Scoreboard auth metadata:** Should the Scoreboard API provide OAuth endpoint URLs and client IDs? This would make adding OAuth servers zero-config. (Yes — future Scoreboard enhancement.)

5. **MCP protocol version pinning:** Should the proxy advertise a specific MCP protocol version independent of upstream? Could help with cross-tool compatibility.

6. **Should `bw` support piping?** Probably yes by default — stdout is text/JSON, piping works for free. Ensure debug/error goes to stderr only.

7. **Interactive confirmation for destructive tools?** If a profile marks tools as destructive, should `bw` prompt "Are you sure?" on stderr? `--yes` flag to skip for non-interactive agent usage.

8. **Daemon HTTP API?** A local HTTP API (`localhost:7483/call/github/search_repos?query=mcp`) would make `bw` accessible from languages that can't easily exec shell commands. Low priority, architecturally trivial.

9. **Shell auto-complete for `bw`?** Completions generated from cached tool schemas would improve human usage. Worth doing but not for v1.

10. **Tool filtering per-AI-tool or global per-server?** v1 is global. Per-client filtering deferred unless users request it.

11. **Should the proxy block `tools/call` for filtered tools?** Probably not — adds enforcement complexity for a scenario that shouldn't occur. Optional safety mode for v2.

12. **Preset filter templates beyond curated profiles?** e.g., "Read Only" (auto-disable write tools), "Minimal" (top 5 by usage). Nice-to-have for v2.
