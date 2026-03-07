use crate::tools::definitions::TOOL_DEFINITIONS;
use std::fs;

/// Create a backup of a tool's config file before modifying it.
/// Returns the backup content.
pub fn backup_config(tool_id: &str) -> Result<String, String> {
    let def = TOOL_DEFINITIONS
        .iter()
        .find(|d| d.id == tool_id)
        .ok_or_else(|| format!("Unknown tool: {}", tool_id))?;

    let config_path = def
        .config_path()
        .ok_or_else(|| format!("No config path for {}", tool_id))?;

    if !config_path.exists() {
        return Ok(String::new());
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config for backup: {}", e))?;

    // Also write a .brightwing-backup file next to the config
    let backup_path = config_path.with_extension("brightwing-backup");
    if !backup_path.exists() {
        fs::write(&backup_path, &content)
            .map_err(|e| format!("Failed to write backup file: {}", e))?;
    }

    Ok(content)
}
