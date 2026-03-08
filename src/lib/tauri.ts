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
