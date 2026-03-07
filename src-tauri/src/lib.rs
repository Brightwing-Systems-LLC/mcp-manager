mod config;
mod db;
mod deeplink;
mod tools;

use config::writer::{InstallResult, ServerInstallConfig};
use db::queries::{Favorite, Installation};
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
fn backup_tool_config(tool_id: String) -> Result<String, String> {
    config::backup::backup_config(&tool_id)
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
                    let _ = app.emit("deep-link", action);
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
            read_tool_config,
            install_server,
            uninstall_server,
            get_installations,
            get_favorites,
            add_favorite,
            remove_favorite,
            get_pending_deep_link,
            clear_pending_deep_link,
            backup_tool_config,
            api_search_servers,
            api_get_install_config,
            api_get_installable_ids,
            api_get_server,
        ])
        .setup(|app| {
            // Handle deep links on macOS (open-url event)
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                let handle = app.handle().clone();
                app.listen("tauri://deep-link", move |event: tauri::Event| {
                    if let Ok(urls) = serde_json::from_str::<Vec<String>>(event.payload()) {
                        for url in urls {
                            if let Some(action) = deeplink::parse_deep_link(&url) {
                                if let Some(state) = handle.try_state::<DeepLinkState>() {
                                    if let Ok(mut pending) = state.pending.lock() {
                                        *pending = Some(action.clone());
                                    }
                                }
                                let _ = handle.emit("deep-link", action);
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Brightwing MCP Manager");
}
