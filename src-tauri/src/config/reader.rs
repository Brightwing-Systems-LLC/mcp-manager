use crate::tools::definitions::{ConfigFormat, TOOL_DEFINITIONS};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
