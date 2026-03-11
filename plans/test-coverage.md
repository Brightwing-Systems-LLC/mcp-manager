# Test Coverage Improvement Plan

## Current State

**~130 test functions** across Rust backend; **0 frontend tests**.

Coverage is concentrated in IPC serialization (31), CLI shim (24), OAuth sub-modules (19), and integration tests (40). Major gaps exist in config management, database operations, tool scanning, the auth daemon, and the entire React frontend.

---

## Phase 1: Database Query Tests

**File:** `src-tauri/src/db/queries.rs`
**Test location:** Add `#[cfg(test)] mod tests` at bottom of `queries.rs`
**Why first:** Database is the foundation — every other module depends on it. `Database::new_in_memory()` already exists, making these tests easy to write with zero mocking.

### 1.1 Installation CRUD (4 tests)

| Test | Description |
|------|-------------|
| `test_record_and_get_installation` | `record_installation()` then `get_installations()` returns the entry with correct fields |
| `test_record_installation_unique_constraint` | Recording same `(server_uuid, tool_id)` twice succeeds (UPSERT or no error) |
| `test_remove_installation` | `remove_installation()` removes entry; `get_installations()` returns empty |
| `test_multiple_installations_different_tools` | Same server installed to 3 tools → 3 entries returned |

### 1.2 Favorites CRUD (4 tests)

| Test | Description |
|------|-------------|
| `test_add_and_get_favorite` | Add favorite with all optional fields, retrieve it, verify all fields |
| `test_add_favorite_minimal` | Add favorite with `None` for optional fields |
| `test_remove_favorite` | Add then remove → `get_favorites()` returns empty |
| `test_favorite_duplicate_uuid` | Adding same UUID twice succeeds (idempotent or replaces) |

### 1.3 Disabled Servers (4 tests)

| Test | Description |
|------|-------------|
| `test_disable_and_get_server` | `disable_server()` stores config JSON, `get_disabled_servers()` returns it |
| `test_enable_server_returns_config` | `enable_server()` returns the stored `config_json` and removes the record |
| `test_enable_nonexistent_server` | `enable_server()` for unknown server returns error |
| `test_disable_unique_constraint` | Same `(tool_id, server_name)` disabled twice → last wins or errors gracefully |

### 1.4 Proxy Server Registration (5 tests)

| Test | Description |
|------|-------------|
| `test_register_and_get_proxy_server` | Register with all fields, retrieve by ID, verify all fields |
| `test_register_proxy_server_http` | Register HTTP upstream (url set, command/args None) |
| `test_register_proxy_server_stdio` | Register stdio upstream (command/args set, url None) |
| `test_unregister_cascades` | Unregister → tool_filter, tool_cache, tool_installs, oauth, api_keys all deleted |
| `test_get_proxy_server_not_found` | `get_proxy_server("nonexistent")` returns `Ok(None)` |

### 1.5 Tool Filter (5 tests)

| Test | Description |
|------|-------------|
| `test_set_and_get_tool_filter` | Set individual tool filter, retrieve, verify enabled/disabled state |
| `test_set_tool_filter_bulk` | Bulk set 3 tools enabled → only those 3 enabled, rest disabled |
| `test_set_tool_filter_bulk_overwrites` | Bulk set, then bulk set different list → only new list enabled |
| `test_get_tool_filter_empty` | No filters set → returns empty vec |
| `test_tool_filter_toggle` | Set enabled=true then enabled=false → verify final state |

### 1.6 Tool Schema Cache (3 tests)

| Test | Description |
|------|-------------|
| `test_cache_and_get_tools` | Cache 3 tool schemas, `get_cached_tools()` returns all 3 with correct descriptions and schemas |
| `test_cache_tool_creates_filter_entry` | Caching a tool auto-creates a tool_filter entry (default: enabled) |
| `test_cache_tool_overwrite` | Caching same tool name twice → latest description/schema wins |

### 1.7 OAuth Metadata & Tokens (5 tests)

| Test | Description |
|------|-------------|
| `test_store_and_get_oauth_metadata` | Store metadata JSON, retrieve, verify |
| `test_delete_oauth_metadata` | Store then delete → `get_oauth_metadata()` returns `None` |
| `test_store_and_get_token_set` | Store token JSON, retrieve, verify |
| `test_delete_oauth_token_set` | Store then delete → returns `None` |
| `test_oauth_data_cascade_on_unregister` | Unregistering proxy server deletes associated oauth metadata and tokens |

### 1.8 API Key Management (4 tests)

| Test | Description |
|------|-------------|
| `test_store_and_get_api_key` | Store env map `{"API_KEY": "abc"}`, retrieve, verify |
| `test_delete_api_key` | Store then delete → `get_api_key()` returns `None` |
| `test_get_all_api_keys` | Store keys for 3 servers → `get_all_api_keys()` returns all 3 |
| `test_api_key_overwrite` | Store then store again with different value → latest wins |

### 1.9 Proxy Tool Installs (3 tests)

| Test | Description |
|------|-------------|
| `test_record_and_get_proxy_installs` | Record installs for 2 tools, `get_proxy_installs()` returns both tool_ids |
| `test_remove_proxy_install` | Remove one → only the other remains |
| `test_proxy_installs_cascade_on_unregister` | Unregister server → installs deleted |

### 1.10 Delete Server Records (2 tests)

| Test | Description |
|------|-------------|
| `test_delete_server_records_cascades_all` | Create installation + favorite + disabled + proxy → `delete_server_records()` removes all, returns tool/config pairs |
| `test_delete_server_records_no_records` | Deleting nonexistent server returns empty vec, no error |

### 1.11 Config Backup (1 test)

| Test | Description |
|------|-------------|
| `test_save_config_backup` | Save backup, verify no errors (read-back if possible) |

**Phase 1 total: ~40 tests**

---

## Phase 2: Database Migration Tests

**File:** `src-tauri/src/db/migrations.rs`
**Test location:** New `#[cfg(test)] mod tests` in `migrations.rs`

### 2.1 Schema Validation (4 tests)

| Test | Description |
|------|-------------|
| `test_migrations_run_on_fresh_db` | `run_migrations()` on empty in-memory DB succeeds |
| `test_migrations_idempotent` | Running `run_migrations()` twice on same DB doesn't error (CREATE TABLE IF NOT EXISTS) |
| `test_all_tables_created` | After migration, query `sqlite_master` for all 14 expected tables |
| `test_foreign_keys_enforced` | Insert into `proxy_tool_filter` with invalid `server_id` → FK error (after `PRAGMA foreign_keys=ON`) |

**Phase 2 total: 4 tests**

---

## Phase 3: Config Writer Tests

**File:** `src-tauri/src/config/writer.rs`
**Test location:** `#[cfg(test)] mod tests` in `writer.rs`
**Approach:** Use `tempfile` crate to create temporary config files. Tests manipulate temp files rather than real tool configs.

### 3.1 Name Sanitization (5 tests)

| Test | Description |
|------|-------------|
| `test_sanitize_clean_name` | `"my-server"` → `"my-server"` (unchanged) |
| `test_sanitize_dots` | `"@scope/pkg.name"` → `"_scope_pkg_name"` |
| `test_sanitize_slashes` | `"path/to/server"` → `"path_to_server"` |
| `test_needs_sanitizing_true` | `"hello.world"` returns `true` |
| `test_needs_sanitizing_false` | `"hello-world_v2"` returns `false` |

### 3.2 JSON Config Install/Uninstall (5 tests)

These tests require creating temporary JSON config files and pointing tool definitions at them. A helper function should create a `ToolDefinition`-like struct with a temp path.

| Test | Description |
|------|-------------|
| `test_install_json_stdio_server` | Install stdio server → config file contains `mcpServers.{name}` with `command` and `args` |
| `test_install_json_http_server` | Install HTTP server → config has `url` field |
| `test_install_json_preserves_existing` | Config already has server A → install B → both A and B present |
| `test_uninstall_json_server` | Install then uninstall → server key removed, file still valid JSON |
| `test_install_json_creates_file` | Config file doesn't exist → creates parent dirs + file |

### 3.3 TOML Config Install/Uninstall (3 tests)

| Test | Description |
|------|-------------|
| `test_install_toml_server` | Install to TOML (Codex) → `[mcp_servers.{name}]` section created |
| `test_uninstall_toml_server` | Uninstall → section removed |
| `test_install_toml_preserves_formatting` | Existing comments/whitespace in TOML preserved |

### 3.4 Proxy Install (3 tests)

| Test | Description |
|------|-------------|
| `test_install_proxy_server_json` | `install_proxy_server()` writes config with `brightwing-proxy --server {id}` command |
| `test_install_proxy_server_sets_args` | Verify args include `["--server", server_id]` |
| `test_proxy_binary_path_fallback` | `proxy_binary_path()` returns a path (test the fallback logic) |

### 3.5 Restore Server Entry (2 tests)

| Test | Description |
|------|-------------|
| `test_restore_json_entry` | Restore from saved config JSON → server appears in config file |
| `test_restore_toml_entry` | Restore TOML entry from backup |

### 3.6 Edge Cases (3 tests)

| Test | Description |
|------|-------------|
| `test_install_to_unknown_tool` | Install with unknown `tool_id` → returns error |
| `test_uninstall_nonexistent_server` | Uninstall server not in config → graceful (success or appropriate error) |
| `test_install_with_env_vars` | Env vars in `ServerInstallConfig` appear in config output |

**Phase 3 total: ~21 tests**

---

## Phase 4: Config Reader Tests

**File:** `src-tauri/src/config/reader.rs`
**Test location:** `#[cfg(test)] mod tests` in `reader.rs`
**Approach:** Create temp config files with known content, test parsing.

### 4.1 JSON Config Reading (4 tests)

| Test | Description |
|------|-------------|
| `test_read_json_servers_basic` | JSON with 2 servers under `mcpServers` → returns HashMap with both |
| `test_read_json_servers_empty` | JSON with empty `mcpServers: {}` → empty HashMap |
| `test_read_json_missing_key` | JSON without `mcpServers` key → empty HashMap or error |
| `test_read_json_malformed` | Invalid JSON → returns error |

### 4.2 TOML Config Reading (3 tests)

| Test | Description |
|------|-------------|
| `test_read_toml_servers_basic` | TOML with `[mcp_servers.foo]` section → returns HashMap |
| `test_read_toml_servers_empty` | No `mcp_servers` section → empty |
| `test_read_toml_malformed` | Invalid TOML → returns error |

### 4.3 CLI Server Parsing (4 tests)

| Test | Description |
|------|-------------|
| `test_extract_url_from_list_line` | Parses URL from `claude mcp list` output format |
| `test_parse_cli_get_output` | Parses `claude mcp get` text output into JSON config |
| `test_parse_cli_get_output_malformed` | Bad CLI output → error |
| `test_extract_url_missing` | Line without URL → returns None |

### 4.4 ConfiguredServer Construction (2 tests)

| Test | Description |
|------|-------------|
| `test_read_all_configured_servers_returns_struct` | Verify `ConfiguredServer` fields populated (tool_id, server_name, config_json) |
| `test_configured_server_cli_only_flag` | CLI-only tools set `is_cli_only = true` |

**Phase 4 total: ~13 tests**

---

## Phase 5: Config Backup Tests

**File:** `src-tauri/src/config/backup.rs`
**Test location:** `#[cfg(test)] mod tests` in `backup.rs`

### 5.1 Backup Operations (3 tests)

| Test | Description |
|------|-------------|
| `test_backup_creates_file` | `backup_config()` creates `.brightwing-backup` file alongside config |
| `test_backup_returns_content` | Returns file content as string |
| `test_backup_missing_config` | Config file doesn't exist → returns empty string |

**Phase 5 total: 3 tests**

---

## Phase 6: Tool Definitions & Scanner Tests

**File:** `src-tauri/src/tools/definitions.rs` and `scanner.rs`
**Test location:** `#[cfg(test)] mod tests` in each file

### 6.1 Tool Definitions (5 tests)

| Test | Description |
|------|-------------|
| `test_all_tools_have_unique_ids` | No duplicate IDs in `TOOL_DEFINITIONS` |
| `test_all_tools_have_unique_short_names` | No duplicate short names |
| `test_config_format_matches_cli_flag` | `is_cli_only=true` ↔ `config_format=Cli` consistency |
| `test_cli_tools_have_cli_command` | CLI-only tools have `cli_command.is_some()` |
| `test_config_path_returns_value_for_non_cli` | Non-CLI tools have `config_path()` that returns `Some(PathBuf)` on current platform |

### 6.2 Scanner (3 tests)

| Test | Description |
|------|-------------|
| `test_scan_returns_all_tools` | `scan_all_tools()` returns one `DetectedTool` per `TOOL_DEFINITIONS` entry |
| `test_detected_tool_fields_populated` | Each returned tool has `id`, `display_name`, `short_name` filled |
| `test_scan_sets_detected_flag` | `detected` field is `bool` (true/false depending on whether tool actually exists on system) |

**Phase 6 total: 8 tests**

---

## Phase 7: Auth Daemon Handler Tests

**File:** `src-tauri/src/bin/brightwing_authd.rs`
**Test location:** `#[cfg(test)] mod tests` at bottom of file
**Approach:** Instantiate `DaemonState` with in-memory DB, call `handle_request()` directly (no socket needed).

### 7.1 Handshake (3 tests)

| Test | Description |
|------|-------------|
| `test_handle_handshake_compatible` | Compatible version → `HandshakeOk` |
| `test_handle_handshake_incompatible` | Incompatible version → `HandshakeError` with message |
| `test_handle_ping` | `Ping` → `Pong` with uptime and version |

### 7.2 Credential Lifecycle (6 tests)

| Test | Description |
|------|-------------|
| `test_handle_store_oauth_credential` | Store OAuth token set → `Ok` response |
| `test_handle_get_credential_oauth` | Store then get → `Credentials` with OAuth token |
| `test_handle_get_credential_api_key` | Store API key then get → `Credentials` with API key env map |
| `test_handle_get_credential_not_registered` | Get for unregistered server → `CredentialError(NotFound)` |
| `test_handle_get_credential_expired_token` | Store OAuth with past `expires_at` → `CredentialError(AuthExpired)` |
| `test_handle_delete_credential` | Store then delete then get → `CredentialError` |

### 7.3 Server Registration (4 tests)

| Test | Description |
|------|-------------|
| `test_handle_register_server` | Register → `Ok`, then `ListServers` includes it |
| `test_handle_unregister_server` | Register then unregister → `ListServers` empty |
| `test_handle_list_servers_empty` | No servers registered → empty list |
| `test_handle_list_servers_multiple` | Register 3 servers → all returned |

### 7.4 Tool Operations (4 tests)

| Test | Description |
|------|-------------|
| `test_handle_list_tools` | Cache tools then `ListTools` → returns cached entries |
| `test_handle_list_tools_empty` | No cached tools → empty list |
| `test_handle_set_tool_filter` | Set filter then `GetToolFilter` → reflects change |
| `test_handle_set_tool_filter_bulk` | Bulk set then get → only specified tools enabled |

### 7.5 Proxy Logs (2 tests)

| Test | Description |
|------|-------------|
| `test_handle_get_proxy_logs_empty` | No logs → empty events list |
| `test_log_buffer_max_200` | Push >200 events → only latest 200 returned |

**Phase 7 total: ~19 tests**

---

## Phase 8: Transport Layer Tests

**File:** `src-tauri/crates/proxy-common/src/transport.rs`
**Test location:** `#[cfg(test)] mod tests` in `transport.rs`

### 8.1 Path/Address Functions (3 tests)

| Test | Description |
|------|-------------|
| `test_default_socket_path_not_empty` | `default_socket_path()` returns non-empty PathBuf |
| `test_default_data_dir_not_empty` | `default_data_dir()` returns non-empty PathBuf |
| `test_default_socket_path_platform_specific` | On macOS ends with `.sock`; on Windows starts with `\\.\pipe\` |

### 8.2 IPC Listener & Client (5 tests)

| Test | Description |
|------|-------------|
| `test_bind_and_accept` | `IpcListener::bind()` then client connects → `accept()` returns stream |
| `test_daemon_client_connect_nonexistent` | `DaemonClient::connect()` to nonexistent path → error with helpful message |
| `test_send_recv_roundtrip` | Client sends `Ping`, server reads and responds `Pong`, client reads `Pong` |
| `test_multiple_messages_one_connection` | Send 5 messages on same connection → all received correctly |
| `test_concurrent_accept` | 3 clients connect simultaneously → all accepted |

### 8.3 DaemonClient High-Level (3 tests)

| Test | Description |
|------|-------------|
| `test_handshake_method` | `DaemonClient::handshake()` completes successfully against mock server |
| `test_request_method` | `DaemonClient::request()` sends and receives in one call |
| `test_connect_default_path` | `DaemonClient::connect_default()` uses `default_socket_path()` (may fail if daemon not running — test the path, not the connection) |

**Phase 8 total: ~11 tests**

---

## Phase 9: Vault Backend Tests

**File:** `src-tauri/crates/proxy-common/src/vault.rs` and `stronghold_vault.rs`
**Test location:** Expand existing `#[cfg(test)]` modules

### 9.1 InMemoryVaultBackend (4 tests)

| Test | Description |
|------|-------------|
| `test_store_and_retrieve` | Store credential, retrieve → matches |
| `test_retrieve_nonexistent` | Get missing key → returns None or error |
| `test_delete` | Store then delete then get → gone |
| `test_overwrite` | Store, store again with new value → latest returned |

### 9.2 StrongholdVaultBackend (3 tests)

| Test | Description |
|------|-------------|
| `test_stronghold_store_and_retrieve` | Same as in-memory but with real Stronghold file (temp dir) |
| `test_stronghold_persistence` | Store, drop backend, reopen from same file → data persists |
| `test_stronghold_delete` | Store then delete → gone |

**Phase 9 total: ~7 tests**

---

## Phase 10: Proxy Binary Unit Tests

**File:** `src-tauri/src/bin/brightwing_proxy.rs`
**Test location:** Expand existing `#[cfg(test)]` module (currently ~5 tests)

### 10.1 Request Routing (4 tests)

| Test | Description |
|------|-------------|
| `test_route_initialize_request` | `initialize` method routed correctly |
| `test_route_tools_list_request` | `tools/list` routed and filter applied |
| `test_route_tools_call_request` | `tools/call` routed without filter |
| `test_route_unknown_method` | Unknown JSON-RPC method → error response |

### 10.2 Auth Header Injection (3 tests)

| Test | Description |
|------|-------------|
| `test_inject_oauth_bearer` | OAuth credential → `Authorization: Bearer {token}` header |
| `test_inject_api_key_env` | API key credential → env vars set on upstream process (or header for HTTP) |
| `test_no_auth_credential` | None credential → no auth header |

### 10.3 Tool Filter Application (3 tests)

| Test | Description |
|------|-------------|
| `test_filter_removes_disabled_tools` | Filter with 2 of 5 enabled → response has 2 tools |
| `test_filter_preserves_tool_details` | Filtered tools retain name, description, inputSchema |
| `test_no_filter_passes_all` | No filter entries → all tools pass through |

### 10.4 Error Handling (2 tests)

| Test | Description |
|------|-------------|
| `test_upstream_connection_failure` | Upstream unreachable → JSON-RPC error response |
| `test_malformed_upstream_response` | Upstream returns invalid JSON → error response |

**Phase 10 total: ~12 tests**

---

## Phase 11: Frontend Test Infrastructure Setup

**Files to create:**
- `vitest.config.ts`
- `src/test/setup.ts`
- `src/test/mocks/tauri.ts`

### 11.1 Vitest Configuration

```typescript
// vitest.config.ts
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    css: false,
  },
  resolve: {
    alias: {
      '@tauri-apps/api': './src/test/mocks/tauri.ts',
      '@tauri-apps/plugin-updater': './src/test/mocks/tauri-updater.ts',
      '@tauri-apps/plugin-shell': './src/test/mocks/tauri-shell.ts',
      '@tauri-apps/plugin-notification': './src/test/mocks/tauri-notification.ts',
    },
  },
});
```

### 11.2 Tauri Mock

```typescript
// src/test/mocks/tauri.ts
const commandHandlers: Record<string, (...args: any[]) => any> = {};

export function mockTauriCommand(command: string, handler: (...args: any[]) => any) {
  commandHandlers[command] = handler;
}

export function resetTauriMocks() {
  Object.keys(commandHandlers).forEach(k => delete commandHandlers[k]);
}

export const invoke = vi.fn(async (cmd: string, args?: any) => {
  if (commandHandlers[cmd]) return commandHandlers[cmd](args);
  throw new Error(`Unmocked Tauri command: ${cmd}`);
});

export const listen = vi.fn(() => Promise.resolve(() => {}));
```

### 11.3 Dependencies to Install

```bash
npm install -D vitest @testing-library/react @testing-library/jest-dom @testing-library/user-event jsdom
```

### 11.4 package.json Script

```json
{
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest"
  }
}
```

**Phase 11 total: 0 tests (infrastructure only)**

---

## Phase 12: Zustand Store Tests

**File:** `src/store.test.ts`
**Approach:** Test store actions directly (no component rendering needed). Mock all Tauri `invoke` calls.

### 12.1 View Navigation (3 tests)

| Test | Description |
|------|-------------|
| `test_set_view` | `setView("search")` → `store.view === "search"` |
| `test_set_view_with_install_target` | `setView("install")` after `setInstallTarget()` |
| `test_initial_view` | Default view is `"dashboard"` |

### 12.2 Data Refresh Actions (5 tests)

| Test | Description |
|------|-------------|
| `test_refresh_tools` | Mock `scan_tools` → `store.tools` populated, `loadingTools` transitions false→true→false |
| `test_refresh_configured_servers` | Mock `scan_configured_servers` → `store.configuredServers` populated |
| `test_refresh_installations` | Mock `get_installations` → `store.installations` populated |
| `test_refresh_favorites` | Mock `get_favorites` → `store.favorites` populated |
| `test_refresh_proxy_servers` | Mock `get_proxy_servers` → `store.proxyServers` populated |

### 12.3 Favorites (3 tests)

| Test | Description |
|------|-------------|
| `test_toggle_favorite_add` | Server not favorited → `toggleFavorite()` calls `add_favorite` |
| `test_toggle_favorite_remove` | Server already favorited → `toggleFavorite()` calls `remove_favorite` |
| `test_toggle_favorite_refreshes` | After toggle, `refreshFavorites()` called |

### 12.4 Server Enable/Disable (4 tests)

| Test | Description |
|------|-------------|
| `test_disable_server` | `disableServer()` calls backend, adds to `pendingRestarts` |
| `test_enable_server` | `enableServer()` calls backend, adds to `pendingRestarts` |
| `test_pending_restarts_accumulate` | Disable 2 servers on same tool → 1 pending restart (deduplicated by tool) |
| `test_restart_tool_clears_pending` | `restartTool()` removes tool from `pendingRestarts` |

### 12.5 Tool Filter (3 tests)

| Test | Description |
|------|-------------|
| `test_toggle_tool_filter_optimistic` | `toggleToolFilter()` updates `activeFilter` immediately before backend responds |
| `test_toggle_tool_filter_rollback` | Backend error → `activeFilter` reverts to previous state |
| `test_active_filter_cleared_on_server_change` | Switching `serverDetailId` clears `activeFilter` |

### 12.6 Update Flow (3 tests)

| Test | Description |
|------|-------------|
| `test_check_for_update_available` | Mock updater → `updateAvailable` set to version string |
| `test_check_for_update_none` | No update → `updateAvailable` stays null |
| `test_install_update_sets_downloading` | `installUpdate()` sets `updateDownloading = true` |

### 12.7 Deep Link (2 tests)

| Test | Description |
|------|-------------|
| `test_check_pending_deep_link` | Mock returns deep link → `pendingDeepLink` set, navigates to install |
| `test_check_pending_deep_link_none` | Mock returns null → no state change |

### 12.8 Toast (2 tests)

| Test | Description |
|------|-------------|
| `test_toast_set` | `setToast("message")` → `store.toast === "message"` |
| `test_toast_auto_clear` | Toast auto-clears after 4 seconds (use `vi.useFakeTimers()`) |

**Phase 12 total: ~25 tests**

---

## Phase 13: Tauri Binding Tests

**File:** `src/lib/tauri.test.ts`
**Approach:** Verify each wrapper calls `invoke` with the correct command name and argument shape.

### 13.1 Tool Scanning Commands (3 tests)

| Test | Description |
|------|-------------|
| `test_scan_tools_invokes_correct_command` | `scanTools()` → `invoke("scan_tools")` |
| `test_scan_configured_servers` | `scanConfiguredServers()` → `invoke("scan_configured_servers")` |
| `test_read_tool_config` | `readToolConfig("claude_desktop")` → `invoke("read_tool_config", { toolId: "claude_desktop" })` |

### 13.2 Server Management Commands (5 tests)

| Test | Description |
|------|-------------|
| `test_install_server` | Verify `invoke` args include `toolId`, `config` with all fields |
| `test_uninstall_server` | Verify `invoke` args: `toolId`, `configKey` |
| `test_disable_server` | Verify `invoke` args: `toolId`, `serverName` |
| `test_enable_server` | Verify `invoke` args: `toolId`, `serverName` |
| `test_delete_server` | Verify `invoke` args: `serverName` |

### 13.3 Proxy Commands (4 tests)

| Test | Description |
|------|-------------|
| `test_register_proxy_server` | All fields passed (serverId, displayName, authType, upstreamUrl, etc.) |
| `test_install_proxy_to_tool` | `invoke("install_proxy_to_tool", { toolId, serverId, configKey })` |
| `test_get_proxy_servers` | No args passed |
| `test_get_proxy_logs` | `invoke("get_proxy_logs", { serverId })` |

### 13.4 OAuth Commands (4 tests)

| Test | Description |
|------|-------------|
| `test_start_oauth_flow` | Args: `serverId`, `serverUrl`, optional `clientId` |
| `test_complete_oauth_callback` | Args: `state` |
| `test_get_oauth_status` | Args: `serverId` |
| `test_disconnect_oauth` | Args: `serverId` |

### 13.5 Error Propagation (2 tests)

| Test | Description |
|------|-------------|
| `test_invoke_error_propagates` | Backend throws → wrapper rejects with same error |
| `test_invoke_returns_typed_data` | Backend returns typed data → wrapper resolves with correct type |

**Phase 13 total: ~18 tests**

---

## Phase 14: React Component Tests

**Approach:** Use React Testing Library. Test rendering, user interactions, and Tauri command calls. Mock all Tauri commands.

### 14.1 Navigation Component (4 tests)

**File:** `src/components/Navigation.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_all_nav_items` | All nav items (Dashboard, Search, Add, Proxy, etc.) visible |
| `test_click_nav_item_changes_view` | Click "Search" → `store.view` changes to `"search"` |
| `test_active_item_highlighted` | Current view's nav item has active styling |
| `test_update_button_shows_when_available` | Set `updateAvailable` → "Update to vX.Y.Z" button visible |

### 14.2 Search Component (5 tests)

**File:** `src/components/Search.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_search_input` | Search input field visible |
| `test_search_debounces_400ms` | Type "test" → API called after 400ms, not immediately |
| `test_search_results_displayed` | Mock API returns 3 servers → 3 rows rendered |
| `test_grade_color_coding` | Grade "A" → green badge; "F" → red badge |
| `test_install_button_navigates` | Click install → `setView("install")` with target set |

### 14.3 Dashboard Component (7 tests)

**File:** `src/components/Dashboard.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_tool_columns` | Detected tools appear as column headers |
| `test_renders_server_rows` | Configured servers appear as rows |
| `test_cell_toggle_add` | Toggle empty cell → pending change "add" created |
| `test_cell_toggle_disable` | Toggle configured cell → pending change "disable" created |
| `test_save_button_disabled_when_no_changes` | No pending changes → save button disabled |
| `test_save_executes_changes` | Save with pending add → calls `installServer()` |
| `test_favorites_filter` | Click "Favorites only" → only favorited servers shown |

### 14.4 InstallDialog Component (6 tests)

**File:** `src/components/InstallDialog.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_server_info` | Server name, description, grade displayed |
| `test_tool_checkboxes` | Detected tools shown as checkboxes |
| `test_env_var_inputs_rendered` | Required env vars from schema → input fields rendered |
| `test_required_env_var_validation` | Leave required env var empty → save blocked |
| `test_http_server_auto_probes` | HTTP server URL → `probeServerAuth()` called |
| `test_save_registers_proxy_and_installs` | Save → calls register + install for each selected tool |

### 14.5 OAuthConnect Component (5 tests)

**File:** `src/components/OAuthConnect.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_disconnected_state` | Status "disconnected" → "Connect" button visible |
| `test_renders_connected_state` | Status "connected" → "Refresh" and "Disconnect" buttons visible |
| `test_renders_expired_state` | Status "expired" → warning + "Refresh Token" button |
| `test_connect_opens_auth_url` | Click Connect → `startOAuthFlow()` called |
| `test_disconnect_clears_status` | Click Disconnect → `disconnectOAuth()` called, status reset |

### 14.6 AddServer Component (5 tests)

**File:** `src/components/AddServer.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_form_fields` | Config key, command, args, transport toggle visible |
| `test_transport_toggle_shows_url` | Select "HTTP" → URL input appears, command/args hidden |
| `test_http_probe_on_url_entry` | Enter URL → `probeServerAuth()` called |
| `test_save_registers_server` | Fill form, save → `registerProxyServer()` called with correct args |
| `test_env_var_rows` | Add env var row → additional key/value inputs appear |

### 14.7 ServerDetail Component (5 tests)

**File:** `src/components/ServerDetail.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_server_info` | Server ID, auth type, upstream URL displayed |
| `test_shows_installed_tools` | Proxy installed in 2 tools → both shown |
| `test_delete_confirmation` | Click delete → confirmation dialog appears |
| `test_discover_tools_button` | Click discover → `discoverUpstreamTools()` called |
| `test_sub_view_navigation` | Click "Filter" → ToolFilterPanel shown; click "Logs" → ProxyLogViewer shown |

### 14.8 ToolFilterPanel Component (5 tests)

**File:** `src/components/ToolFilterPanel.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_tool_list` | 5 cached tools → 5 checkboxes rendered |
| `test_toggle_tool` | Uncheck tool → `setToolFilter()` called with `enabled: false` |
| `test_enable_all` | Click "Enable All" → all tools checked |
| `test_disable_all` | Click "Disable All" → all tools unchecked |
| `test_token_budget_bar` | Shows token count and percentage bar |

### 14.9 ProxyLogViewer Component (4 tests)

**File:** `src/components/ProxyLogViewer.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_log_entries` | 3 events → 3 rows displayed with timestamps |
| `test_event_type_badges` | CONN event → green badge; ERR → red badge |
| `test_auto_scroll_on_new_events` | New events → scrolls to bottom |
| `test_client_abbreviations` | `claude-code` → `CC`; `cursor` → `CU` |

### 14.10 MigrationAssistant Component (4 tests)

**File:** `src/components/MigrationAssistant.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_when_migratable_servers_exist` | Servers with env vars → assistant visible |
| `test_hidden_when_no_migratable` | No plaintext secrets → assistant hidden |
| `test_dismiss_hides` | Click dismiss → hidden |
| `test_migrate_calls_backend` | Click migrate → register + store credentials + uninstall + install proxy |

### 14.11 ProxyServers Component (3 tests)

**File:** `src/components/ProxyServers.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_server_list` | 3 proxy servers → 3 cards rendered |
| `test_auth_type_badge` | OAuth server → "oauth" badge; API key → "api_key" badge |
| `test_click_navigates_to_detail` | Click server card → `setView("server-detail")` |

### 14.12 App Component (3 tests)

**File:** `src/App.test.tsx`

| Test | Description |
|------|-------------|
| `test_renders_navigation` | Navigation sidebar present |
| `test_renders_dashboard_by_default` | Default view → Dashboard component rendered |
| `test_view_switching` | Set view to "search" → Search component rendered |

**Phase 14 total: ~56 tests**

---

## Phase 15: Frontend Utility & Edge Case Tests

**File:** `src/lib/utils.test.ts` (if utility functions extracted) or inline in component tests

### 15.1 Server Name Normalization (3 tests)

The frontend mirrors the Rust `sanitize_server_name()` in Dashboard.tsx. Extract and test separately.

| Test | Description |
|------|-------------|
| `test_normalize_dots_to_underscores` | `"@scope/pkg.name"` → `"_scope_pkg_name"` |
| `test_normalize_preserves_valid` | `"my-server_v2"` → unchanged |
| `test_normalize_matches_rust` | Same inputs produce same outputs as Rust `sanitize_server_name()` |

### 15.2 Grade Color Mapping (2 tests)

| Test | Description |
|------|-------------|
| `test_grade_colors` | A→green, B→blue, C→yellow, D→orange, F→red |
| `test_unknown_grade` | Unknown grade → default/gray color |

**Phase 15 total: ~5 tests**

---

## Summary

| Phase | Area | New Tests | Cumulative |
|-------|------|-----------|------------|
| 1 | Database queries | 40 | 40 |
| 2 | Database migrations | 4 | 44 |
| 3 | Config writer | 21 | 65 |
| 4 | Config reader | 13 | 78 |
| 5 | Config backup | 3 | 81 |
| 6 | Tool definitions & scanner | 8 | 89 |
| 7 | Auth daemon handlers | 19 | 108 |
| 8 | Transport layer | 11 | 119 |
| 9 | Vault backends | 7 | 126 |
| 10 | Proxy binary unit tests | 12 | 138 |
| 11 | Frontend infra setup | 0 | 138 |
| 12 | Zustand store | 25 | 163 |
| 13 | Tauri bindings | 18 | 181 |
| 14 | React components | 56 | 237 |
| 15 | Frontend utilities | 5 | 242 |
| **Total** | | **242** | **~370 total with existing 130** |

## Implementation Notes

### Rust Testing Conventions (match existing patterns)

- All async tests use `#[tokio::test]`
- Unit tests go in `#[cfg(test)] mod tests { ... }` at bottom of each source file
- Use `Database::new_in_memory()` for all DB tests (no file cleanup)
- Use `tempfile` crate for config file tests (add to `[dev-dependencies]`)
- Use builder pattern for mock construction (consistent with `MockMcpServer::builder()`)
- Assertion style: `match` + `panic!("Expected X, got: {:?}", other)` for enum responses; `assert_eq!` for values
- Test naming: `test_<subject>_<scenario>` (e.g., `test_install_json_stdio_server`)
- Integration tests live in `src-tauri/tests/`; unit tests live in source files

### Frontend Testing Conventions (new, to establish)

- Vitest + React Testing Library + jsdom
- Mock all Tauri `invoke` calls via alias in vitest config
- Test files colocated: `Component.test.tsx` next to `Component.tsx`
- Store tests use `useStore.getState()` and `useStore.setState()` directly
- Use `vi.useFakeTimers()` for debounce/timer tests
- Use `@testing-library/user-event` for user interactions
- Prefer `getByRole` and `getByText` over `getByTestId`

### Dependencies to Add

**Rust (`src-tauri/Cargo.toml` `[dev-dependencies]`):**
```toml
tempfile = "3"
```

**Frontend (`package.json` `devDependencies`):**
```json
{
  "vitest": "^3.x",
  "@testing-library/react": "^16.x",
  "@testing-library/jest-dom": "^6.x",
  "@testing-library/user-event": "^14.x",
  "jsdom": "^26.x"
}
```
