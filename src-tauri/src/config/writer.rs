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

    match def.config_format {
        ConfigFormat::Json => install_json(def, config),
        ConfigFormat::Toml => install_toml(def, config),
        ConfigFormat::Cli => install_cli(def, config),
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
    let mut entry = serde_json::Map::new();

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

    if !config.env.is_empty() {
        let env_obj: serde_json::Map<String, JsonValue> = config
            .env
            .iter()
            .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
            .collect();
        entry.insert("env".to_string(), JsonValue::Object(env_obj));
    }

    JsonValue::Object(entry)
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

    // Ensure the servers key exists
    let root_obj = root
        .as_object_mut()
        .ok_or("Config root is not a JSON object")?;

    if !root_obj.contains_key(def.servers_key) {
        root_obj.insert(
            def.servers_key.to_string(),
            JsonValue::Object(serde_json::Map::new()),
        );
    }

    let servers = root_obj
        .get_mut(def.servers_key)
        .and_then(|v| v.as_object_mut())
        .ok_or("Servers key is not an object")?;

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
    entry.insert("command", toml_edit::value(&config.command));

    let mut args_arr = toml_edit::Array::new();
    for arg in &config.args {
        args_arr.push(arg.as_str());
    }
    entry.insert("args", toml_edit::value(args_arr));

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

    match def.config_format {
        ConfigFormat::Json => {
            let entry: JsonValue = serde_json::from_str(config_json)
                .map_err(|e| format!("Invalid config JSON: {}", e))?;
            restore_json(def, server_name, entry)
        }
        ConfigFormat::Toml => {
            restore_toml(def, server_name, config_json)
        }
        ConfigFormat::Cli => {
            restore_cli(def, server_name, config_json)
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

    let root_obj = root
        .as_object_mut()
        .ok_or("Config root is not a JSON object")?;

    if !root_obj.contains_key(def.servers_key) {
        root_obj.insert(
            def.servers_key.to_string(),
            JsonValue::Object(serde_json::Map::new()),
        );
    }

    let servers = root_obj
        .get_mut(def.servers_key)
        .and_then(|v| v.as_object_mut())
        .ok_or("Servers key is not an object")?;

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

    if let Some(servers) = root
        .get_mut(def.servers_key)
        .and_then(|v| v.as_object_mut())
    {
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
