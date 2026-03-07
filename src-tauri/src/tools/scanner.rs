use super::definitions::{DetectedTool, TOOL_DEFINITIONS};
use std::path::PathBuf;

/// Common binary locations that GUI apps on macOS won't have in PATH
fn extra_bin_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".local/bin"));
        paths.push(home.join(".cargo/bin"));
        paths.push(home.join("bin"));
    }
    paths.push(PathBuf::from("/usr/local/bin"));
    paths.push(PathBuf::from("/opt/homebrew/bin"));
    paths
}

/// Check if a CLI command exists, including common user paths that
/// macOS GUI apps don't inherit from the shell environment.
fn find_cli_command(cmd: &str) -> bool {
    // First try the standard PATH
    if which::which(cmd).is_ok() {
        return true;
    }
    // Then check common user binary locations
    for dir in extra_bin_paths() {
        if dir.join(cmd).exists() {
            return true;
        }
    }
    false
}

pub fn scan_all_tools() -> Vec<DetectedTool> {
    TOOL_DEFINITIONS
        .iter()
        .map(|def| {
            let detected = if def.is_cli_only {
                def.cli_command
                    .map(|cmd| find_cli_command(cmd))
                    .unwrap_or(false)
            } else if let Some(detection_path) = def.detection_path() {
                detection_path.exists()
            } else {
                false
            };

            let config_path = def
                .config_path()
                .map(|p| p.to_string_lossy().to_string());

            DetectedTool {
                id: def.id.to_string(),
                display_name: def.display_name.to_string(),
                short_name: def.short_name.to_string(),
                config_path,
                detected,
                config_format: def.config_format.clone(),
                servers_key: def.servers_key.to_string(),
                needs_type_field: def.needs_type_field,
                remote_url_key: def.remote_url_key.map(|s| s.to_string()),
                is_cli_only: def.is_cli_only,
                cli_command: def.cli_command.map(|s| s.to_string()),
            }
        })
        .collect()
}
