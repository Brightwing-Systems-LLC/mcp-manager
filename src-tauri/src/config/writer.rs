use crate::config::reader::{build_cli_env, find_cli_binary};
use crate::tools::definitions::{ConfigFormat, TOOL_DEFINITIONS};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInstallConfig {
    pub server_name: String,
    pub config_key: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub transport: String, // "stdio" or "http"
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
    pub needs_restart: bool,
}

pub fn install_server(tool_id: &str, config: &ServerInstallConfig) -> Result<InstallResult, String> {
    let def = TOOL_DEFINITIONS
        .iter()
        .find(|d| d.id == tool_id)
        .ok_or_else(|| format!("Unknown tool: {}", tool_id))?;

    // Sanitize the config key for tools that only accept identifier characters
    let config = if needs_sanitizing(&config.config_key) {
        let mut sanitized = config.clone();
        sanitized.config_key = sanitize_server_name(&sanitized.config_key);
        sanitized
    } else {
        config.clone()
    };

    match def.config_format {
        ConfigFormat::Json => install_json(def, &config),
        ConfigFormat::Toml => install_toml(def, &config),
        ConfigFormat::Cli => install_cli(def, &config),
    }
}

pub fn uninstall_server(tool_id: &str, config_key: &str) -> Result<InstallResult, String> {
    let def = TOOL_DEFINITIONS
        .iter()
        .find(|d| d.id == tool_id)
        .ok_or_else(|| format!("Unknown tool: {}", tool_id))?;

    match def.config_format {
        ConfigFormat::Json => uninstall_json(def, config_key),
        ConfigFormat::Toml => uninstall_toml(def, config_key),
        ConfigFormat::Cli => uninstall_cli(def, config_key),
    }
}

fn build_server_entry(def: &crate::tools::definitions::ToolDefinition, config: &ServerInstallConfig) -> JsonValue {
    // Zed uses a unique format: {"command": {"path": "cmd", "args": [...]}, "settings": {}}
    if def.id == "zed" {
        let mut entry = serde_json::Map::new();
        if config.transport == "http" || config.transport == "sse" {
            // Zed doesn't have a documented HTTP MCP format — use url settings
            let mut settings = serde_json::Map::new();
            settings.insert("url".to_string(), JsonValue::String(config.url.clone()));
            entry.insert("settings".to_string(), JsonValue::Object(settings));
        } else {
            let mut cmd_obj = serde_json::Map::new();
            cmd_obj.insert("path".to_string(), JsonValue::String(config.command.clone()));
            cmd_obj.insert(
                "args".to_string(),
                JsonValue::Array(config.args.iter().map(|a| JsonValue::String(a.clone())).collect()),
            );
            if !config.env.is_empty() {
                let env_obj: serde_json::Map<String, JsonValue> = config.env.iter()
                    .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
                    .collect();
                cmd_obj.insert("env".to_string(), JsonValue::Object(env_obj));
            }
            entry.insert("command".to_string(), JsonValue::Object(cmd_obj));
        }
        return JsonValue::Object(entry);
    }

    let mut entry = serde_json::Map::new();

    if config.transport == "http" || config.transport == "sse" {
        // HTTP/SSE transport — use URL
        if def.needs_type_field {
            entry.insert("type".to_string(), JsonValue::String(config.transport.clone()));
        }
        if def.uses_command_array {
            entry.insert("type".to_string(), JsonValue::String("remote".to_string()));
        }
        // Use tool-specific URL key if defined, otherwise "url"
        let url_key = def.remote_url_key.unwrap_or("url");
        entry.insert(url_key.to_string(), JsonValue::String(config.url.clone()));
    } else if def.uses_command_array {
        // OpenCode format: command is an array [cmd, arg1, arg2, ...]
        entry.insert("type".to_string(), JsonValue::String("local".to_string()));
        let mut cmd_array = vec![JsonValue::String(config.command.clone())];
        cmd_array.extend(config.args.iter().map(|a| JsonValue::String(a.clone())));
        entry.insert("command".to_string(), JsonValue::Array(cmd_array));
    } else {
        // stdio transport — use command/args
        if def.needs_type_field {
            entry.insert("type".to_string(), JsonValue::String(config.transport.clone()));
        }
        entry.insert(
            "command".to_string(),
            JsonValue::String(config.command.clone()),
        );
        entry.insert(
            "args".to_string(),
            JsonValue::Array(config.args.iter().map(|a| JsonValue::String(a.clone())).collect()),
        );
    }

    if !config.env.is_empty() {
        let env_obj: serde_json::Map<String, JsonValue> = config
            .env
            .iter()
            .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
            .collect();
        entry.insert(def.env_key.to_string(), JsonValue::Object(env_obj));
    }

    JsonValue::Object(entry)
}

/// Ensure a nested dotted key path exists in a JSON object, creating intermediate
/// objects as needed. Returns a mutable reference to the leaf object.
fn json_ensure_nested<'a>(root: &'a mut JsonValue, key: &str) -> Result<&'a mut serde_json::Map<String, JsonValue>, String> {
    if key.contains('.') {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = root;
        for part in &parts {
            if !current.as_object().map_or(false, |o| o.contains_key(*part)) {
                current.as_object_mut()
                    .ok_or("Not an object")?
                    .insert(part.to_string(), JsonValue::Object(serde_json::Map::new()));
            }
            current = current.get_mut(*part).ok_or("Failed to traverse key")?;
        }
        current.as_object_mut().ok_or_else(|| format!("'{}' is not an object", key))
    } else {
        let root_obj = root.as_object_mut().ok_or("Config root is not a JSON object")?;
        if !root_obj.contains_key(key) {
            root_obj.insert(key.to_string(), JsonValue::Object(serde_json::Map::new()));
        }
        root_obj.get_mut(key)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| format!("'{}' is not an object", key))
    }
}

/// Get a mutable reference to the servers object at a dotted key path.
fn json_get_nested_mut<'a>(root: &'a mut JsonValue, key: &str) -> Option<&'a mut serde_json::Map<String, JsonValue>> {
    if key.contains('.') {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = root;
        for part in &parts {
            current = current.get_mut(*part)?;
        }
        current.as_object_mut()
    } else {
        root.get_mut(key).and_then(|v| v.as_object_mut())
    }
}

fn install_json(
    def: &crate::tools::definitions::ToolDefinition,
    config: &ServerInstallConfig,
) -> Result<InstallResult, String> {
    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", def.id))?;

    // Read existing or create new
    let mut root: JsonValue = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON in config: {}", e))?
    } else {
        // Create parent dirs if needed
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        JsonValue::Object(serde_json::Map::new())
    };

    let servers = json_ensure_nested(&mut root, def.servers_key)?;

    let entry = build_server_entry(def, config);
    servers.insert(config.config_key.clone(), entry);

    // Write back
    let output = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&config_path, output)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(InstallResult {
        success: true,
        message: format!(
            "Installed {} into {}",
            config.config_key, def.display_name
        ),
        needs_restart: true,
    })
}

fn install_toml(
    def: &crate::tools::definitions::ToolDefinition,
    config: &ServerInstallConfig,
) -> Result<InstallResult, String> {
    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", def.id))?;

    // Use toml_edit to preserve formatting
    let mut doc: toml_edit::DocumentMut = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        content
            .parse()
            .map_err(|e| format!("Invalid TOML in config: {}", e))?
    } else {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        toml_edit::DocumentMut::new()
    };

    // Ensure mcp_servers table exists
    if !doc.contains_key(def.servers_key) {
        doc[def.servers_key] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    // Build the server entry as a TOML table
    let server_table = doc[def.servers_key]
        .as_table_mut()
        .ok_or("mcp_servers is not a table")?;

    let mut entry = toml_edit::Table::new();

    if config.transport == "http" || config.transport == "sse" {
        let url_key = def.remote_url_key.unwrap_or("url");
        entry.insert(url_key, toml_edit::value(&config.url));
    } else {
        entry.insert("command", toml_edit::value(&config.command));
        let mut args_arr = toml_edit::Array::new();
        for arg in &config.args {
            args_arr.push(arg.as_str());
        }
        entry.insert("args", toml_edit::value(args_arr));
    }

    if !config.env.is_empty() {
        let mut env_table = toml_edit::InlineTable::new();
        for (k, v) in &config.env {
            env_table.insert(k, v.as_str().into());
        }
        entry.insert("env", toml_edit::value(env_table));
    }

    server_table.insert(&config.config_key, toml_edit::Item::Table(entry));

    // Use dotted keys (e.g. [mcp_servers.name]) instead of a bare [mcp_servers] header
    // to avoid breaking parsers that treat a later bare header as an override
    if let Some(table) = doc.get_mut(def.servers_key).and_then(|v| v.as_table_mut()) {
        table.set_implicit(true);
    }

    fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(InstallResult {
        success: true,
        message: format!(
            "Installed {} into {}",
            config.config_key, def.display_name
        ),
        needs_restart: true,
    })
}

fn install_cli(
    def: &crate::tools::definitions::ToolDefinition,
    config: &ServerInstallConfig,
) -> Result<InstallResult, String> {
    let cli_cmd = def
        .cli_command
        .ok_or_else(|| format!("No CLI command for {}", def.id))?;

    let bin = find_cli_binary(cli_cmd)
        .unwrap_or_else(|| std::path::PathBuf::from(cli_cmd));
    let (full_path, home) = build_cli_env();

    // Build the JSON config for claude mcp add-json
    let entry = build_server_entry(def, config);
    let json_str = serde_json::to_string(&entry)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    let mut cmd_builder = std::process::Command::new(&bin);
    cmd_builder
        .args(["mcp", "add-json", &config.config_key, &json_str, "--scope", "user"])
        .env("PATH", &full_path);
    if let Some(h) = home { cmd_builder.env("HOME", h); }

    let output = cmd_builder
        .output()
        .map_err(|e| format!("Failed to run {} mcp add-json: {}", cli_cmd, e))?;

    if output.status.success() {
        Ok(InstallResult {
            success: true,
            message: format!(
                "Installed {} into {} via CLI",
                config.config_key, def.display_name
            ),
            needs_restart: false,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} mcp add-json failed: {}", cli_cmd, stderr))
    }
}

/// Sanitize a server name into a valid config key.
/// Many tools (Codex, Claude Code) only accept [a-zA-Z0-9_-] identifiers.
/// "claude.ai AI Cost Manager" -> "claude_ai_ai_cost_manager"
pub fn sanitize_server_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Collapse multiple underscores
    let mut result = String::new();
    let mut prev_underscore = false;
    for c in sanitized.chars() {
        if c == '_' {
            if !prev_underscore {
                result.push(c);
            }
            prev_underscore = true;
        } else {
            result.push(c);
            prev_underscore = false;
        }
    }
    result.trim_matches('_').to_lowercase()
}

/// Build the path to the brightwing-proxy binary.
/// Checks ~/.local/bin first, then falls back to the cargo target directory.
pub fn proxy_binary_path() -> std::path::PathBuf {
    // Primary: user's local bin (where Brightwing copies binaries)
    if let Some(home) = dirs::home_dir() {
        let local_bin = home.join(".local/bin/brightwing-proxy");
        if local_bin.exists() {
            return local_bin;
        }
    }
    // Fallback: check PATH
    if let Ok(path) = which::which("brightwing-proxy") {
        return path;
    }
    // Last resort: assume it'll be at ~/.local/bin (pre-distribution)
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/usr/local/bin"))
        .join(".local/bin/brightwing-proxy")
}

/// Install a server in "proxy" mode — writes a config that spawns brightwing-proxy
/// instead of the original server command. The proxy handles auth via the daemon.
pub fn install_proxy_server(
    tool_id: &str,
    server_id: &str,
    config_key: &str,
) -> Result<InstallResult, String> {
    let proxy_bin = proxy_binary_path();
    let proxy_config = ServerInstallConfig {
        server_name: config_key.to_string(),
        config_key: config_key.to_string(),
        command: proxy_bin.to_string_lossy().to_string(),
        args: vec!["--server".to_string(), server_id.to_string()],
        env: HashMap::new(),
        transport: "stdio".to_string(),
        url: String::new(),
    };
    install_server(tool_id, &proxy_config)
}

/// Check if a server name needs sanitizing (contains non-identifier chars).
pub fn needs_sanitizing(name: &str) -> bool {
    name.chars().any(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
}

/// Restore a server entry from raw JSON into a tool's config file.
/// The server_name is used as-is for the config key. Callers should
/// sanitize it first if needed (see sanitize_server_name).
pub fn restore_server_entry(
    tool_id: &str,
    server_name: &str,
    config_json: &str,
) -> Result<InstallResult, String> {
    let def = TOOL_DEFINITIONS
        .iter()
        .find(|d| d.id == tool_id)
        .ok_or_else(|| format!("Unknown tool: {}", tool_id))?;

    let safe_name = if needs_sanitizing(server_name) {
        sanitize_server_name(server_name)
    } else {
        server_name.to_string()
    };

    match def.config_format {
        ConfigFormat::Json => {
            let entry: JsonValue = serde_json::from_str(config_json)
                .map_err(|e| format!("Invalid config JSON: {}", e))?;
            restore_json(def, &safe_name, entry)
        }
        ConfigFormat::Toml => {
            restore_toml(def, &safe_name, config_json)
        }
        ConfigFormat::Cli => {
            restore_cli(def, &safe_name, config_json)
        }
    }
}

fn restore_json(
    def: &crate::tools::definitions::ToolDefinition,
    server_name: &str,
    entry: JsonValue,
) -> Result<InstallResult, String> {
    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", def.id))?;

    let mut root: JsonValue = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON in config: {}", e))?
    } else {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        JsonValue::Object(serde_json::Map::new())
    };

    let servers = json_ensure_nested(&mut root, def.servers_key)?;
    servers.insert(server_name.to_string(), entry);

    let output = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&config_path, output)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(InstallResult {
        success: true,
        message: format!("Restored {} in {}", server_name, def.display_name),
        needs_restart: true,
    })
}

fn restore_toml(
    def: &crate::tools::definitions::ToolDefinition,
    server_name: &str,
    config_json: &str,
) -> Result<InstallResult, String> {
    // Convert JSON back to TOML-compatible structure
    let json_val: JsonValue = serde_json::from_str(config_json)
        .map_err(|e| format!("Invalid config JSON: {}", e))?;
    let toml_val: toml::Value = serde_json::from_str(config_json)
        .map_err(|e| format!("Failed to convert JSON to TOML: {}", e))?;

    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", def.id))?;

    let mut doc: toml_edit::DocumentMut = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        content
            .parse()
            .map_err(|e| format!("Invalid TOML: {}", e))?
    } else {
        toml_edit::DocumentMut::new()
    };

    if !doc.contains_key(def.servers_key) {
        doc[def.servers_key] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    // Serialize the JSON value as a TOML string, parse it, and insert
    let toml_str = toml::to_string(&toml_val)
        .map_err(|e| format!("Failed to serialize as TOML: {}", e))?;
    let wrapper = format!("[\"{}\"]\n{}", server_name, toml_str);
    let parsed: toml_edit::DocumentMut = wrapper
        .parse()
        .map_err(|e| format!("Failed to parse TOML entry: {}", e))?;

    if let Some(entry) = parsed.get(server_name) {
        doc[def.servers_key][server_name] = entry.clone();
    }

    // Use dotted keys only — no bare [mcp_servers] header
    if let Some(table) = doc.get_mut(def.servers_key).and_then(|v| v.as_table_mut()) {
        table.set_implicit(true);
    }

    fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("Failed to write config: {}", e))?;

    let _ = json_val; // suppress unused warning
    Ok(InstallResult {
        success: true,
        message: format!("Restored {} in {}", server_name, def.display_name),
        needs_restart: true,
    })
}

fn restore_cli(
    def: &crate::tools::definitions::ToolDefinition,
    server_name: &str,
    config_json: &str,
) -> Result<InstallResult, String> {
    let cli_cmd = def
        .cli_command
        .ok_or_else(|| format!("No CLI command for {}", def.id))?;

    let bin = find_cli_binary(cli_cmd)
        .unwrap_or_else(|| std::path::PathBuf::from(cli_cmd));
    let (full_path, home) = build_cli_env();

    let mut cmd_builder = std::process::Command::new(&bin);
    cmd_builder
        .args(["mcp", "add-json", server_name, config_json, "--scope", "user"])
        .env("PATH", &full_path);
    if let Some(h) = home { cmd_builder.env("HOME", h); }

    let output = cmd_builder
        .output()
        .map_err(|e| format!("Failed to run {} mcp add-json: {}", cli_cmd, e))?;

    if output.status.success() {
        Ok(InstallResult {
            success: true,
            message: format!("Restored {} in {} via CLI", server_name, def.display_name),
            needs_restart: false,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} mcp add-json failed: {}", cli_cmd, stderr))
    }
}

fn uninstall_json(
    def: &crate::tools::definitions::ToolDefinition,
    config_key: &str,
) -> Result<InstallResult, String> {
    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", def.id))?;

    if !config_path.exists() {
        return Ok(InstallResult {
            success: true,
            message: "Config file doesn't exist, nothing to remove".to_string(),
            needs_restart: false,
        });
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let mut root: JsonValue =
        serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?;

    if let Some(servers) = json_get_nested_mut(&mut root, def.servers_key) {
        servers.remove(config_key);
    }

    let output = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&config_path, output)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(InstallResult {
        success: true,
        message: format!("Removed {} from {}", config_key, def.display_name),
        needs_restart: true,
    })
}

fn uninstall_toml(
    def: &crate::tools::definitions::ToolDefinition,
    config_key: &str,
) -> Result<InstallResult, String> {
    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", def.id))?;

    if !config_path.exists() {
        return Ok(InstallResult {
            success: true,
            message: "Config file doesn't exist, nothing to remove".to_string(),
            needs_restart: false,
        });
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .map_err(|e| format!("Invalid TOML: {}", e))?;

    if let Some(servers) = doc.get_mut(def.servers_key).and_then(|v| v.as_table_mut()) {
        servers.remove(config_key);
        // Keep dotted keys only — no bare [mcp_servers] header
        servers.set_implicit(true);
    }

    fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(InstallResult {
        success: true,
        message: format!("Removed {} from {}", config_key, def.display_name),
        needs_restart: true,
    })
}

fn uninstall_cli(
    def: &crate::tools::definitions::ToolDefinition,
    config_key: &str,
) -> Result<InstallResult, String> {
    // For Claude Code, edit ~/.claude.json directly instead of shelling out to
    // `claude mcp remove`, which can hang or fail when Claude Code is running.
    if def.id == "claude_code" {
        return uninstall_claude_code_direct(config_key);
    }

    let cli_cmd = def
        .cli_command
        .ok_or_else(|| format!("No CLI command for {}", def.id))?;

    let bin = find_cli_binary(cli_cmd)
        .unwrap_or_else(|| std::path::PathBuf::from(cli_cmd));
    let (full_path, home) = build_cli_env();

    let mut cmd_builder = std::process::Command::new(&bin);
    cmd_builder
        .args(["mcp", "remove", config_key, "--scope", "user"])
        .env("PATH", &full_path);
    if let Some(h) = home { cmd_builder.env("HOME", h); }

    let output = cmd_builder
        .output()
        .map_err(|e| format!("Failed to run {} mcp remove: {}", cli_cmd, e))?;

    if output.status.success() {
        Ok(InstallResult {
            success: true,
            message: format!("Removed {} from {} via CLI", config_key, def.display_name),
            needs_restart: false,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} mcp remove failed: {}", cli_cmd, stderr))
    }
}

/// Remove an MCP server from ~/.claude.json by editing the file directly.
/// This avoids `claude mcp remove` which can hang when Claude Code is running.
fn uninstall_claude_code_direct(config_key: &str) -> Result<InstallResult, String> {
    let config_path = dirs::home_dir()
        .ok_or("No home directory")?
        .join(".claude.json");

    if !config_path.exists() {
        return Ok(InstallResult {
            success: true,
            message: format!("{} not found in Claude Code (no config file)", config_key),
            needs_restart: false,
        });
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read ~/.claude.json: {}", e))?;
    let mut root: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid JSON in ~/.claude.json: {}", e))?;

    let mut removed = false;

    // Remove from top-level mcpServers
    if let Some(serde_json::Value::Object(servers)) = root.get_mut("mcpServers") {
        if servers.remove(config_key).is_some() {
            removed = true;
        }
    }

    // Remove from project-scoped mcpServers
    if let Some(serde_json::Value::Object(projects)) = root.get_mut("projects") {
        for (_path, project_config) in projects.iter_mut() {
            if let Some(serde_json::Value::Object(servers)) = project_config.get_mut("mcpServers") {
                if servers.remove(config_key).is_some() {
                    removed = true;
                }
            }
        }
    }

    if removed {
        let updated = serde_json::to_string_pretty(&root)
            .map_err(|e| format!("Failed to serialize ~/.claude.json: {}", e))?;
        std::fs::write(&config_path, updated)
            .map_err(|e| format!("Failed to write ~/.claude.json: {}", e))?;
    }

    Ok(InstallResult {
        success: true,
        message: if removed {
            format!("Removed {} from Claude Code", config_key)
        } else {
            format!("{} not found in Claude Code config", config_key)
        },
        needs_restart: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::definitions::{ConfigFormat, ToolDefinition};

    fn make_stdio_config(name: &str) -> ServerInstallConfig {
        ServerInstallConfig {
            server_name: name.to_string(),
            config_key: name.to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@org/pkg".to_string()],
            env: HashMap::from([("API_KEY".to_string(), "secret".to_string())]),
            transport: "stdio".to_string(),
            url: String::new(),
        }
    }

    fn make_http_config(name: &str, url: &str) -> ServerInstallConfig {
        ServerInstallConfig {
            server_name: name.to_string(),
            config_key: name.to_string(),
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            transport: "http".to_string(),
            url: url.to_string(),
        }
    }

    fn standard_def() -> ToolDefinition {
        ToolDefinition {
            id: "test_tool",
            display_name: "Test Tool",
            short_name: "TT",
            config_format: ConfigFormat::Json,
            servers_key: "mcpServers",
            needs_type_field: false,
            remote_url_key: None,
            is_cli_only: false,
            cli_command: None,
            uses_command_array: false,
            env_key: "env",
        }
    }

    fn vscode_def() -> ToolDefinition {
        ToolDefinition {
            id: "vscode",
            display_name: "VS Code",
            short_name: "VC",
            config_format: ConfigFormat::Json,
            servers_key: "servers",
            needs_type_field: true,
            remote_url_key: Some("url"),
            is_cli_only: false,
            cli_command: None,
            uses_command_array: false,
            env_key: "env",
        }
    }

    fn opencode_def() -> ToolDefinition {
        ToolDefinition {
            id: "opencode",
            display_name: "OpenCode",
            short_name: "OC",
            config_format: ConfigFormat::Json,
            servers_key: "mcp",
            needs_type_field: false,
            remote_url_key: None,
            is_cli_only: false,
            cli_command: None,
            uses_command_array: true,
            env_key: "environment",
        }
    }

    fn gemini_def() -> ToolDefinition {
        ToolDefinition {
            id: "gemini_cli",
            display_name: "Gemini CLI",
            short_name: "GC",
            config_format: ConfigFormat::Json,
            servers_key: "mcpServers",
            needs_type_field: false,
            remote_url_key: Some("httpUrl"),
            is_cli_only: false,
            cli_command: None,
            uses_command_array: false,
            env_key: "env",
        }
    }

    // ── build_server_entry ──

    #[test]
    fn test_build_entry_standard_stdio() {
        let def = standard_def();
        let config = make_stdio_config("my-server");
        let entry = build_server_entry(&def, &config);

        assert_eq!(entry["command"], "npx");
        assert_eq!(entry["args"][0], "-y");
        assert_eq!(entry["args"][1], "@org/pkg");
        assert_eq!(entry["env"]["API_KEY"], "secret");
        assert!(entry.get("type").is_none());
    }

    #[test]
    fn test_build_entry_standard_http() {
        let def = standard_def();
        let config = make_http_config("my-server", "https://example.com/mcp");
        let entry = build_server_entry(&def, &config);

        assert_eq!(entry["url"], "https://example.com/mcp");
        assert!(entry.get("command").is_none());
        assert!(entry.get("type").is_none());
    }

    #[test]
    fn test_build_entry_vscode_adds_type_field() {
        let def = vscode_def();
        let config = make_stdio_config("my-server");
        let entry = build_server_entry(&def, &config);

        assert_eq!(entry["type"], "stdio");
        assert_eq!(entry["command"], "npx");
    }

    #[test]
    fn test_build_entry_vscode_http() {
        let def = vscode_def();
        let config = make_http_config("my-server", "https://example.com/mcp");
        let entry = build_server_entry(&def, &config);

        assert_eq!(entry["type"], "http");
        assert_eq!(entry["url"], "https://example.com/mcp");
    }

    #[test]
    fn test_build_entry_opencode_stdio_uses_command_array() {
        let def = opencode_def();
        let config = make_stdio_config("my-server");
        let entry = build_server_entry(&def, &config);

        // OpenCode: type is "local", command is an array [cmd, arg1, arg2]
        assert_eq!(entry["type"], "local");
        assert!(entry["command"].is_array());
        let cmd_arr = entry["command"].as_array().unwrap();
        assert_eq!(cmd_arr[0], "npx");
        assert_eq!(cmd_arr[1], "-y");
        assert_eq!(cmd_arr[2], "@org/pkg");
        // No separate "args" key
        assert!(entry.get("args").is_none());
        // Uses "environment" instead of "env"
        assert_eq!(entry["environment"]["API_KEY"], "secret");
        assert!(entry.get("env").is_none());
    }

    #[test]
    fn test_build_entry_opencode_http_uses_remote_type() {
        let def = opencode_def();
        let config = make_http_config("my-server", "https://example.com/mcp");
        let entry = build_server_entry(&def, &config);

        assert_eq!(entry["type"], "remote");
        assert_eq!(entry["url"], "https://example.com/mcp");
    }

    #[test]
    fn test_build_entry_gemini_uses_httpurl_key() {
        let def = gemini_def();
        let config = make_http_config("my-server", "https://example.com/mcp");
        let entry = build_server_entry(&def, &config);

        assert_eq!(entry["httpUrl"], "https://example.com/mcp");
        assert!(entry.get("url").is_none());
    }

    #[test]
    fn test_build_entry_no_env_when_empty() {
        let def = standard_def();
        let config = make_http_config("my-server", "https://example.com");
        let entry = build_server_entry(&def, &config);

        assert!(entry.get("env").is_none());
    }

    // ── install_json / uninstall_json (file round-trip) ──

    #[test]
    fn test_install_json_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        // Create a def that points at our temp file
        let def = standard_def();
        let config = make_stdio_config("my-server");
        let entry = build_server_entry(&def, &config);

        // Manually do what install_json does, using our temp path
        let mut root = JsonValue::Object(serde_json::Map::new());
        let root_obj = root.as_object_mut().unwrap();
        root_obj.insert(
            "mcpServers".to_string(),
            JsonValue::Object(serde_json::Map::new()),
        );
        let servers = root_obj.get_mut("mcpServers").unwrap().as_object_mut().unwrap();
        servers.insert("my-server".to_string(), entry);

        let output = serde_json::to_string_pretty(&root).unwrap();
        std::fs::write(&config_path, &output).unwrap();

        // Read it back
        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["mcpServers"]["my-server"]["command"], "npx");
        assert_eq!(parsed["mcpServers"]["my-server"]["args"][0], "-y");
    }

    #[test]
    fn test_install_json_preserves_existing_servers() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        // Write initial config with one server
        let initial = r#"{"mcpServers":{"existing":{"command":"node","args":["a.js"]}}}"#;
        std::fs::write(&config_path, initial).unwrap();

        // Add a second server
        let content = std::fs::read_to_string(&config_path).unwrap();
        let mut root: JsonValue = serde_json::from_str(&content).unwrap();
        let servers = root.get_mut("mcpServers").unwrap().as_object_mut().unwrap();
        let def = standard_def();
        let config = make_stdio_config("new-server");
        servers.insert("new-server".to_string(), build_server_entry(&def, &config));
        std::fs::write(&config_path, serde_json::to_string_pretty(&root).unwrap()).unwrap();

        // Both should exist
        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["mcpServers"]["existing"]["command"], "node");
        assert_eq!(parsed["mcpServers"]["new-server"]["command"], "npx");
    }

    #[test]
    fn test_uninstall_json_removes_server() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        let initial = r#"{"mcpServers":{"keep":{"command":"a"},"remove":{"command":"b"}}}"#;
        std::fs::write(&config_path, initial).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let mut root: JsonValue = serde_json::from_str(&content).unwrap();
        root.get_mut("mcpServers").unwrap().as_object_mut().unwrap().remove("remove");
        std::fs::write(&config_path, serde_json::to_string_pretty(&root).unwrap()).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();
        assert!(parsed["mcpServers"].get("keep").is_some());
        assert!(parsed["mcpServers"].get("remove").is_none());
    }

    // ── install_toml round-trip ──

    #[test]
    fn test_install_toml_creates_server() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        let mut doc = toml_edit::DocumentMut::new();
        doc["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
        let table = doc["mcp_servers"].as_table_mut().unwrap();

        let mut entry = toml_edit::Table::new();
        entry.insert("command", toml_edit::value("npx"));
        let mut args = toml_edit::Array::new();
        args.push("-y");
        args.push("@org/pkg");
        entry.insert("args", toml_edit::value(args));
        table.insert("my-server", toml_edit::Item::Table(entry));

        std::fs::write(&config_path, doc.to_string()).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: toml::Value = content.parse().unwrap();
        assert_eq!(
            parsed["mcp_servers"]["my-server"]["command"].as_str().unwrap(),
            "npx"
        );
    }

    // ── sanitize_server_name ──

    #[test]
    fn test_sanitize_basic() {
        assert_eq!(sanitize_server_name("my-server"), "my-server");
    }

    #[test]
    fn test_sanitize_spaces() {
        assert_eq!(sanitize_server_name("my server name"), "my_server_name");
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(sanitize_server_name("claude.ai AI Cost Manager"), "claude_ai_ai_cost_manager");
    }

    #[test]
    fn test_sanitize_collapses_underscores() {
        assert_eq!(sanitize_server_name("a___b"), "a_b");
    }

    #[test]
    fn test_sanitize_trims_underscores() {
        assert_eq!(sanitize_server_name("_hello_"), "hello");
    }

    #[test]
    fn test_sanitize_preserves_hyphens() {
        assert_eq!(sanitize_server_name("my-cool-server"), "my-cool-server");
    }

    #[test]
    fn test_sanitize_mixed() {
        assert_eq!(sanitize_server_name("My Server (v2.0)"), "my_server_v2_0");
    }

    // ── needs_sanitizing ──

    #[test]
    fn test_needs_sanitizing_clean() {
        assert!(!needs_sanitizing("my-server_name"));
    }

    #[test]
    fn test_needs_sanitizing_spaces() {
        assert!(needs_sanitizing("my server"));
    }

    #[test]
    fn test_needs_sanitizing_dots() {
        assert!(needs_sanitizing("claude.ai"));
    }

    // ── OpenCode format round-trip ──

    #[test]
    fn test_opencode_server_entry_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("opencode.json");

        let def = opencode_def();
        let config = make_stdio_config("proxy-server");
        let entry = build_server_entry(&def, &config);

        // Write in OpenCode format
        let mut root = serde_json::Map::new();
        let mut mcp = serde_json::Map::new();
        mcp.insert("proxy-server".to_string(), entry);
        root.insert("mcp".to_string(), JsonValue::Object(mcp));
        std::fs::write(&config_path, serde_json::to_string_pretty(&JsonValue::Object(root)).unwrap()).unwrap();

        // Read back with the reader
        let servers = crate::config::reader::tests::read_json_servers_pub(
            &config_path, "mcp"
        );
        assert_eq!(servers.len(), 1);
        let server = &servers["proxy-server"];
        assert_eq!(server["type"], "local");
        assert!(server["command"].is_array());
        assert_eq!(server["environment"]["API_KEY"], "secret");
    }

    // ── Zed format ──

    fn zed_def() -> ToolDefinition {
        ToolDefinition {
            id: "zed",
            display_name: "Zed",
            short_name: "ZD",
            config_format: ConfigFormat::Json,
            servers_key: "context_servers",
            needs_type_field: false,
            remote_url_key: None,
            is_cli_only: false,
            cli_command: None,
            uses_command_array: false,
            env_key: "env",
        }
    }

    fn amp_def() -> ToolDefinition {
        ToolDefinition {
            id: "amp",
            display_name: "Amp",
            short_name: "AM",
            config_format: ConfigFormat::Json,
            servers_key: "amp.mcpServers",
            needs_type_field: false,
            remote_url_key: None,
            is_cli_only: false,
            cli_command: None,
            uses_command_array: false,
            env_key: "env",
        }
    }

    #[test]
    fn test_build_entry_zed_stdio() {
        let def = zed_def();
        let config = make_stdio_config("my-server");
        let entry = build_server_entry(&def, &config);

        // Zed uses {"command": {"path": "...", "args": [...], "env": {...}}}
        assert!(entry.get("command").unwrap().is_object());
        let cmd = entry["command"].as_object().unwrap();
        assert_eq!(cmd["path"], "npx");
        assert_eq!(cmd["args"][0], "-y");
        assert_eq!(cmd["env"]["API_KEY"], "secret");
        // Should NOT have top-level command/args
        assert!(entry.get("args").is_none());
    }

    #[test]
    fn test_build_entry_zed_no_env_in_command_when_empty() {
        let def = zed_def();
        let config = make_http_config("my-server", "https://example.com");
        let entry = build_server_entry(&def, &config);

        // HTTP goes into settings
        assert!(entry.get("settings").is_some());
        assert_eq!(entry["settings"]["url"], "https://example.com");
    }

    // ── Amp dotted key ──

    #[test]
    fn test_json_ensure_nested_dotted_key() {
        let mut root = JsonValue::Object(serde_json::Map::new());
        let servers = json_ensure_nested(&mut root, "amp.mcpServers").unwrap();
        servers.insert("test".to_string(), JsonValue::String("ok".to_string()));

        assert_eq!(root["amp"]["mcpServers"]["test"], "ok");
    }

    #[test]
    fn test_json_ensure_nested_simple_key() {
        let mut root = JsonValue::Object(serde_json::Map::new());
        let servers = json_ensure_nested(&mut root, "mcpServers").unwrap();
        servers.insert("test".to_string(), JsonValue::String("ok".to_string()));

        assert_eq!(root["mcpServers"]["test"], "ok");
    }

    #[test]
    fn test_json_get_nested_mut_dotted() {
        let mut root: JsonValue = serde_json::from_str(
            r#"{"amp": {"mcpServers": {"server1": {"command": "test"}}}}"#
        ).unwrap();

        let servers = json_get_nested_mut(&mut root, "amp.mcpServers").unwrap();
        servers.remove("server1");
        assert!(root["amp"]["mcpServers"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_build_entry_amp_standard_format() {
        // Amp uses standard command/args format, just with a dotted key
        let def = amp_def();
        let config = make_stdio_config("my-server");
        let entry = build_server_entry(&def, &config);

        assert_eq!(entry["command"], "npx");
        assert_eq!(entry["args"][0], "-y");
    }

    // ── Reader dotted key support ──

    #[test]
    fn test_read_json_servers_amp_dotted_key() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(f, r#"{{
            "amp": {{
                "mcpServers": {{
                    "my-server": {{ "command": "npx", "args": ["-y", "pkg"] }}
                }}
            }}
        }}"#).unwrap();

        let servers = crate::config::reader::tests::read_json_servers_pub(
            f.path(), "amp.mcpServers"
        );
        assert_eq!(servers.len(), 1);
        assert_eq!(servers["my-server"]["command"], "npx");
    }
}
