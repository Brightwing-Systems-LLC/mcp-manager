# Auth Plan: API Key Vault & OAuth Detection

## Overview

Two features:
1. **Encrypted API Key Vault** — Securely store MCP server API keys/secrets in-app, auto-inject them into tool configs on install/enable. Users can view, edit, and manage keys from a dedicated "API Keys" sidebar panel.
2. **OAuth Detection & Guidance** — Detect OAuth-based MCP servers and show users clear instructions to authenticate inside each tool.

---

## Part 1: Encrypted API Key Vault

### 1.1 Storage Backend

**Choice: `tauri-plugin-stronghold`** (IOTA Stronghold)
- Encrypted at rest using Argon2-derived key
- No user password prompt needed (salt file + app-scoped key)
- Official Tauri v2 plugin, well-documented
- Will need migration in Tauri v3 (stronghold deprecated), but that's a future concern

**Setup:**
```toml
# src-tauri/Cargo.toml
[dependencies]
tauri-plugin-stronghold = "2"

# IMPORTANT: Prevent extremely slow debug builds
[profile.dev.package.scrypt]
opt-level = 3
```

**Permissions:**
```json
// src-tauri/capabilities/default.json
{ "permissions": ["stronghold:default"] }
```

**Init in lib.rs:**
```rust
.setup(|app| {
    let salt_path = app.path().app_local_data_dir()?
        .join("salt.txt");
    app.handle().plugin(
        tauri_plugin_stronghold::Builder::with_argon2(&salt_path).build()
    )?;
    Ok(())
})
```

### 1.2 Data Model

Keys are stored in Stronghold as serialized JSON, keyed by a composite identifier. Keys are strictly per-server (no shared/global keys). If two servers need the same API key, each stores its own copy.

**Vault key format:** `secret:{server_name}:{env_var_name}`
- Example: `secret:my-github-mcp:GITHUB_TOKEN`

**Vault metadata** (stored in SQLite, NOT Stronghold — no secrets here):
```sql
CREATE TABLE api_key_meta (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_name TEXT NOT NULL,        -- "my-github-mcp"
    env_var_name TEXT NOT NULL,       -- "GITHUB_TOKEN"
    description TEXT,                 -- "GitHub personal access token"
    sensitive INTEGER DEFAULT 1,      -- from env_schema
    required INTEGER DEFAULT 1,       -- from env_schema
    source TEXT,                      -- "scoreboard", "manual", "scan"
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    UNIQUE(server_name, env_var_name)
);
```

**Auth type overrides** (stored in SQLite, user-correctable):
```sql
CREATE TABLE server_auth_type (
    server_name TEXT PRIMARY KEY,
    auth_type TEXT NOT NULL,          -- "oauth", "api_key", "none"
    set_by TEXT NOT NULL,             -- "heuristic", "user"
    updated_at TEXT DEFAULT (datetime('now'))
);
```

This separation means:
- SQLite tells us *what* keys exist, their descriptions, which servers need them
- SQLite also stores auth type classification per server (heuristic or user-overridden)
- Stronghold holds the *actual secret values*, encrypted at rest
- The UI can list keys without decrypting anything

### 1.3 Tauri Commands

```rust
/// List all stored key metadata (no secret values).
#[tauri::command]
fn list_api_keys(db: State<Database>) -> Result<Vec<ApiKeyMeta>, String>

/// Get a single key's decrypted value (for eye-toggle reveal).
#[tauri::command]
fn get_api_key_value(
    server_name: String,
    env_var_name: String,
    stronghold: State<Stronghold>,
) -> Result<String, String>

/// Store or update a key.
#[tauri::command]
fn set_api_key(
    server_name: String,
    env_var_name: String,
    value: String,
    description: Option<String>,
    sensitive: bool,
    required: bool,
    source: String,           // "scoreboard" | "manual" | "scan"
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<(), String>

/// Delete a key.
#[tauri::command]
fn delete_api_key(
    server_name: String,
    env_var_name: String,
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<(), String>

/// Get all keys for a given server (for injection during install).
#[tauri::command]
fn get_server_keys(
    server_name: String,
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<HashMap<String, String>, String>  // env_var_name -> decrypted value

/// Get auth type for a server.
#[tauri::command]
fn get_server_auth_type(
    server_name: String,
    db: State<Database>,
) -> Result<Option<ServerAuthType>, String>

/// Set auth type for a server (user override or heuristic).
#[tauri::command]
fn set_server_auth_type(
    server_name: String,
    auth_type: String,        // "oauth" | "api_key" | "none"
    set_by: String,           // "heuristic" | "user"
    db: State<Database>,
) -> Result<(), String>

/// Export the vault as an encrypted file.
#[tauri::command]
fn export_vault(
    export_path: String,
    password: String,
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<String, String>

/// Import a vault from an encrypted export file.
#[tauri::command]
fn import_vault(
    import_path: String,
    password: String,
    db: State<Database>,
    stronghold: State<Stronghold>,
) -> Result<String, String>   // "Imported N keys"
```

### 1.4 TypeScript Layer

```typescript
// src/lib/tauri.ts
export interface ApiKeyMeta {
  server_name: string;
  env_var_name: string;
  description: string | null;
  sensitive: boolean;
  required: boolean;
  source: string;
  created_at: string;
  updated_at: string;
}

export interface ServerAuthType {
  server_name: string;
  auth_type: "oauth" | "api_key" | "none";
  set_by: "heuristic" | "user";
}

export async function listApiKeys(): Promise<ApiKeyMeta[]>;
export async function getApiKeyValue(serverName: string, envVarName: string): Promise<string>;
export async function setApiKey(params: {
  serverName: string; envVarName: string; value: string;
  description?: string; sensitive?: boolean; required?: boolean; source: string;
}): Promise<void>;
export async function deleteApiKey(serverName: string, envVarName: string): Promise<void>;
export async function getServerKeys(serverName: string): Promise<Record<string, string>>;
export async function getServerAuthType(serverName: string): Promise<ServerAuthType | null>;
export async function setServerAuthType(serverName: string, authType: string, setBy: string): Promise<void>;
export async function exportVault(exportPath: string, password: string): Promise<string>;
export async function importVault(importPath: string, password: string): Promise<string>;
```

### 1.5 View Type

Add `"api-keys"` to the `View` union type in `src/lib/types.ts`.

### 1.6 Navigation

Add to `navItems` in `Navigation.tsx`, positioned above "About":
```typescript
{ id: "api-keys", label: "API Keys", icon: "key" }
```

Icon: key SVG (Heroicons `key` icon).

### 1.7 API Keys Panel (`src/components/ApiKeys.tsx`)

**Layout:**
```
API Keys                          [Export]  [Import]  [+ Add Key]
─────────────────────────────────────────────────────────────────
🔒 All keys are encrypted at rest using IOTA Stronghold.
Keys are injected into tool configs when servers are installed
or enabled.
─────────────────────────────────────────────────────────────────

my-github-mcp (2 keys)
┌─────────────────────────────────────────────────────┐
│ GITHUB_TOKEN          ••••••••••••    👁  ✏️  🗑    │
│ GitHub personal access token                        │
│ GITHUB_ORG            ••••••••••••    👁  ✏️  🗑    │
│ Organization name                                   │
└─────────────────────────────────────────────────────┘

sentry-mcp (1 key)
┌─────────────────────────────────────────────────────┐
│ SENTRY_AUTH_TOKEN     ••••••••••••    👁  ✏️  🗑    │
│ Sentry authentication token                         │
└─────────────────────────────────────────────────────┘
```

**Interactions:**
- **Eye icon**: Toggle reveal/hide. Calls `getApiKeyValue()` on first reveal, caches in component state. Clears cache after 30s auto-hide timeout.
- **Edit icon**: Inline edit mode — password input field replaces masked value. Save/cancel buttons appear.
- **Delete icon**: Confirmation prompt ("Delete GITHUB_TOKEN for my-github-mcp?"), then `deleteApiKey()`.
- **+ Add Key button**: Opens a small form — server name (autocomplete from known servers), env var name, value, description.
- **Export button**: Opens OS save dialog, prompts for an export password, calls `exportVault()`. Saves an encrypted `.bwvault` file.
- **Import button**: Opens OS file dialog, prompts for the export password, calls `importVault()`. Merges keys (existing keys are not overwritten unless user confirms).

**Empty state:**
```
No API keys stored yet.

Keys are captured automatically when you install servers
from the Scoreboard or Add Server. You can also add them
manually here.
```

### 1.8 Key Capture Points

#### A. Scoreboard Install (InstallDialog.tsx)

Current flow already prompts for env vars via `env_schema`. Modify:

1. Before showing the env var form, check vault for existing keys:
   ```typescript
   const existingKeys = await getServerKeys(serverConfig.config_key);
   // Pre-fill env var inputs with vault values
   ```

2. After user fills in env vars and clicks Install, offer to save:
   ```
   ┌──────────────────────────────────────────────┐
   │  🔒 Save API keys securely?                  │
   │                                               │
   │  We can encrypt and store these keys so you   │
   │  won't need to enter them again when adding   │
   │  this server to other tools.                  │
   │                                               │
   │  [Save & Install]  [Skip, just install]       │
   └──────────────────────────────────────────────┘
   ```

3. If "Save & Install": call `setApiKey()` for each env var, then proceed with install.
4. If "Skip": proceed with install as-is, no vault storage.

#### B. Manual Add Server (AddServer.tsx)

Same pattern as scoreboard install. If user provides env vars in the manual form:
- Offer to save to vault after successful install
- Pre-fill from vault if keys already exist for that server name

#### C. Cross-Install from Grid (Dashboard.tsx)

When adding a server to a new tool via checkbox:

1. **On save**, before writing config, check if the source config has env vars.
2. If env vars exist in the source config → also save them to vault (source: "scan") if not already there.
3. If env vars are in vault but NOT in the source config (rare) → inject from vault.
4. If neither has env vars and the server is known to need them (from scoreboard env_schema) → prompt:
   ```
   "sentry-mcp" requires SENTRY_AUTH_TOKEN. Enter it now or add it later in API Keys.
   [Enter Key]  [Skip]
   ```

#### D. Initial Config Scan (on app launch)

Add a one-time scan on first launch (or on "Rescan Configs"):

1. Read all configured servers across all tools
2. For each server with env vars in its config JSON
3. Check if those keys are already in the vault
4. If not, silently import them (source: "scan")
5. Show a one-time toast: "Imported N API keys from existing configs into secure storage"

This ensures the vault is populated from the start without user effort.

### 1.9 Key Injection During Install/Enable

Modify the install and enable flows to auto-inject vault keys:

**In `handleSave()` (Dashboard grid):**
```typescript
// Before writing config for "add" action:
if (change.action === "add") {
  let configJson = /* ... get config from source ... */;

  // Inject vault keys into config
  const vaultKeys = await getServerKeys(change.serverName);
  if (Object.keys(vaultKeys).length > 0) {
    const config = JSON.parse(configJson);
    config.env = { ...(config.env || {}), ...vaultKeys };
    configJson = JSON.stringify(config);
  }

  await addServerToTool(change.toolId, change.serverName, configJson);
}
```

**In `install_server` / `restore_server_entry` (Rust):**
No changes needed — the frontend injects keys into config JSON before sending to backend.

### 1.10 Encrypted Config Backups

Currently `backup_tool_config` saves plaintext copies of tool config files. Since these contain API keys, they must be encrypted.

**Changes to `config/backup.rs`:**
- Accept a Stronghold reference (or a derived encryption key)
- Encrypt the backup file content before writing to disk
- Decrypt on restore
- Backup file extension: `.bak.enc` instead of `.bak`
- Use the same Stronghold-derived key that protects the vault

**Backup format:**
```
[4 bytes: nonce length][nonce][encrypted config content]
```

Use `aes-gcm` or similar from Stronghold's underlying crypto. The key is derived from the same Argon2 salt used by Stronghold, ensuring backups are tied to this installation.

### 1.11 Vault Export/Import

Users can export their vault for backup or migration to another machine.

**Export flow:**
1. User clicks "Export" in the API Keys panel
2. OS save dialog opens (default filename: `mcp-manager-keys-YYYY-MM-DD.bwvault`)
3. User enters an export password (separate from the app's internal Stronghold key)
4. All key metadata + encrypted values are serialized and encrypted with the export password
5. Written to the `.bwvault` file

**Import flow:**
1. User clicks "Import" in the API Keys panel
2. OS file dialog opens (filter: `*.bwvault`)
3. User enters the export password
4. Keys are decrypted and merged into the vault
5. On conflict (same server_name + env_var_name already exists), show a merge dialog:
   ```
   "GITHUB_TOKEN" for "my-github-mcp" already exists.
   [Keep existing]  [Overwrite with imported]  [Keep both — rename imported]
   ```
6. Toast: "Imported N keys successfully"

**Export file format (JSON, then encrypted):**
```json
{
  "version": 1,
  "exported_at": "2026-03-08T...",
  "keys": [
    {
      "server_name": "my-github-mcp",
      "env_var_name": "GITHUB_TOKEN",
      "value": "ghp_xxx...",
      "description": "GitHub personal access token",
      "sensitive": true,
      "required": true
    }
  ]
}
```

The entire JSON blob is encrypted with the user's export password using Argon2 key derivation + AES-256-GCM.

### 1.12 Security Considerations

- **Stronghold file**: Stored at `~/.local/share/com.brightwing.mcp-manager/vault.hold`, encrypted with Argon2-derived key
- **Salt file**: `~/.local/share/com.brightwing.mcp-manager/salt.txt` — unique per installation
- **In-memory exposure**: Decrypted values exist in memory only during active reveal or install operations
- **Config files remain plaintext**: This is by design — tools (Claude Desktop, Cursor, etc.) require plaintext env vars in their config files. The vault's job is to be the secure *source of truth*; config files are the *deployment targets*.
- **Config backups are encrypted**: Unlike the tool config files themselves (which we don't control), our backup copies are always encrypted at rest.
- **No clipboard**: Don't offer "copy to clipboard" for secrets (clipboard is shared, insecure)
- **Reveal timeout**: Auto-hide revealed values after 30 seconds
- **Export files**: Encrypted with a user-chosen password, safe to store in cloud drives or email to self

---

## Part 2: OAuth Detection & Guidance

### 2.1 Detection & Classification

OAuth detection is managed entirely in-app using a combination of heuristics and user overrides. No scoreboard dependency.

**Heuristic (applied on scan):**

An MCP server is classified as OAuth-based if:
```typescript
function detectAuthType(serverName: string, configJson: string | null): "oauth" | "api_key" | "none" {
  if (!configJson) return "none";
  const config = JSON.parse(configJson);

  // Has a URL (HTTP/SSE transport) + no command + no env vars → OAuth
  const hasUrl = !!config.url;
  const isNotStdio = !config.command;
  const hasNoEnvKeys = !config.env || Object.keys(config.env).length === 0;

  if (hasUrl && isNotStdio && hasNoEnvKeys) return "oauth";

  // Has env vars → API key auth
  if (config.env && Object.keys(config.env).length > 0) return "api_key";

  return "none";
}
```

**Persistence:**
- On each config scan, run the heuristic for every server
- Store result in `server_auth_type` table with `set_by: "heuristic"`
- If user has manually overridden (`set_by: "user"`), the heuristic does NOT overwrite it
- User can correct misclassifications from the grid UI (right-click or icon menu)

**User override:**
If the heuristic is wrong (e.g., an HTTP server that uses API keys, not OAuth), the user can click the auth type indicator in the grid and change it:
```
┌──────────────────────────────────────────┐
│  How does this server authenticate?      │
│                                          │
│  ○ OAuth (in-app browser auth)           │
│  ○ API Key (environment variable)        │
│  ○ None (no auth required)               │
│                                          │
│  [Save]                                  │
└──────────────────────────────────────────┘
```

### 2.2 UI Treatment in Grid

For OAuth servers in the grid, show a small link icon next to the checkbox:

```
┌───────────────────────────────────────────────────────────┐
│ Server              │  CU  │  VC  │  CC  │  CX  │  GC   │
├─────────────────────┼──────┼──────┼──────┼──────┼───────│
│ claude.ai Gmail     │  ☑🔗 │  ☐🔗 │  ☑🔗 │      │       │
│ sentry-mcp          │  ☑   │  ☑   │  ☑   │      │       │
└───────────────────────────────────────────────────────────┘
```

The link icon (small SVG, not emoji) indicates "requires in-app authentication."

**On hover/click of the link icon**, show a tooltip or popover:

```
┌──────────────────────────────────────────────┐
│  OAuth Authentication Required               │
│                                               │
│  After enabling, open Cursor and go to:       │
│  Settings → MCP Servers → claude.ai Gmail     │
│  Click "Authenticate" to connect.             │
└──────────────────────────────────────────────┘
```

### 2.3 Post-Install OAuth Reminder

After saving changes that include OAuth servers, show a summary modal instead of just a toast:

```
┌──────────────────────────────────────────────────────┐
│  Saved 3 changes successfully                        │
│                                                       │
│  1 server requires authentication:                    │
│                                                       │
│  • claude.ai Gmail → Cursor                          │
│    Open Cursor → Settings → MCP → Authenticate       │
│                                                       │
│                                        [Got it]       │
└──────────────────────────────────────────────────────┘
```

If no OAuth servers were involved, show the normal success toast instead.

### 2.4 Tool-Specific Auth Instructions

Map each tool to its settings path for MCP authentication:

```typescript
const AUTH_INSTRUCTIONS: Record<string, string> = {
  cursor: "Settings → MCP Servers → select server → Authenticate",
  vscode: "Settings (Ctrl+,) → search 'MCP' → select server → Authenticate",
  windsurf: "Settings → MCP → select server → Authenticate",
  claude_code: "Run: claude mcp auth <server-name>",
  codex: "Settings → MCP Servers → select server → Authenticate",
};
```

---

## Implementation Order

### Phase 1: Vault Foundation
1. Add `tauri-plugin-stronghold` dependency and init
2. Add `api_key_meta` and `server_auth_type` tables to SQLite
3. Implement Tauri commands (list, get, set, delete, get_server_keys)
4. Add TypeScript wrappers
5. Add `"api-keys"` view type and navigation entry
6. Build `ApiKeys.tsx` panel (list, reveal, edit, delete, add)

### Phase 2: Key Capture Integration
7. Modify `InstallDialog.tsx` — pre-fill from vault, offer to save
8. Modify `AddServer.tsx` — same pattern
9. Modify `Dashboard.tsx` grid save — inject vault keys on cross-install
10. Add initial config scan to import existing keys on launch

### Phase 3: Encrypted Backups
11. Encrypt config backup files using Stronghold-derived key
12. Update backup/restore flow to handle encrypted format
13. Maintain backward compat (detect unencrypted backups, offer to re-encrypt)

### Phase 4: Vault Export/Import
14. Implement `export_vault` command with password-based encryption
15. Implement `import_vault` command with merge conflict handling
16. Add Export/Import buttons to API Keys panel with OS file dialogs

### Phase 5: OAuth Detection
17. Implement `detectAuthType()` heuristic
18. Run heuristic on config scan, persist to `server_auth_type` table
19. Add link icon + tooltip to grid cells for OAuth servers
20. Add post-install OAuth reminder modal
21. Add tool-specific auth instruction map
22. Add user override UI for correcting auth type classification

### Phase 6: Polish
23. Auto-hide revealed keys after 30s timeout
24. Empty state messaging
25. Vault status in About page ("N keys securely stored")
26. Edge cases: key conflicts, server rename, key rotation prompts

---

## Files to Create/Modify

### New Files
- `src/components/ApiKeys.tsx` — API Keys management panel
- `src-tauri/src/vault.rs` — Stronghold wrapper module

### Modified Files
- `src-tauri/Cargo.toml` — add stronghold + aes-gcm dependencies
- `src-tauri/src/lib.rs` — stronghold init, new commands, register handlers
- `src-tauri/src/db/queries.rs` — api_key_meta + server_auth_type tables and queries
- `src-tauri/src/config/backup.rs` — encrypt backups
- `src-tauri/capabilities/default.json` — stronghold permissions
- `src/lib/types.ts` — View type, ApiKeyMeta, ServerAuthType interfaces
- `src/lib/tauri.ts` — new invoke wrappers
- `src/store.ts` — api keys state (list, loading)
- `src/components/Navigation.tsx` — add API Keys nav item
- `src/App.tsx` — render ApiKeys view
- `src/components/Dashboard.tsx` — vault injection on save, OAuth indicators, auth type override
- `src/components/InstallDialog.tsx` — pre-fill from vault, save-to-vault prompt
- `src/components/AddServer.tsx` — same as InstallDialog
