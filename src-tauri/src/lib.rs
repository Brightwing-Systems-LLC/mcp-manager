mod config;
pub mod db;
mod deeplink;
pub mod oauth;
pub mod proxy;
mod tools;

use config::reader::ConfiguredServer;
use config::writer::{InstallResult, ServerInstallConfig};
use db::queries::{DisabledServer, Favorite, Installation, ProxyServer, ProxyApiKey, ToolFilterEntry, CachedTool};
use db::Database;
use deeplink::{DeepLinkAction, DeepLinkState};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{Emitter, Listener, Manager};
use tools::definitions::DetectedTool;

// --- Installable IDs Cache (5-minute TTL) ---

static INSTALLABLE_IDS_CACHE: Mutex<Option<(Instant, Vec<String>)>> = Mutex::new(None);
const INSTALLABLE_IDS_TTL_SECS: u64 = 300; // 5 minutes

// --- Auth Probe Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProbeResult {
    pub auth_type: String,         // "oauth" | "api_key" | "none" | "unknown"
    pub server_reachable: bool,
    pub error_message: Option<String>,
    pub has_oauth_metadata: bool,
}

// --- Tauri Commands ---

#[tauri::command]
fn scan_tools() -> Result<Vec<DetectedTool>, String> {
    Ok(tools::scanner::scan_all_tools())
}

#[tauri::command]
async fn scan_configured_servers() -> Result<Vec<ConfiguredServer>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(config::reader::read_all_configured_servers());
    });
    rx.recv().map_err(|e| format!("Scan failed: {}", e))
}

#[tauri::command]
fn read_tool_config(tool_id: String) -> Result<HashMap<String, JsonValue>, String> {
    config::reader::read_installed_servers(&tool_id)
}

#[tauri::command]
fn install_server(
    tool_id: String,
    server_config: ServerInstallConfig,
    db: tauri::State<'_, Database>,
) -> Result<InstallResult, String> {
    // Backup first
    let _ = config::backup::backup_config(&tool_id);

    let result = config::writer::install_server(&tool_id, &server_config)?;

    if result.success {
        let snapshot = serde_json::to_string(&server_config).ok();
        db.record_installation(
            &server_config.server_name,
            &server_config.server_name,
            &tool_id,
            &server_config.config_key,
            snapshot.as_deref(),
        )?;
    }

    Ok(result)
}

#[tauri::command]
fn uninstall_server(
    tool_id: String,
    config_key: String,
    server_uuid: String,
    db: tauri::State<'_, Database>,
) -> Result<InstallResult, String> {
    let result = config::writer::uninstall_server(&tool_id, &config_key)?;

    if result.success {
        db.remove_installation(&server_uuid, &tool_id)?;
    }

    Ok(result)
}

#[tauri::command]
fn get_installations(db: tauri::State<'_, Database>) -> Result<Vec<Installation>, String> {
    db.get_installations()
}

#[tauri::command]
fn get_favorites(db: tauri::State<'_, Database>) -> Result<Vec<Favorite>, String> {
    db.get_favorites()
}

#[tauri::command]
fn add_favorite(
    server_uuid: String,
    server_name: String,
    display_name: Option<String>,
    grade: Option<String>,
    score: Option<i64>,
    language: Option<String>,
    install_config_json: Option<String>,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    db.add_favorite(
        &server_uuid,
        &server_name,
        display_name.as_deref(),
        grade.as_deref(),
        score,
        language.as_deref(),
        install_config_json.as_deref(),
    )
}

#[tauri::command]
fn remove_favorite(
    server_uuid: String,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    db.remove_favorite(&server_uuid)
}

#[tauri::command]
fn get_pending_deep_link(
    state: tauri::State<'_, DeepLinkState>,
) -> Result<Option<DeepLinkAction>, String> {
    let pending = state.pending.lock().map_err(|e| e.to_string())?;
    Ok(pending.clone())
}

#[tauri::command]
fn clear_pending_deep_link(
    state: tauri::State<'_, DeepLinkState>,
) -> Result<(), String> {
    let mut pending = state.pending.lock().map_err(|e| e.to_string())?;
    *pending = None;
    Ok(())
}

#[tauri::command]
fn disable_server(
    tool_id: String,
    server_name: String,
    config_json: String,
    db: tauri::State<'_, Database>,
) -> Result<InstallResult, String> {
    // Backup first
    let _ = config::backup::backup_config(&tool_id);

    // Store the config snapshot in DB
    db.disable_server(&tool_id, &server_name, &config_json)?;

    // Remove from config file
    config::writer::uninstall_server(&tool_id, &server_name)
}

#[tauri::command]
fn enable_server(
    tool_id: String,
    server_name: String,
    db: tauri::State<'_, Database>,
) -> Result<InstallResult, String> {
    // Get stored config from DB
    let config_json = db.enable_server(&tool_id, &server_name)?;

    // Restore to config file
    config::writer::restore_server_entry(&tool_id, &server_name, &config_json)
}

#[tauri::command]
fn add_server_to_tool(
    tool_id: String,
    server_name: String,
    config_json: String,
) -> Result<InstallResult, String> {
    let _ = config::backup::backup_config(&tool_id);
    // Sanitize server name for tools that require clean identifiers
    let safe_name = if config::writer::needs_sanitizing(&server_name) {
        config::writer::sanitize_server_name(&server_name)
    } else {
        server_name
    };
    config::writer::restore_server_entry(&tool_id, &safe_name, &config_json)
}

#[tauri::command]
fn get_disabled_servers(
    db: tauri::State<'_, Database>,
) -> Result<Vec<DisabledServer>, String> {
    db.get_disabled_servers()
}

/// Completely delete a server: uninstall from all app configs, remove all DB records.
/// Accepts multiple name variants (the same server may have different names across tools).
/// Returns the list of tool IDs where the server was removed.
#[tauri::command]
fn delete_server(
    server_names: Vec<String>,
    db: tauri::State<'_, Database>,
) -> Result<Vec<String>, String> {
    let mut removed_from = Vec::new();

    // 1. Delete DB records for all name variants
    for name in &server_names {
        if let Ok(installs) = db.delete_server_records(name) {
            // 2. Uninstall from each tool's config file
            for (tool_id, config_key) in &installs {
                if removed_from.contains(tool_id) {
                    continue;
                }
                let _ = config::backup::backup_config(tool_id);
                match config::writer::uninstall_server(tool_id, config_key) {
                    Ok(result) if result.success => {
                        removed_from.push(tool_id.clone());
                    }
                    _ => {
                        // Also try with server_name as config_key
                        if let Ok(result) = config::writer::uninstall_server(tool_id, name) {
                            if result.success {
                                removed_from.push(tool_id.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Scan live configs as a safety net — catch anything not in the DB
    let configured = config::reader::read_all_configured_servers();
    for cs in &configured {
        if removed_from.contains(&cs.tool_id) {
            continue;
        }
        if server_names.iter().any(|n| n == &cs.server_name) {
            let _ = config::backup::backup_config(&cs.tool_id);
            if let Ok(result) = config::writer::uninstall_server(&cs.tool_id, &cs.server_name) {
                if result.success {
                    removed_from.push(cs.tool_id.clone());
                }
            }
        }
    }

    Ok(removed_from)
}

#[tauri::command]
fn backup_tool_config(tool_id: String) -> Result<String, String> {
    config::backup::backup_config(&tool_id)
}

#[tauri::command]
async fn fetch_cli_server_config(tool_id: String, server_name: String) -> Result<String, String> {
    let def = tools::definitions::TOOL_DEFINITIONS
        .iter()
        .find(|d| d.id == tool_id)
        .ok_or_else(|| format!("Unknown tool: {}", tool_id))?;

    let cmd = def
        .cli_command
        .ok_or_else(|| format!("{} is not a CLI tool", tool_id))?;

    let (tx, rx) = std::sync::mpsc::channel();
    let cmd_str = cmd.to_string();
    let name = server_name.clone();
    std::thread::spawn(move || {
        let _ = tx.send(config::reader::fetch_cli_server_config(&cmd_str, &name));
    });
    rx.recv().map_err(|e| format!("Fetch failed: {}", e))?
}

#[tauri::command]
async fn restart_tool(tool_id: String) -> Result<String, String> {
    // Returns a message about what happened
    match tool_id.as_str() {
        "claude_desktop" => {
            #[cfg(target_os = "macos")]
            {
                // Kill Claude Desktop gracefully first, then force if needed
                let _ = std::process::Command::new("osascript")
                    .args(["-e", r#"tell application "Claude" to quit"#])
                    .output();
                // Give it a moment to quit gracefully
                std::thread::sleep(std::time::Duration::from_secs(2));
                // Force kill if still running
                let _ = std::process::Command::new("pkill")
                    .args(["-f", "Claude.app"])
                    .output();
                std::thread::sleep(std::time::Duration::from_millis(500));
                // Relaunch
                let output = std::process::Command::new("open")
                    .args(["-a", "Claude"])
                    .output()
                    .map_err(|e| format!("Failed to relaunch Claude Desktop: {}", e))?;
                if output.status.success() {
                    Ok("Claude Desktop restarted".to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("Failed to relaunch Claude Desktop: {}", stderr))
                }
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/IM", "Claude.exe", "/F"])
                    .output();
                std::thread::sleep(std::time::Duration::from_secs(1));
                // Try to find and relaunch Claude on Windows
                if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                    let claude_path = std::path::PathBuf::from(local_app_data)
                        .join("Programs")
                        .join("Claude")
                        .join("Claude.exe");
                    if claude_path.exists() {
                        let _ = std::process::Command::new(claude_path)
                            .spawn()
                            .map_err(|e| format!("Failed to relaunch Claude Desktop: {}", e))?;
                        return Ok("Claude Desktop restarted".to_string());
                    }
                }
                Err("Could not find Claude Desktop to relaunch".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("pkill")
                    .args(["-f", "claude"])
                    .output();
                Err("Please relaunch Claude Desktop manually on Linux".to_string())
            }
        }
        "cursor" => {
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", r#"tell application "Cursor" to quit"#])
                    .output();
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = std::process::Command::new("open")
                    .args(["-a", "Cursor"])
                    .output()
                    .map_err(|e| format!("Failed to relaunch Cursor: {}", e))?;
                Ok("Cursor restarted".to_string())
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("Automatic restart not supported for Cursor on this platform".to_string())
            }
        }
        "windsurf" => {
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", r#"tell application "Windsurf" to quit"#])
                    .output();
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = std::process::Command::new("open")
                    .args(["-a", "Windsurf"])
                    .output()
                    .map_err(|e| format!("Failed to relaunch Windsurf: {}", e))?;
                Ok("Windsurf restarted".to_string())
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("Automatic restart not supported for Windsurf on this platform".to_string())
            }
        }
        "vscode" => {
            // VS Code typically auto-reloads settings, but we can offer a reload
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", r#"tell application "Visual Studio Code" to quit"#])
                    .output();
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = std::process::Command::new("open")
                    .args(["-a", "Visual Studio Code"])
                    .output()
                    .map_err(|e| format!("Failed to relaunch VS Code: {}", e))?;
                Ok("VS Code restarted".to_string())
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("Automatic restart not supported for VS Code on this platform".to_string())
            }
        }
        "codex" => {
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", r#"tell application "Codex" to quit"#])
                    .output();
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = std::process::Command::new("pkill")
                    .args(["-f", "Codex.app"])
                    .output();
                std::thread::sleep(std::time::Duration::from_millis(500));
                let output = std::process::Command::new("open")
                    .args(["-a", "Codex"])
                    .output()
                    .map_err(|e| format!("Failed to relaunch Codex: {}", e))?;
                if output.status.success() {
                    Ok("OpenAI Codex restarted".to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("Failed to relaunch Codex: {}", stderr))
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("Automatic restart not supported for Codex on this platform".to_string())
            }
        }
        // CLI tools don't need restart
        "claude_code" | "gemini_cli" => {
            Ok("No restart needed for CLI tools".to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool_id)),
    }
}

// --- Auth Probe ---

#[tauri::command]
async fn probe_server_auth(url: String) -> Result<AuthProbeResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Try MCP initialize with no auth
    let init_payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "brightwing-probe",
                "version": "1.0.0"
            }
        }
    });

    let resp = match client.post(&url).json(&init_payload).send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(AuthProbeResult {
                auth_type: "unknown".to_string(),
                server_reachable: false,
                error_message: Some(format!("Connection failed: {}", e)),
                has_oauth_metadata: false,
            });
        }
    };

    let status = resp.status().as_u16();

    if status == 200 || (200..300).contains(&status) {
        return Ok(AuthProbeResult {
            auth_type: "none".to_string(),
            server_reachable: true,
            error_message: None,
            has_oauth_metadata: false,
        });
    }

    if status == 401 || status == 403 {
        // Check for OAuth metadata
        match oauth::discovery::discover_oauth_metadata(&url).await {
            Ok(_) => {
                return Ok(AuthProbeResult {
                    auth_type: "oauth".to_string(),
                    server_reachable: true,
                    error_message: None,
                    has_oauth_metadata: true,
                });
            }
            Err(_) => {
                return Ok(AuthProbeResult {
                    auth_type: "api_key".to_string(),
                    server_reachable: true,
                    error_message: None,
                    has_oauth_metadata: false,
                });
            }
        }
    }

    // Other status codes — server is reachable but unexpected response
    Ok(AuthProbeResult {
        auth_type: "unknown".to_string(),
        server_reachable: true,
        error_message: Some(format!("Unexpected HTTP status: {}", status)),
        has_oauth_metadata: false,
    })
}

// --- Proxy Server Management ---

#[tauri::command]
fn register_proxy_server(
    server_id: String,
    display_name: String,
    auth_type: String,
    upstream_url: Option<String>,
    upstream_command: Option<String>,
    upstream_args: Option<String>,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    db.register_proxy_server(
        &server_id,
        &display_name,
        &auth_type,
        upstream_url.as_deref(),
        upstream_command.as_deref(),
        upstream_args.as_deref(),
    )
}

#[tauri::command]
fn unregister_proxy_server(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    db.unregister_proxy_server(&server_id)
}

#[tauri::command]
fn get_proxy_servers(
    db: tauri::State<'_, Database>,
) -> Result<Vec<ProxyServer>, String> {
    db.get_proxy_servers()
}

#[tauri::command]
fn get_proxy_server(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<Option<ProxyServer>, String> {
    db.get_proxy_server(&server_id)
}

#[tauri::command]
fn install_proxy_to_tool(
    tool_id: String,
    server_id: String,
    config_key: String,
    db: tauri::State<'_, Database>,
) -> Result<InstallResult, String> {
    let _ = config::backup::backup_config(&tool_id);
    let result = config::writer::install_proxy_server(&tool_id, &server_id, &config_key)?;
    if result.success {
        db.record_proxy_install(&server_id, &tool_id)?;
    }
    Ok(result)
}

#[tauri::command]
fn uninstall_proxy_from_tool(
    tool_id: String,
    server_id: String,
    config_key: String,
    db: tauri::State<'_, Database>,
) -> Result<InstallResult, String> {
    let result = config::writer::uninstall_server(&tool_id, &config_key)?;
    if result.success {
        db.remove_proxy_install(&server_id, &tool_id)?;
    }
    Ok(result)
}

#[tauri::command]
fn get_proxy_installs(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<Vec<String>, String> {
    db.get_proxy_installs(&server_id)
}

#[tauri::command]
async fn get_tool_filter(
    server_id: String,
    tool_id: Option<String>,
    db: tauri::State<'_, Database>,
) -> Result<Vec<ToolFilterEntry>, String> {
    let tid = tool_id.as_deref().unwrap_or("_all");
    db.get_tool_filter(&server_id, tid)
}

#[tauri::command]
async fn set_tool_filter(
    server_id: String,
    tool_id: Option<String>,
    tool_name: String,
    enabled: bool,
    token_estimate: u32,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    let tid = tool_id.as_deref().unwrap_or("_all");
    // If setting per-app filter for the first time, initialize from global
    if tid != "_all" {
        let _ = db.init_tool_filter_for_app(&server_id, tid);
    }
    db.set_tool_filter(&server_id, tid, &tool_name, enabled, token_estimate)
}

#[tauri::command]
async fn set_tool_filter_bulk(
    server_id: String,
    tool_id: Option<String>,
    enabled_tools: Vec<String>,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    let tid = tool_id.as_deref().unwrap_or("_all");
    if tid != "_all" {
        let _ = db.init_tool_filter_for_app(&server_id, tid);
    }
    db.set_tool_filter_bulk(&server_id, tid, &enabled_tools)
}

#[tauri::command]
fn get_cached_tools(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<Vec<CachedTool>, String> {
    db.get_cached_tools(&server_id)
}

#[tauri::command]
fn cache_tool_schema(
    server_id: String,
    tool_name: String,
    description: String,
    input_schema: String,
    token_estimate: u32,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    db.cache_tool_schema(&server_id, &tool_name, &description, &input_schema, token_estimate)
}

// --- Tool Discovery ---

#[tauri::command]
async fn discover_upstream_tools(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<Vec<CachedTool>, String> {
    // 1. Look up server
    let server = db
        .get_proxy_server(&server_id)?
        .ok_or_else(|| format!("Server '{}' not found", server_id))?;

    let upstream_url = server
        .upstream_url
        .as_ref()
        .ok_or("Server has no upstream URL configured")?;

    // 2. Build auth header from credentials
    let auth_header = match server.auth_type.as_str() {
        "oauth" => {
            if let Ok(Some(token_json)) = db.get_oauth_token_set(&server_id) {
                if let Ok(ts) = serde_json::from_str::<oauth::types::OAuthTokenSet>(&token_json) {
                    Some(format!("Bearer {}", ts.access_token))
                } else {
                    None
                }
            } else {
                None
            }
        }
        "api_key" => {
            if let Ok(Some(key)) = db.get_api_key(&server_id) {
                key.env.values().next().map(|v| format!("Bearer {}", v))
            } else {
                None
            }
        }
        _ => None,
    };

    // 3. Discover tools from upstream
    let tools = proxy::discovery::discover_tools(upstream_url, auth_header.as_deref()).await?;

    // 4. Cache each tool in DB (also creates tool_filter entries)
    for tool in &tools {
        let schema_str = serde_json::to_string(&tool.input_schema).unwrap_or_default();
        db.cache_tool_schema(
            &server_id,
            &tool.name,
            &tool.description,
            &schema_str,
            tool.token_estimate,
        )?;
    }

    // 5. Return cached tools
    db.get_cached_tools(&server_id)
}

// --- OAuth Flow ---

#[tauri::command]
async fn start_oauth_flow(
    server_id: String,
    server_url: String,
    client_id: Option<String>,
    db: tauri::State<'_, Database>,
    flow_states: tauri::State<'_, oauth::flow::OAuthFlowStates>,
) -> Result<oauth::types::OAuthFlowInfo, String> {
    oauth::flow::start_flow(
        &server_id,
        &server_url,
        client_id.as_deref(),
        &db,
        &flow_states,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn complete_oauth_callback(
    state: String,
    code: Option<String>,
    flow_states: tauri::State<'_, oauth::flow::OAuthFlowStates>,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    oauth::flow::complete_flow(&state, code.as_deref(), &flow_states, &db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn discover_oauth_server(
    server_url: String,
) -> Result<oauth::types::OAuthServerMetadata, String> {
    oauth::discovery::discover_oauth_metadata(&server_url)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_oauth_status(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<oauth::types::OAuthStatus, String> {
    Ok(oauth::flow::get_status(&server_id, &db))
}

#[tauri::command]
fn disconnect_oauth(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    oauth::flow::disconnect(&server_id, &db).map_err(|e| e.to_string())
}

#[tauri::command]
async fn refresh_oauth_token(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    let token_json = db
        .get_oauth_token_set(&server_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No OAuth tokens for {}", server_id))?;

    let token_set: oauth::types::OAuthTokenSet =
        serde_json::from_str(&token_json).map_err(|e| format!("Bad token data: {}", e))?;

    let new_set = oauth::refresh::refresh_token(&token_set)
        .await
        .map_err(|e| e.to_string())?;

    let new_json = serde_json::to_string(&new_set).map_err(|e| e.to_string())?;
    db.store_oauth_token_set(&server_id, &new_json)
        .map_err(|e| e.to_string())?;

    Ok(())
}

// --- API Key Management ---

#[tauri::command]
fn store_api_key(
    server_id: String,
    env: HashMap<String, String>,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    db.store_api_key(&server_id, &env)
}

#[tauri::command]
fn get_api_key(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<Option<ProxyApiKey>, String> {
    db.get_api_key(&server_id)
}

#[tauri::command]
fn delete_api_key(
    server_id: String,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    db.delete_api_key(&server_id)
}

#[tauri::command]
fn get_all_api_keys(
    db: tauri::State<'_, Database>,
) -> Result<Vec<ProxyApiKey>, String> {
    db.get_all_api_keys()
}

// --- Daemon Lifecycle ---

/// Status info returned by daemon_status.
#[derive(serde::Serialize, Clone)]
struct DaemonStatusInfo {
    running: bool,
    pid: Option<u32>,
    uptime_secs: Option<u64>,
    daemon_version: Option<String>,
}

/// Get the data directory used by the daemon.
fn daemon_data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("Library/Application Support/com.brightwing.mcp-manager")
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\ProgramData"))
            .join("Brightwing")
    }
}

fn daemon_socket_path() -> std::path::PathBuf {
    proxy_common::transport::default_socket_path()
}

fn daemon_pid_path() -> std::path::PathBuf {
    daemon_data_dir().join("authd.pid")
}

/// Try to ping the daemon via IPC and get uptime info.
async fn ping_daemon() -> Option<(u64, String)> {
    use proxy_common::ipc::{IpcRequest, IpcResponse};
    use proxy_common::transport::DaemonClient;

    let socket = daemon_socket_path();
    let mut client = DaemonClient::connect(&socket).await.ok()?;

    client.send(&IpcRequest::Ping).await.ok()?;

    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        client.recv(),
    ).await.ok()?.ok()?;

    match resp {
        IpcResponse::Pong { uptime_secs, daemon_version } => Some((uptime_secs, daemon_version)),
        _ => None,
    }
}

/// Fetch recent proxy log events for a server from the daemon.
#[tauri::command]
async fn get_proxy_logs(server_id: String) -> Result<Vec<proxy_common::ipc::ProxyLogEvent>, String> {
    use proxy_common::ipc::{IpcRequest, IpcResponse};
    use proxy_common::transport::DaemonClient;

    let socket = daemon_socket_path();
    let mut client = DaemonClient::connect(&socket).await
        .map_err(|e| format!("Cannot connect to daemon: {}", e))?;

    client.send(&IpcRequest::GetProxyLogs { server_id }).await
        .map_err(|e| format!("Send failed: {}", e))?;

    let resp: IpcResponse = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        client.recv(),
    ).await
        .map_err(|_| "Timeout waiting for daemon".to_string())?
        .map_err(|e| format!("Receive failed: {}", e))?;

    match resp {
        IpcResponse::ProxyLogs { events, .. } => Ok(events),
        IpcResponse::Error { message, .. } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Read PID from PID file if process is alive.
fn read_daemon_pid() -> Option<u32> {
    let pid_path = daemon_pid_path();
    let contents = std::fs::read_to_string(&pid_path).ok()?;
    let pid = contents.trim().parse::<u32>().ok()?;
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result == 0 { Some(pid) } else { None }
    }
    #[cfg(not(unix))]
    {
        Some(pid)
    }
}

#[tauri::command]
async fn daemon_status() -> Result<DaemonStatusInfo, String> {
    // First try PID file
    let pid = read_daemon_pid();
    if pid.is_none() {
        return Ok(DaemonStatusInfo {
            running: false,
            pid: None,
            uptime_secs: None,
            daemon_version: None,
        });
    }

    // Try to ping for uptime
    match ping_daemon().await {
        Some((uptime, version)) => Ok(DaemonStatusInfo {
            running: true,
            pid,
            uptime_secs: Some(uptime),
            daemon_version: Some(version),
        }),
        None => Ok(DaemonStatusInfo {
            running: pid.is_some(),
            pid,
            uptime_secs: None,
            daemon_version: None,
        }),
    }
}

#[tauri::command]
async fn start_daemon(app: tauri::AppHandle) -> Result<DaemonStatusInfo, String> {
    // Check if already running
    if read_daemon_pid().is_some() {
        return daemon_status().await;
    }

    // Find the daemon binary
    let binary = find_bundled_binary(&app, "brightwing-authd")
        .or_else(|| {
            let install_dir = binaries_install_dir().ok()?;
            let path = install_dir.join("brightwing-authd");
            if path.exists() { Some(path) } else { None }
        })
        .ok_or("brightwing-authd binary not found")?;

    // Spawn daemon as a detached process
    std::process::Command::new(&binary)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start daemon: {}", e))?;

    // Wait briefly for it to start up
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    daemon_status().await
}

#[tauri::command]
async fn stop_daemon() -> Result<DaemonStatusInfo, String> {
    let pid = read_daemon_pid();
    if let Some(pid) = pid {
        #[cfg(unix)]
        {
            unsafe { libc::kill(pid as i32, libc::SIGTERM); }
        }
        #[cfg(not(unix))]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
        }

        // Wait for cleanup
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    Ok(DaemonStatusInfo {
        running: false,
        pid: None,
        uptime_secs: None,
        daemon_version: None,
    })
}

/// Launchd plist path for macOS auto-start.
#[cfg(target_os = "macos")]
fn launchd_plist_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("Library/LaunchAgents/com.brightwing.authd.plist")
}

#[tauri::command]
fn is_autostart_enabled() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(launchd_plist_path().exists())
    }
    #[cfg(target_os = "linux")]
    {
        let path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
            .join("systemd/user/brightwing-authd.service");
        Ok(path.exists())
    }
    #[cfg(target_os = "windows")]
    {
        Ok(false) // TODO: Windows registry auto-start
    }
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let binary = find_bundled_binary(&app, "brightwing-authd")
        .or_else(|| {
            let install_dir = binaries_install_dir().ok()?;
            let path = install_dir.join("brightwing-authd");
            if path.exists() { Some(path) } else { None }
        })
        .ok_or("brightwing-authd binary not found")?;

    #[cfg(target_os = "macos")]
    {
        let plist_path = launchd_plist_path();
        if enabled {
            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.brightwing.authd</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>{}/authd.log</string>
</dict>
</plist>"#,
                binary.display(),
                daemon_data_dir().display(),
            );
            if let Some(parent) = plist_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create LaunchAgents dir: {}", e))?;
            }
            std::fs::write(&plist_path, plist)
                .map_err(|e| format!("Failed to write plist: {}", e))?;

            // Load the agent
            let _ = std::process::Command::new("launchctl")
                .args(["load", &plist_path.to_string_lossy()])
                .output();
        } else {
            if plist_path.exists() {
                // Unload first
                let _ = std::process::Command::new("launchctl")
                    .args(["unload", &plist_path.to_string_lossy()])
                    .output();
                std::fs::remove_file(&plist_path)
                    .map_err(|e| format!("Failed to remove plist: {}", e))?;
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let service_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
            .join("systemd/user");
        let service_path = service_dir.join("brightwing-authd.service");

        if enabled {
            let unit = format!(
                r#"[Unit]
Description=Brightwing Auth Daemon
After=default.target

[Service]
ExecStart={}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
                binary.display(),
            );
            std::fs::create_dir_all(&service_dir)
                .map_err(|e| format!("Failed to create systemd dir: {}", e))?;
            std::fs::write(&service_path, unit)
                .map_err(|e| format!("Failed to write service file: {}", e))?;

            let _ = std::process::Command::new("systemctl")
                .args(["--user", "enable", "--now", "brightwing-authd"])
                .output();
        } else {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "--now", "brightwing-authd"])
                .output();
            if service_path.exists() {
                let _ = std::fs::remove_file(&service_path);
            }
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let _ = (binary, enabled);
        Err("Auto-start not yet implemented on Windows".to_string())
    }
}

// --- Binary Distribution ---

/// Get the path where Brightwing binaries should be installed.
fn binaries_install_dir() -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".local/bin"))
}

/// Find a binary in the app's resource directory or next to the app binary.
fn find_bundled_binary(app: &tauri::AppHandle, name: &str) -> Option<std::path::PathBuf> {
    // Check next to the app binary first (development builds)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    // Check Tauri resource directory
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[tauri::command]
fn distribute_binaries(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let install_dir = binaries_install_dir()?;
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create {}: {}", install_dir.display(), e))?;

    let binaries = ["brightwing-proxy", "brightwing-authd", "bw"];
    let mut installed = Vec::new();

    for name in &binaries {
        if let Some(src) = find_bundled_binary(&app, name) {
            let dest = install_dir.join(name);
            std::fs::copy(&src, &dest)
                .map_err(|e| format!("Failed to copy {} to {}: {}", name, dest.display(), e))?;
            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| format!("Failed to set permissions on {}: {}", name, e))?;
            }
            installed.push(name.to_string());
        }
    }

    Ok(installed)
}

#[tauri::command]
fn get_binary_versions() -> Result<HashMap<String, String>, String> {
    let install_dir = binaries_install_dir()?;
    let mut versions = HashMap::new();

    for name in &["brightwing-proxy", "brightwing-authd", "bw"] {
        let path = install_dir.join(name);
        if path.exists() {
            // Try running --version
            if let Ok(output) = std::process::Command::new(&path).arg("--version").output() {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    versions.insert(name.to_string(), version);
                } else {
                    versions.insert(name.to_string(), "installed".to_string());
                }
            } else {
                versions.insert(name.to_string(), "installed".to_string());
            }
        }
    }

    Ok(versions)
}

#[tauri::command]
fn check_cli_path() -> Result<bool, String> {
    let path_env = std::env::var("PATH").unwrap_or_default();
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let local_bin = home.join(".local/bin");
    let local_bin_str = local_bin.to_string_lossy();
    Ok(path_env.split(':').any(|p| {
        let expanded = p.replace('~', &home.to_string_lossy());
        expanded == *local_bin_str || p == "~/.local/bin"
    }))
}

// --- API Proxy (bypasses CORS) ---

const API_BASE: &str = "https://mcpscoreboard.com/api/v1";

#[tauri::command]
async fn api_search_servers(query: String, per_page: Option<u32>) -> Result<JsonValue, String> {
    let per_page = per_page.unwrap_or(25);
    let url = format!(
        "{}/servers/?q={}&per_page={}",
        API_BASE,
        urlencoding::encode(&query),
        per_page
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let json: JsonValue = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    Ok(json)
}

#[tauri::command]
async fn api_get_install_config(server_id: String) -> Result<JsonValue, String> {
    let url = format!("{}/servers/{}/install-config/", API_BASE, server_id);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    if resp.status().as_u16() == 404 {
        return Ok(JsonValue::Null);
    }
    let json: JsonValue = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    Ok(json)
}

#[tauri::command]
async fn api_get_installable_ids() -> Result<Vec<String>, String> {
    // Check cache first
    {
        if let Ok(cache) = INSTALLABLE_IDS_CACHE.lock() {
            if let Some((cached_at, ref ids)) = *cache {
                if cached_at.elapsed().as_secs() < INSTALLABLE_IDS_TTL_SECS {
                    return Ok(ids.clone());
                }
            }
        }
    }

    // Fetch page 1 to discover total_pages
    let url = format!("{}/servers/installable/?per_page=100&page=1", API_BASE);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let json: JsonValue = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut all_ids = Vec::new();
    if let Some(results) = json["results"].as_array() {
        for r in results {
            if let Some(id) = r["id"].as_str() {
                all_ids.push(id.to_string());
            }
        }
    } else {
        return Ok(all_ids);
    }

    let total_pages = json["meta"]["total_pages"].as_u64().unwrap_or(1);

    // Fetch remaining pages in parallel
    if total_pages > 1 {
        let futures: Vec<_> = (2..=total_pages as u32)
            .map(|page| {
                let url = format!(
                    "{}/servers/installable/?per_page=100&page={}",
                    API_BASE, page
                );
                async move {
                    let resp = reqwest::get(&url).await.ok()?;
                    let json: JsonValue = resp.json().await.ok()?;
                    let results = json["results"].as_array()?;
                    Some(
                        results
                            .iter()
                            .filter_map(|r| r["id"].as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        for page_ids in results.into_iter().flatten() {
            all_ids.extend(page_ids);
        }
    }

    // Update cache
    if let Ok(mut cache) = INSTALLABLE_IDS_CACHE.lock() {
        *cache = Some((Instant::now(), all_ids.clone()));
    }

    Ok(all_ids)
}

#[tauri::command]
async fn api_get_server(server_id: String) -> Result<JsonValue, String> {
    let url = format!("{}/servers/{}/", API_BASE, server_id);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let json: JsonValue = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    Ok(json)
}

// --- App Setup ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Database::new().expect("Failed to initialize database");
    let deep_link_state = DeepLinkState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // When a second instance is launched (e.g., from a deep link),
            // parse the URL and store it
            if let Some(url) = argv.iter().find(|a| a.starts_with("brightwing://")) {
                if let Some(action) = deeplink::parse_deep_link(url) {
                    if let Some(state) = app.try_state::<DeepLinkState>() {
                        if let Ok(mut pending) = state.pending.lock() {
                            *pending = Some(action.clone());
                        }
                    }
                    // Emit event to frontend
                    let _ = app.emit("deep-link-action", action);
                }
            }
            // Focus the existing window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(db)
        .manage(deep_link_state)
        .manage(oauth::flow::OAuthFlowStates::new())
        .invoke_handler(tauri::generate_handler![
            scan_tools,
            scan_configured_servers,
            read_tool_config,
            install_server,
            uninstall_server,
            get_installations,
            get_favorites,
            add_favorite,
            remove_favorite,
            get_pending_deep_link,
            clear_pending_deep_link,
            disable_server,
            enable_server,
            get_disabled_servers,
            delete_server,
            backup_tool_config,
            api_search_servers,
            api_get_install_config,
            api_get_installable_ids,
            api_get_server,
            restart_tool,
            fetch_cli_server_config,
            add_server_to_tool,
            // Auth probe
            probe_server_auth,
            // Proxy server management
            register_proxy_server,
            unregister_proxy_server,
            get_proxy_servers,
            get_proxy_server,
            install_proxy_to_tool,
            uninstall_proxy_from_tool,
            get_proxy_installs,
            get_tool_filter,
            set_tool_filter,
            set_tool_filter_bulk,
            get_cached_tools,
            cache_tool_schema,
            discover_upstream_tools,
            // OAuth
            start_oauth_flow,
            complete_oauth_callback,
            discover_oauth_server,
            get_oauth_status,
            disconnect_oauth,
            refresh_oauth_token,
            // API key management
            store_api_key,
            get_api_key,
            delete_api_key,
            get_all_api_keys,
            // Binary distribution
            distribute_binaries,
            get_binary_versions,
            check_cli_path,
            // Proxy logs
            get_proxy_logs,
            // Daemon lifecycle
            daemon_status,
            start_daemon,
            stop_daemon,
            is_autostart_enabled,
            set_autostart,
        ])
        .setup(|app| {
            // Handle deep links (open-url event from tauri-plugin-deep-link)
            {
                let handle = app.handle().clone();

                app.listen("deep-link://new-url", move |event: tauri::Event| {
                    let payload = event.payload();

                    let url_strings: Vec<String> = if let Ok(urls) = serde_json::from_str::<Vec<String>>(payload) {
                        urls
                    } else if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
                        match value {
                            serde_json::Value::Array(arr) => {
                                arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                            }
                            serde_json::Value::String(s) => vec![s],
                            _ => vec![],
                        }
                    } else {
                        vec![]
                    };

                    for url in url_strings {
                        if let Some(action) = deeplink::parse_deep_link(&url) {
                            if let Some(state) = handle.try_state::<DeepLinkState>() {
                                if let Ok(mut pending) = state.pending.lock() {
                                    *pending = Some(action.clone());
                                }
                            }
                            let _ = handle.emit("deep-link-action", action);
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Brightwing MCP Manager");
}
