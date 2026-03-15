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
    pub uses_command_array: bool, // OpenCode: command is an array [cmd, arg1, arg2] instead of separate command/args
    pub env_key: &'static str,   // Key for environment variables ("env" for most, "environment" for OpenCode)
}

impl ToolDefinition {
    pub fn config_path(&self) -> Option<PathBuf> {
        let home = || dirs::home_dir();
        match self.id {
            "claude_desktop" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Claude/claude_desktop_config.json")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Claude/claude_desktop_config.json")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Claude/claude_desktop_config.json")) }
            }
            "cursor" => home().map(|h| h.join(".cursor/mcp.json")),
            "windsurf" => home().map(|h| h.join(".codeium/windsurf/mcp_config.json")),
            "vscode" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Code/User/settings.json")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Code/User/settings.json")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Code/User/settings.json")) }
            }
            "claude_code" => None, // CLI-only
            "codex" => home().map(|h| h.join(".codex/config.toml")),
            "gemini_cli" => home().map(|h| h.join(".gemini/settings.json")),
            "antigravity" => home().map(|h| h.join(".gemini/antigravity/mcp_config.json")),
            "opencode" => home().map(|h| h.join(".config/opencode/opencode.json")),
            "pi" => home().map(|h| h.join(".pi/agent/mcp.json")),
            "copilot_cli" => home().map(|h| h.join(".copilot/mcp-config.json")),
            "amazon_q" => home().map(|h| h.join(".aws/amazonq/mcp.json")),
            "kiro" => home().map(|h| h.join(".kiro/settings/mcp.json")),
            "lm_studio" => home().map(|h| h.join(".lmstudio/mcp.json")),
            "cline" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json")) }
            }
            "roo_code" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json")) }
            }
            "junie" => home().map(|h| h.join(".junie/mcp/mcp.json")),
            "zed" => home().map(|h| h.join(".config/zed/settings.json")),
            "amp" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join(".config/amp/settings.json")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("amp/settings.json")) }
                #[cfg(target_os = "linux")]
                { home().map(|h| h.join(".config/amp/settings.json")) }
            }
            "copilot_jetbrains" => home().map(|h| h.join(".config/github-copilot/mcp.json")),
            _ => None,
        }
    }

    pub fn detection_path(&self) -> Option<PathBuf> {
        let home = || dirs::home_dir();
        match self.id {
            "claude_desktop" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Claude")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Claude")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Claude")) }
            }
            "cursor" => home().map(|h| h.join(".cursor")),
            "windsurf" => home().map(|h| h.join(".codeium/windsurf")),
            "vscode" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Code")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Code")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Code")) }
            }
            "claude_code" => None, // Detected via PATH
            "codex" => home().map(|h| h.join(".codex")),
            "gemini_cli" => home().map(|h| h.join(".gemini")),
            "antigravity" => home().map(|h| h.join(".gemini/antigravity")),
            "opencode" => home().map(|h| h.join(".config/opencode")),
            "pi" => home().map(|h| h.join(".pi/agent")),
            "copilot_cli" => home().map(|h| h.join(".copilot")),
            "amazon_q" => home().map(|h| h.join(".aws/amazonq")),
            "kiro" => home().map(|h| h.join(".kiro")),
            "lm_studio" => home().map(|h| h.join(".lmstudio")),
            "cline" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/saoudrizwan.claude-dev")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/saoudrizwan.claude-dev")) }
            }
            "roo_code" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/rooveterinaryinc.roo-cline")) }
                #[cfg(target_os = "linux")]
                { dirs::config_dir().map(|c| c.join("Code/User/globalStorage/rooveterinaryinc.roo-cline")) }
            }
            "junie" => home().map(|h| h.join(".junie")),
            "zed" => home().map(|h| h.join(".config/zed")),
            "amp" => {
                #[cfg(target_os = "macos")]
                { home().map(|h| h.join(".config/amp")) }
                #[cfg(target_os = "windows")]
                { dirs::config_dir().map(|c| c.join("amp")) }
                #[cfg(target_os = "linux")]
                { home().map(|h| h.join(".config/amp")) }
            }
            "copilot_jetbrains" => home().map(|h| h.join(".config/github-copilot")),
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
        uses_command_array: false,
        env_key: "env",
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
        uses_command_array: false,
        env_key: "env",
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
        uses_command_array: false,
        env_key: "env",
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
        uses_command_array: false,
        env_key: "env",
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
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "codex",
        display_name: "Codex CLI / Desktop",
        short_name: "CX",
        config_format: ConfigFormat::Toml,
        servers_key: "mcp_servers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
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
        uses_command_array: false,
        env_key: "env",
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
        uses_command_array: false,
        env_key: "env",
    },
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
    },
    ToolDefinition {
        id: "pi",
        display_name: "Pi",
        short_name: "PI",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "copilot_cli",
        display_name: "GitHub Copilot CLI",
        short_name: "GH",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "amazon_q",
        display_name: "Amazon Q",
        short_name: "AQ",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "kiro",
        display_name: "Kiro",
        short_name: "KI",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "lm_studio",
        display_name: "LM Studio",
        short_name: "LM",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "cline",
        display_name: "Cline",
        short_name: "CL",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "roo_code",
        display_name: "Roo Code",
        short_name: "RC",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "junie",
        display_name: "Junie CLI",
        short_name: "JU",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
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
    },
    ToolDefinition {
        id: "amp",
        display_name: "Amp",
        short_name: "AM",
        config_format: ConfigFormat::Json,
        servers_key: "amp.mcpServers",  // dotted key — nested in {"amp": {"mcpServers": ...}}
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
    ToolDefinition {
        id: "copilot_jetbrains",
        display_name: "Copilot (JetBrains)",
        short_name: "CJ",
        config_format: ConfigFormat::Json,
        servers_key: "mcpServers",
        needs_type_field: false,
        remote_url_key: None,
        is_cli_only: false,
        cli_command: None,
        uses_command_array: false,
        env_key: "env",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_have_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for def in TOOL_DEFINITIONS {
            assert!(ids.insert(def.id), "Duplicate tool id: {}", def.id);
        }
    }

    #[test]
    fn test_all_tools_have_unique_short_names() {
        let mut names = std::collections::HashSet::new();
        for def in TOOL_DEFINITIONS {
            assert!(names.insert(def.short_name), "Duplicate short_name: {}", def.short_name);
        }
    }

    #[test]
    fn test_cli_tools_have_cli_command() {
        for def in TOOL_DEFINITIONS {
            if def.is_cli_only {
                assert!(def.cli_command.is_some(), "{} is CLI-only but has no cli_command", def.id);
            }
        }
    }

    #[test]
    fn test_non_cli_tools_have_config_path() {
        for def in TOOL_DEFINITIONS {
            if !def.is_cli_only {
                assert!(def.config_path().is_some(), "{} has no config_path", def.id);
            }
        }
    }

    #[test]
    fn test_non_cli_tools_have_detection_path() {
        for def in TOOL_DEFINITIONS {
            if !def.is_cli_only {
                assert!(def.detection_path().is_some(), "{} has no detection_path", def.id);
            }
        }
    }

    #[test]
    fn test_opencode_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "opencode").unwrap();
        assert_eq!(def.servers_key, "mcp");
        assert!(def.uses_command_array);
        assert_eq!(def.env_key, "environment");
        assert!(!def.is_cli_only);
        assert!(def.config_path().unwrap().ends_with("opencode/opencode.json"));
        assert!(def.detection_path().unwrap().ends_with("opencode"));
    }

    #[test]
    fn test_pi_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "pi").unwrap();
        assert_eq!(def.servers_key, "mcpServers");
        assert!(!def.uses_command_array);
        assert_eq!(def.env_key, "env");
        assert!(!def.is_cli_only);
        assert!(def.config_path().unwrap().ends_with("agent/mcp.json"));
        assert!(def.detection_path().unwrap().ends_with(".pi/agent"));
    }

    #[test]
    fn test_standard_tools_use_env_key() {
        for def in TOOL_DEFINITIONS {
            if def.id != "opencode" {
                assert_eq!(def.env_key, "env", "{} should use 'env' key", def.id);
            }
        }
    }

    #[test]
    fn test_only_opencode_uses_command_array() {
        for def in TOOL_DEFINITIONS {
            if def.id != "opencode" {
                assert!(!def.uses_command_array, "{} should not use command array", def.id);
            }
        }
    }

    #[test]
    fn test_tool_count() {
        // Ensure we don't accidentally lose tools
        assert_eq!(TOOL_DEFINITIONS.len(), 20);
    }

    #[test]
    fn test_copilot_cli_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "copilot_cli").unwrap();
        assert_eq!(def.servers_key, "mcpServers");
        assert!(def.config_path().unwrap().ends_with(".copilot/mcp-config.json"));
    }

    #[test]
    fn test_amazon_q_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "amazon_q").unwrap();
        assert_eq!(def.servers_key, "mcpServers");
        assert!(def.config_path().unwrap().ends_with("amazonq/mcp.json"));
    }

    #[test]
    fn test_zed_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "zed").unwrap();
        assert_eq!(def.servers_key, "context_servers");
        assert!(def.config_path().unwrap().ends_with("zed/settings.json"));
    }

    #[test]
    fn test_amp_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "amp").unwrap();
        assert_eq!(def.servers_key, "amp.mcpServers");
        assert!(def.config_path().unwrap().ends_with("amp/settings.json"));
    }

    #[test]
    fn test_cline_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "cline").unwrap();
        assert_eq!(def.servers_key, "mcpServers");
        let path = def.config_path().unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("saoudrizwan.claude-dev"));
    }

    #[test]
    fn test_roo_code_definition() {
        let def = TOOL_DEFINITIONS.iter().find(|d| d.id == "roo_code").unwrap();
        assert_eq!(def.servers_key, "mcpServers");
        let path = def.config_path().unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("rooveterinaryinc.roo-cline"));
    }
}
