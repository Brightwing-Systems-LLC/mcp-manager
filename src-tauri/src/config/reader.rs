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
                    Ok(names) => {
                        log::info!(
                            "Found {} servers from {} CLI",
                            names.len(),
                            def.display_name
                        );
                        for name in names {
                            servers.push(ConfiguredServer {
                                tool_id: def.id.to_string(),
                                tool_short_name: def.short_name.to_string(),
                                server_name: name,
                                config_json: None,
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

/// Parse server names from `claude mcp list` output.
fn read_cli_servers(cmd: &str) -> Result<Vec<String>, String> {
    // Find the binary in common locations since GUI apps have limited PATH
    let bin = find_cli_binary(cmd).ok_or("CLI not found")?;

    // Build a PATH that includes common user binary locations
    // since macOS GUI apps inherit a minimal environment from launchd
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
    let full_path = path_parts.join(":");

    // Use a child process with timeout since `claude mcp list` does health checks
    let mut cmd_builder = std::process::Command::new(&bin);
    cmd_builder
        .args(["mcp", "list"])
        .env("PATH", &full_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Ensure HOME is set (macOS GUI apps may not have it)
    if let Some(home) = dirs::home_dir() {
        cmd_builder.env("HOME", home);
    }

    let mut child = cmd_builder
        .spawn()
        .map_err(|e| format!("Failed to spawn {} mcp list: {}", cmd, e))?;

    // Wait up to 30 seconds for the command to complete
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(format!("{} mcp list timed out after 30s", cmd));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Error waiting for {} mcp list: {}", cmd, e)),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to read output from {} mcp list: {}", cmd, e))?;

    // Parse both stdout and stderr since some output may go to either
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    let mut names = Vec::new();
    for line in combined.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Checking")
            || line.starts_with("error")
            || line.starts_with("Warning")
        {
            continue;
        }
        // Format: "server_name: details..." — the name is everything before the first colon
        // but names themselves can contain colons (e.g. "plugin:sentry:sentry")
        // The pattern is: name + ": " + url_or_details
        if let Some(idx) = line.find(": ") {
            let name = line[..idx].trim();
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
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
