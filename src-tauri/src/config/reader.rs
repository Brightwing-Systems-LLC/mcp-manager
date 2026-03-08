use crate::tools::definitions::{ConfigFormat, TOOL_DEFINITIONS};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredServer {
    pub tool_id: String,
    pub tool_short_name: String,
    pub server_name: String,
    pub config_json: Option<String>,
    pub is_cli_only: bool,
}

/// Read all configured MCP servers across all detected tools.
pub fn read_all_configured_servers() -> Vec<ConfiguredServer> {
    let mut servers = Vec::new();

    for def in TOOL_DEFINITIONS {
        if def.is_cli_only {
            // For Claude Code, try to parse `claude mcp list` output
            if let Some(cmd) = def.cli_command {
                match read_cli_servers(cmd) {
                    Ok(entries) => {
                        log::info!(
                            "Found {} servers from {} CLI",
                            entries.len(),
                            def.display_name
                        );
                        for entry in entries {
                            servers.push(ConfiguredServer {
                                tool_id: def.id.to_string(),
                                tool_short_name: def.short_name.to_string(),
                                server_name: entry.name,
                                config_json: entry.config_json,
                                is_cli_only: true,
                            });
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to read {} servers: {}", def.display_name, e);
                    }
                }
            }
            continue;
        }

        match read_installed_servers(def.id) {
            Ok(map) => {
                if !map.is_empty() {
                    log::info!(
                        "Found {} servers in {} config",
                        map.len(),
                        def.display_name
                    );
                }
                for (name, config_val) in &map {
                    let json_str = serde_json::to_string(config_val).ok();
                    servers.push(ConfiguredServer {
                        tool_id: def.id.to_string(),
                        tool_short_name: def.short_name.to_string(),
                        server_name: name.clone(),
                        config_json: json_str,
                        is_cli_only: false,
                    });
                }
            }
            Err(e) => {
                log::debug!("Skipping {}: {}", def.display_name, e);
            }
        }
    }

    servers
}

/// Result from CLI server scan: name + optional config JSON.
struct CliServerEntry {
    name: String,
    config_json: Option<String>,
}

/// Build an enriched environment for CLI subprocesses.
/// macOS GUI apps inherit minimal PATH from launchd.
pub fn build_cli_env() -> (String, Option<std::path::PathBuf>) {
    let mut path_parts: Vec<String> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        path_parts.push(home.join(".local/bin").to_string_lossy().to_string());
        path_parts.push(home.join(".cargo/bin").to_string_lossy().to_string());
        path_parts.push(home.join("bin").to_string_lossy().to_string());
    }
    path_parts.push("/usr/local/bin".to_string());
    path_parts.push("/opt/homebrew/bin".to_string());
    path_parts.push("/usr/bin".to_string());
    path_parts.push("/bin".to_string());
    if let Ok(existing) = std::env::var("PATH") {
        path_parts.push(existing);
    }
    (path_parts.join(":"), dirs::home_dir())
}

/// Run a CLI command with timeout and enriched environment.
fn run_cli_with_timeout(
    bin: &std::path::Path,
    args: &[&str],
    timeout_secs: u64,
) -> Result<std::process::Output, String> {
    let (full_path, home) = build_cli_env();
    let cmd_name = bin.file_name().unwrap_or_default().to_string_lossy();

    let mut cmd_builder = std::process::Command::new(bin);
    cmd_builder
        .args(args)
        .env("PATH", &full_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some(home) = home {
        cmd_builder.env("HOME", home);
    }

    let mut child = cmd_builder
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", cmd_name, e))?;

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(format!("{} timed out after {}s", cmd_name, timeout_secs));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Error waiting for {}: {}", cmd_name, e)),
        }
    }

    child
        .wait_with_output()
        .map_err(|e| format!("Failed to read output from {}: {}", cmd_name, e))
}

/// Parse servers from `claude mcp list` text output.
fn read_cli_servers(cmd: &str) -> Result<Vec<CliServerEntry>, String> {
    let bin = find_cli_binary(cmd).ok_or("CLI not found")?;
    read_cli_servers_text(&bin, cmd)
}

/// Parse `claude mcp list` text output, extracting names and URLs.
fn read_cli_servers_text(bin: &std::path::Path, cmd: &str) -> Result<Vec<CliServerEntry>, String> {
    let output = run_cli_with_timeout(bin, &["mcp", "list"], 30)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    let mut entries = Vec::new();
    for line in combined.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Checking")
            || line.starts_with("error")
            || line.starts_with("Warning")
        {
            continue;
        }
        // Format: "server_name: https://url (HTTP) - status"
        // or:     "server_name: https://url - status"
        if let Some(idx) = line.find(": ") {
            let name = line[..idx].trim();
            if name.is_empty() {
                continue;
            }
            let rest = line[idx + 2..].trim();
            // Try to extract URL from the rest
            // The rest looks like: "https://url (HTTP) - status" or "https://url - status"
            let config_json = if let Some(url) = extract_url_from_list_line(rest) {
                // Build a minimal config JSON for HTTP servers
                let mut obj = serde_json::Map::new();
                obj.insert("url".to_string(), JsonValue::String(url));
                serde_json::to_string(&JsonValue::Object(obj)).ok()
            } else {
                None
            };
            entries.push(CliServerEntry {
                name: name.to_string(),
                config_json,
            });
        }
    }
    Ok(entries)
}

/// Extract a URL from `claude mcp list` line remainder.
/// Input like: "https://url (HTTP) - status" or "https://url - status"
fn extract_url_from_list_line(rest: &str) -> Option<String> {
    // URL is the first token, ends at space or end of string
    let url_end = rest.find(' ').unwrap_or(rest.len());
    let url = &rest[..url_end];
    if url.starts_with("http://") || url.starts_with("https://") {
        Some(url.to_string())
    } else {
        None
    }
}

/// Fetch config JSON for a single CLI server on demand.
/// Parses the text output of `claude mcp get <name>`.
pub fn fetch_cli_server_config(cmd: &str, server_name: &str) -> Result<String, String> {
    let bin = find_cli_binary(cmd).ok_or("CLI not found")?;
    let output = run_cli_with_timeout(&bin, &["mcp", "get", server_name], 15)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to get config for {}: {}", server_name, stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse text output like:
    //   server_name:
    //     Scope: User config (...)
    //     Type: http
    //     URL: https://example.com/mcp
    //     Command: npx
    //     Args: -y @org/pkg
    //     Env: KEY=value, KEY2=value2
    parse_cli_get_output(&stdout)
}

/// Parse the text output of `claude mcp get` into a JSON config string.
fn parse_cli_get_output(output: &str) -> Result<String, String> {
    let mut server_type: Option<String> = None;
    let mut url: Option<String> = None;
    let mut command: Option<String> = None;
    let mut args_str: Option<String> = None;
    let mut env_str: Option<String> = None;
    let mut headers: Vec<(String, String)> = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Type: ") {
            server_type = Some(val.trim().to_lowercase());
        } else if let Some(val) = line.strip_prefix("URL: ") {
            url = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Command: ") {
            command = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Args: ") {
            args_str = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Env: ") {
            env_str = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Header: ") {
            // Format: "Key: Value"
            if let Some(colon_idx) = val.find(": ") {
                headers.push((
                    val[..colon_idx].trim().to_string(),
                    val[colon_idx + 2..].trim().to_string(),
                ));
            }
        }
    }

    let mut obj = serde_json::Map::new();

    match server_type.as_deref() {
        Some("http") | Some("sse") => {
            if let Some(u) = url {
                obj.insert("url".to_string(), JsonValue::String(u));
            }
            if !headers.is_empty() {
                let mut h = serde_json::Map::new();
                for (k, v) in headers {
                    h.insert(k, JsonValue::String(v));
                }
                obj.insert("headers".to_string(), JsonValue::Object(h));
            }
        }
        Some("stdio") | _ => {
            if let Some(cmd) = command {
                obj.insert("command".to_string(), JsonValue::String(cmd));
            }
            if let Some(args) = args_str {
                let args_vec: Vec<JsonValue> = args
                    .split_whitespace()
                    .map(|a| JsonValue::String(a.to_string()))
                    .collect();
                obj.insert("args".to_string(), JsonValue::Array(args_vec));
            }
        }
    }

    // Parse env vars (format: "KEY=value, KEY2=value2")
    if let Some(env) = env_str {
        let mut env_obj = serde_json::Map::new();
        for pair in env.split(", ") {
            if let Some(eq_idx) = pair.find('=') {
                env_obj.insert(
                    pair[..eq_idx].to_string(),
                    JsonValue::String(pair[eq_idx + 1..].to_string()),
                );
            }
        }
        if !env_obj.is_empty() {
            obj.insert("env".to_string(), JsonValue::Object(env_obj));
        }
    }

    if obj.is_empty() {
        return Err("Could not parse any config fields".to_string());
    }

    serde_json::to_string(&JsonValue::Object(obj))
        .map_err(|e| format!("Failed to serialize config: {}", e))
}

pub fn find_cli_binary(cmd: &str) -> Option<std::path::PathBuf> {
    if let Ok(path) = which::which(cmd) {
        return Some(path);
    }
    let extra_dirs = [
        dirs::home_dir().map(|h| h.join(".local/bin")),
        dirs::home_dir().map(|h| h.join(".cargo/bin")),
        dirs::home_dir().map(|h| h.join("bin")),
        Some(std::path::PathBuf::from("/usr/local/bin")),
        Some(std::path::PathBuf::from("/opt/homebrew/bin")),
    ];
    for dir in extra_dirs.iter().flatten() {
        let full = dir.join(cmd);
        if full.exists() {
            return Some(full);
        }
    }
    None
}

/// Read existing MCP servers from a tool's config file.
/// Returns a map of server_name -> server_config_json for each installed server.
pub fn read_installed_servers(tool_id: &str) -> Result<HashMap<String, JsonValue>, String> {
    let def = TOOL_DEFINITIONS
        .iter()
        .find(|d| d.id == tool_id)
        .ok_or_else(|| format!("Unknown tool: {}", tool_id))?;

    if def.is_cli_only {
        return Err("CLI-only tools cannot be read directly".to_string());
    }

    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", tool_id))?;

    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    match def.config_format {
        ConfigFormat::Json => read_json_servers(&config_path, def.servers_key),
        ConfigFormat::Toml => read_toml_servers(&config_path, def.servers_key),
        ConfigFormat::Cli => Err("CLI-only tools cannot be read directly".to_string()),
    }
}

fn read_json_servers(path: &Path, servers_key: &str) -> Result<HashMap<String, JsonValue>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let parsed: JsonValue =
        serde_json::from_str(&content).map_err(|e| format!("Invalid JSON in {}: {}", path.display(), e))?;

    let servers = match parsed.get(servers_key) {
        Some(JsonValue::Object(obj)) => obj
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        _ => HashMap::new(),
    };

    Ok(servers)
}

fn read_toml_servers(path: &Path, servers_key: &str) -> Result<HashMap<String, JsonValue>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let parsed: toml::Value =
        content.parse().map_err(|e| format!("Invalid TOML in {}: {}", path.display(), e))?;

    let servers = match parsed.get(servers_key) {
        Some(toml::Value::Table(table)) => {
            let mut result = HashMap::new();
            for (name, config) in table {
                // Convert TOML value to JSON value for consistent frontend handling
                let json_str = serde_json::to_string(config)
                    .map_err(|e| format!("Failed to serialize TOML to JSON: {}", e))?;
                let json_val: JsonValue = serde_json::from_str(&json_str)
                    .map_err(|e| format!("Failed to parse as JSON: {}", e))?;
                result.insert(name.clone(), json_val);
            }
            result
        }
        _ => HashMap::new(),
    };

    Ok(servers)
}
