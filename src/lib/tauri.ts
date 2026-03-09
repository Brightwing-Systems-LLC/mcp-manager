import { invoke } from "@tauri-apps/api/core";
import type {
  DetectedTool,
  ConfiguredServer,
  DisabledServer,
  ServerInstallConfig,
  InstallResult,
  Installation,
  Favorite,
  DeepLinkAction,
  ScoreboardSearchResponse,
  ApiInstallConfig,
  ScoreboardServer,
  ProxyServer,
  ToolFilterEntry,
  CachedTool,
  ProxyApiKey,
  OAuthFlowInfo,
  OAuthStatus,
  DaemonStatusInfo,
} from "./types";

export async function scanTools(): Promise<DetectedTool[]> {
  return invoke<DetectedTool[]>("scan_tools");
}

export async function scanConfiguredServers(): Promise<ConfiguredServer[]> {
  return invoke<ConfiguredServer[]>("scan_configured_servers");
}

export async function disableServer(
  toolId: string,
  serverName: string,
  configJson: string
): Promise<InstallResult> {
  return invoke<InstallResult>("disable_server", { toolId, serverName, configJson });
}

export async function enableServer(
  toolId: string,
  serverName: string
): Promise<InstallResult> {
  return invoke<InstallResult>("enable_server", { toolId, serverName });
}

export async function getDisabledServers(): Promise<DisabledServer[]> {
  return invoke<DisabledServer[]>("get_disabled_servers");
}

export async function readToolConfig(
  toolId: string
): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("read_tool_config", { toolId });
}

export async function installServer(
  toolId: string,
  serverConfig: ServerInstallConfig
): Promise<InstallResult> {
  return invoke<InstallResult>("install_server", { toolId, serverConfig });
}

export async function uninstallServer(
  toolId: string,
  configKey: string,
  serverUuid: string
): Promise<InstallResult> {
  return invoke<InstallResult>("uninstall_server", {
    toolId,
    configKey,
    serverUuid,
  });
}

export async function getInstallations(): Promise<Installation[]> {
  return invoke<Installation[]>("get_installations");
}

export async function getFavorites(): Promise<Favorite[]> {
  return invoke<Favorite[]>("get_favorites");
}

export async function addFavorite(params: {
  serverUuid: string;
  serverName: string;
  displayName?: string;
  grade?: string;
  score?: number;
  language?: string;
  installConfigJson?: string;
}): Promise<void> {
  return invoke("add_favorite", params);
}

export async function removeFavorite(serverUuid: string): Promise<void> {
  return invoke("remove_favorite", { serverUuid });
}

export async function getPendingDeepLink(): Promise<DeepLinkAction | null> {
  return invoke<DeepLinkAction | null>("get_pending_deep_link");
}

export async function clearPendingDeepLink(): Promise<void> {
  return invoke("clear_pending_deep_link");
}

export async function backupToolConfig(toolId: string): Promise<string> {
  return invoke<string>("backup_tool_config", { toolId });
}

export async function fetchCliServerConfig(
  toolId: string,
  serverName: string
): Promise<string> {
  return invoke<string>("fetch_cli_server_config", { toolId, serverName });
}

export async function addServerToTool(
  toolId: string,
  serverName: string,
  configJson: string
): Promise<InstallResult> {
  return invoke<InstallResult>("add_server_to_tool", { toolId, serverName, configJson });
}

export async function restartTool(toolId: string): Promise<string> {
  return invoke<string>("restart_tool", { toolId });
}

// --- API Proxy (routes through Rust to bypass CORS) ---

export async function apiSearchServers(
  query: string,
  perPage?: number
): Promise<ScoreboardSearchResponse> {
  return invoke<ScoreboardSearchResponse>("api_search_servers", {
    query,
    perPage,
  });
}

export async function apiGetInstallableIds(): Promise<string[]> {
  return invoke<string[]>("api_get_installable_ids");
}

export async function apiGetInstallConfig(
  serverId: string
): Promise<ApiInstallConfig | null> {
  const result = await invoke<ApiInstallConfig | null>(
    "api_get_install_config",
    { serverId }
  );
  return result;
}

export async function apiGetServer(
  serverId: string
): Promise<ScoreboardServer> {
  return invoke<ScoreboardServer>("api_get_server", { serverId });
}

// --- Proxy Server Management ---

export async function registerProxyServer(params: {
  serverId: string;
  displayName: string;
  authType: string;
  upstreamUrl?: string;
  upstreamCommand?: string;
  upstreamArgs?: string;
}): Promise<void> {
  return invoke("register_proxy_server", params);
}

export async function unregisterProxyServer(serverId: string): Promise<void> {
  return invoke("unregister_proxy_server", { serverId });
}

export async function getProxyServers(): Promise<ProxyServer[]> {
  return invoke<ProxyServer[]>("get_proxy_servers");
}

export async function getProxyServer(serverId: string): Promise<ProxyServer | null> {
  return invoke<ProxyServer | null>("get_proxy_server", { serverId });
}

export async function installProxyToTool(
  toolId: string,
  serverId: string,
  configKey: string
): Promise<InstallResult> {
  return invoke<InstallResult>("install_proxy_to_tool", { toolId, serverId, configKey });
}

export async function uninstallProxyFromTool(
  toolId: string,
  serverId: string,
  configKey: string
): Promise<InstallResult> {
  return invoke<InstallResult>("uninstall_proxy_from_tool", { toolId, serverId, configKey });
}

export async function getProxyInstalls(serverId: string): Promise<string[]> {
  return invoke<string[]>("get_proxy_installs", { serverId });
}

export async function getToolFilter(serverId: string): Promise<ToolFilterEntry[]> {
  return invoke<ToolFilterEntry[]>("get_tool_filter", { serverId });
}

export async function setToolFilter(
  serverId: string,
  toolName: string,
  enabled: boolean,
  tokenEstimate: number
): Promise<void> {
  return invoke("set_tool_filter", { serverId, toolName, enabled, tokenEstimate });
}

export async function setToolFilterBulk(
  serverId: string,
  enabledTools: string[]
): Promise<void> {
  return invoke("set_tool_filter_bulk", { serverId, enabledTools });
}

export async function getCachedTools(serverId: string): Promise<CachedTool[]> {
  return invoke<CachedTool[]>("get_cached_tools", { serverId });
}

export async function cacheToolSchema(params: {
  serverId: string;
  toolName: string;
  description: string;
  inputSchema: string;
  tokenEstimate: number;
}): Promise<void> {
  return invoke("cache_tool_schema", params);
}

// --- API Key Management ---

export async function storeApiKey(serverId: string, env: Record<string, string>): Promise<void> {
  return invoke("store_api_key", { serverId, env });
}

export async function getApiKey(serverId: string): Promise<ProxyApiKey | null> {
  return invoke<ProxyApiKey | null>("get_api_key", { serverId });
}

export async function deleteApiKey(serverId: string): Promise<void> {
  return invoke("delete_api_key", { serverId });
}

export async function getAllApiKeys(): Promise<ProxyApiKey[]> {
  return invoke<ProxyApiKey[]>("get_all_api_keys");
}

// --- OAuth ---

export async function startOAuthFlow(
  serverId: string,
  serverUrl: string,
  clientId?: string
): Promise<OAuthFlowInfo> {
  return invoke<OAuthFlowInfo>("start_oauth_flow", { serverId, serverUrl, clientId: clientId || null });
}

export async function completeOAuthCallback(
  state: string,
  code?: string
): Promise<void> {
  return invoke("complete_oauth_callback", { state, code: code || null });
}

export async function getOAuthStatus(serverId: string): Promise<OAuthStatus> {
  return invoke<OAuthStatus>("get_oauth_status", { serverId });
}

export async function disconnectOAuth(serverId: string): Promise<void> {
  return invoke("disconnect_oauth", { serverId });
}

export async function refreshOAuthToken(serverId: string): Promise<void> {
  return invoke("refresh_oauth_token", { serverId });
}

export async function distributeBinaries(): Promise<string[]> {
  return invoke<string[]>("distribute_binaries");
}

export async function getBinaryVersions(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("get_binary_versions");
}

// --- Daemon Lifecycle ---

export async function daemonStatus(): Promise<DaemonStatusInfo> {
  return invoke<DaemonStatusInfo>("daemon_status");
}

export async function startDaemon(): Promise<DaemonStatusInfo> {
  return invoke<DaemonStatusInfo>("start_daemon");
}

export async function stopDaemon(): Promise<DaemonStatusInfo> {
  return invoke<DaemonStatusInfo>("stop_daemon");
}

export async function isAutostartEnabled(): Promise<boolean> {
  return invoke<boolean>("is_autostart_enabled");
}

export async function setAutostart(enabled: boolean): Promise<void> {
  return invoke("set_autostart", { enabled });
}
