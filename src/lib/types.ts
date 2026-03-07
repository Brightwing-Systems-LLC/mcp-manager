export interface DetectedTool {
  id: string;
  display_name: string;
  short_name: string;
  config_path: string | null;
  detected: boolean;
  config_format: "Json" | "Toml" | "Cli";
  servers_key: string;
  needs_type_field: boolean;
  remote_url_key: string | null;
  is_cli_only: boolean;
  cli_command: string | null;
}

export interface ServerInstallConfig {
  server_name: string;
  config_key: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  transport: string;
}

export interface InstallResult {
  success: boolean;
  message: string;
  needs_restart: boolean;
}

export interface Installation {
  id: number;
  server_uuid: string;
  server_name: string;
  tool_id: string;
  config_key: string;
  installed_at: string;
}

export interface Favorite {
  server_uuid: string;
  server_name: string;
  display_name: string | null;
  grade: string | null;
  score: number | null;
  language: string | null;
  install_config_json: string | null;
  added_at: string;
}

export interface DeepLinkAction {
  action: string;
  server_uuid: string;
  tool_id: string | null;
}

// PatchworkMCP API types
export interface ScoreboardServer {
  uuid: string;
  name: string;
  description: string;
  github_url: string;
  grade: string;
  overall_score: number;
  language: string;
  stars: number;
  categories: string[];
}

export type View = "dashboard" | "search" | "favorites" | "install" | "about";
