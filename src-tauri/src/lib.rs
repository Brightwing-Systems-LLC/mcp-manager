mod config;
mod db;
mod deeplink;
mod tools;

use config::reader::ConfiguredServer;
use config::writer::{InstallResult, ServerInstallConfig};
use db::queries::{DisabledServer, Favorite, Installation};
use db::Database;
use deeplink::{DeepLinkAction, DeepLinkState};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tauri::{Emitter, Listener, Manager};
use tools::definitions::DetectedTool;

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
    config::writer::restore_server_entry(&tool_id, &server_name, &config_json)
}

#[tauri::command]
fn get_disabled_servers(
    db: tauri::State<'_, Database>,
) -> Result<Vec<DisabledServer>, String> {
    db.get_disabled_servers()
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
        // CLI tools don't need restart
        "claude_code" | "codex" | "gemini_cli" => {
            Ok("No restart needed for CLI tools".to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool_id)),
    }
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
    let mut all_ids = Vec::new();
    let mut page = 1u32;
    loop {
        let url = format!(
            "{}/servers/installable/?per_page=100&page={}",
            API_BASE, page
        );
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        let json: JsonValue = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        if let Some(results) = json["results"].as_array() {
            for r in results {
                if let Some(id) = r["id"].as_str() {
                    all_ids.push(id.to_string());
                }
            }
            let total_pages = json["meta"]["total_pages"].as_u64().unwrap_or(1);
            if (page as u64) >= total_pages {
                break;
            }
            page += 1;
        } else {
            break;
        }
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
        .manage(db)
        .manage(deep_link_state)
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
            backup_tool_config,
            api_search_servers,
            api_get_install_config,
            api_get_installable_ids,
            api_get_server,
            restart_tool,
            fetch_cli_server_config,
            add_server_to_tool,
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
