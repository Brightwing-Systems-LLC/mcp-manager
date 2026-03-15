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

export interface ConfiguredServer {
  tool_id: string;
  tool_short_name: string;
  server_name: string;
  config_json: string | null;
  is_cli_only: boolean;
}

export interface DisabledServer {
  id: number;
  tool_id: string;
  server_name: string;
  config_json: string;
  disabled_at: string;
}

export interface ServerInstallConfig {
  server_name: string;
  config_key: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  transport: string;
  url: string;
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

// PatchworkMCP Scoreboard API types
export interface ScoreboardServer {
  id: string;
  name: string;
  description: string;
  language: string;
  repo_url: string;
  current_score: number | null;
  current_grade: string | null;
  score_type: string;
  visibility_level: string;
  stars_count: number;
  is_remote: boolean;
  updated_at: string;
}

export interface ScoreboardSearchResponse {
  meta: {
    page: number;
    per_page: number;
    total: number;
    total_pages: number;
  };
  results: ScoreboardServer[];
}

export interface EnvSchemaEntry {
  required: boolean;
  sensitive: boolean;
  description: string;
  default?: string;
}

export interface ApiInstallConfig {
  server_id: string;
  server_name: string;
  command: string;
  args: string[];
  env_schema: Record<string, EnvSchemaEntry>;
  config_key: string;
  transport: string;
  remote_url: string | null;
  compatibility: Record<string, boolean>;
  install_notes: string;
  source: string;
  verified: boolean;
}

// --- Auth Probe Types ---

export interface AuthProbeResult {
  auth_type: "oauth" | "api_key" | "none" | "unknown";
  server_reachable: boolean;
  error_message: string | null;
  has_oauth_metadata: boolean;
}

// --- Proxy Server Types ---

export interface ProxyServer {
  server_id: string;
  display_name: string;
  auth_type: "oauth" | "api_key" | "none";
  upstream_url: string | null;
  upstream_command: string | null;
  upstream_args: string | null;
  api_key_injection: string | null;
  created_at: string;
  updated_at: string;
}

export interface ApiKeyTestResult {
  success: boolean;
  injection_method: string | null;
  error_message: string | null;
}

export interface ToolFilterEntry {
  tool_name: string;
  enabled: boolean;
  token_estimate: number;
  tool_id: string;
}

export interface CachedTool {
  tool_name: string;
  description: string;
  input_schema: string;
  token_estimate: number;
  cached_at: string;
}

export interface ProxyApiKey {
  server_id: string;
  env: Record<string, string>;
  updated_at: string;
}

// --- OAuth Types ---

export interface OAuthFlowInfo {
  auth_url: string;
  state: string;
}

export interface OAuthStatus {
  status: "disconnected" | "connected" | "expired" | "error";
  expires_at: string | null;
  error_message: string | null;
}

// --- Proxy Log Types ---

export interface ProxyLogEvent {
  timestamp: string;
  event_type: "connect" | "request" | "response" | "error" | "session" | "disconnect";
  server_id: string;
  client_name: string | null;
  method: string | null;
  status: string | null;
  error_message: string | null;
  detail: string | null;
}

// --- Daemon Types ---

export interface DaemonStatusInfo {
  running: boolean;
  pid: number | null;
  uptime_secs: number | null;
  daemon_version: string | null;
}

// --- Governance Types ---

export interface GovernanceStatus {
  enabled: boolean;
  has_admin_pin: boolean;
  allowlist_count: number;
  pending_requests: number;
  policy_enforced: boolean;
  policy_org: string | null;
}

export interface GovernanceAllowlistEntry {
  id: number;
  server_identifier: string;
  display_name: string;
  description: string | null;
  approved_by: string;
  approved_at: string;
  review_notes: string | null;
  max_version: string | null;
}

export interface GovernanceRequest {
  id: number;
  server_identifier: string;
  server_name: string;
  requested_by: string;
  request_reason: string | null;
  status: "pending" | "approved" | "denied";
  reviewed_by: string | null;
  review_notes: string | null;
  requested_at: string;
  reviewed_at: string | null;
}

export interface GovernanceAuditEntry {
  id: number;
  action: string;
  actor: string;
  target_server: string | null;
  detail: string | null;
  timestamp: string;
}

export type View = "dashboard" | "search" | "favorites" | "install" | "add-server" | "proxy" | "api-keys" | "cli" | "about" | "server-detail" | "governance";
