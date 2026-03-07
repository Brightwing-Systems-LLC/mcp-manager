use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedTool {
    pub id: String,
    pub display_name: String,
    pub short_name: String, // e.g., "CD", "CU", "WS" for compact display
    pub config_path: Option<String>,
    pub detected: bool,
    pub config_format: ConfigFormat,
    pub servers_key: String,
    pub needs_type_field: bool,    // VS Code requires "type": "stdio"
    pub remote_url_key: Option<String>, // Some tools use different keys for remote URLs
    pub is_cli_only: bool,         // Claude Code, Codex CLI - shell out instead of file edit
    pub cli_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigFormat {
    Json,
    Toml,
    Cli,
}

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub short_name: &'static str,
    pub config_format: ConfigFormat,
    pub servers_key: &'static str,
    pub needs_type_field: bool,
    pub remote_url_key: Option<&'static str>,
    pub is_cli_only: bool,
    pub cli_command: Option<&'static str>,
}

impl ToolDefinition {
    pub fn config_path(&self) -> Option<PathBuf> {
        match self.id {
            "claude_desktop" => {
                #[cfg(target_os = "macos")]
                {
                    dirs::home_dir().map(|h| {
                        h.join("Library/Application Support/Claude/claude_desktop_config.json")
                    })
                }
                #[cfg(target_os = "windows")]
                {
                    dirs::config_dir()
                        .map(|c| c.join("Claude/claude_desktop_config.json"))
                }
                #[cfg(target_os = "linux")]
                {
                    dirs::config_dir()
                        .map(|c| c.join("Claude/claude_desktop_config.json"))
                }
            }
            "cursor" => dirs::home_dir().map(|h| h.join(".cursor/mcp.json")),
            "windsurf" => {
                dirs::home_dir().map(|h| h.join(".codeium/windsurf/mcp_config.json"))
            }
            "vscode" => {
                #[cfg(target_os = "macos")]
                {
                    dirs::home_dir()
                        .map(|h| h.join("Library/Application Support/Code/User/settings.json"))
                }
                #[cfg(target_os = "windows")]
                {
                    dirs::config_dir()
                        .map(|c| c.join("Code/User/settings.json"))
                }
                #[cfg(target_os = "linux")]
                {
                    dirs::config_dir()
                        .map(|c| c.join("Code/User/settings.json"))
                }
            }
            "claude_code" => None, // CLI-only
            "codex" => dirs::home_dir().map(|h| h.join(".codex/config.toml")),
            "gemini_cli" => dirs::home_dir().map(|h| h.join(".gemini/settings.json")),
            "antigravity" => {
                dirs::home_dir().map(|h| h.join(".gemini/antigravity/mcp_config.json"))
            }
            _ => None,
        }
    }

    pub fn detection_path(&self) -> Option<PathBuf> {
        match self.id {
            "claude_desktop" => {
                #[cfg(target_os = "macos")]
                {
                    dirs::home_dir()
                        .map(|h| h.join("Library/Application Support/Claude"))
                }
                #[cfg(target_os = "windows")]
                {
                    dirs::config_dir().map(|c| c.join("Claude"))
                }
                #[cfg(target_os = "linux")]
                {
                    dirs::config_dir().map(|c| c.join("Claude"))
                }
            }
            "cursor" => dirs::home_dir().map(|h| h.join(".cursor")),
            "windsurf" => dirs::home_dir().map(|h| h.join(".codeium/windsurf")),
            "vscode" => {
                #[cfg(target_os = "macos")]
                {
                    dirs::home_dir()
                        .map(|h| h.join("Library/Application Support/Code"))
                }
                #[cfg(target_os = "windows")]
                {
                    dirs::config_dir().map(|c| c.join("Code"))
                }
                #[cfg(target_os = "linux")]
                {
                    dirs::config_dir().map(|c| c.join("Code"))
                }
            }
            "claude_code" => None, // Detected via PATH
            "codex" => dirs::home_dir().map(|h| h.join(".codex")),
            "gemini_cli" => dirs::home_dir().map(|h| h.join(".gemini")),
            "antigravity" => {
                dirs::home_dir().map(|h| h.join(".gemini/antigravity"))
            }
            _ => None,
        }
    }
}

pub static TOOL_DEFINITIONS: &[ToolDefinition] = &[
    ToolDefinition {
        id: "claude_desktop",
        display_name: "Claude Desktop",
        short_name: "CD",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
    },
    ToolDefinition {
        id: "cursor",
        display_name: "Cursor",
        short_name: "CU",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
    },
    ToolDefinition {
        id: "windsurf",
        display_name: "Windsurf",
        short_name: "WS",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
    },
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
    },
    ToolDefinition {
        id: "claude_code",
        display_name: "Claude Code",
        short_name: "CC",
        config_format: ConfigFormat::Cli,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: true,
        cli_command: Some("claude"),
    },
    ToolDefinition {
        id: "codex",
        display_name: "OpenAI Codex",
        short_name: "CX",
        config_format: ConfigFormat::Toml,
        servers_key: "mcp_servers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
    },
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
    },
    ToolDefinition {
        id: "antigravity",
        display_name: "Antigravity",
        short_name: "AG",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: Some("serverUrl"),
        is_cli_only: false,
        cli_command: None,
    },
];
