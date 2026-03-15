//! Brightwing Auth Daemon — serves credentials and tool metadata to proxy and CLI shim instances.
//!
//! Listens on a Unix domain socket (macOS/Linux) or named pipe (Windows) and handles:
//! - Version handshake
//! - Credential retrieval from SQLite (shared with the Tauri GUI)
//! - Tool filter queries
//! - Server/tool listing for the `bw` CLI shim
//! - Ping/Pong health checks
//! - PID file management and graceful shutdown
//! - Background token refresh scheduler

use proxy_common::ipc::{
    ClientType, HandshakeRequest, IpcRequest, IpcResponse,
    decode_message, encode_message,
};
use proxy_common::credentials::{
    Credential, OAuthCredential, ApiKeyCredential,
    CredentialError, CredentialErrorCode,
};
use proxy_common::{IPC_PROTOCOL_VERSION, versions_compatible};

use brightwing_mcp_manager_lib::db::Database;
use brightwing_mcp_manager_lib::oauth::types::OAuthTokenSet;
use brightwing_mcp_manager_lib::oauth::refresh::refresh_token;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use proxy_common::transport::{IpcListener, IpcServerStream};

/// Daemon state shared across all client connections.
struct DaemonState {
    db: Arc<Database>,
    vault: Option<Arc<dyn proxy_common::vault::VaultBackend>>,
    version: String,
    started_at: Instant,
    log_buffers: std::sync::Mutex<std::collections::HashMap<String, std::collections::VecDeque<proxy_common::ipc::ProxyLogEvent>>>,
}

const MAX_LOG_EVENTS_PER_SERVER: usize = 200;

impl DaemonState {
    fn new(db: Arc<Database>, vault: Option<Arc<dyn proxy_common::vault::VaultBackend>>) -> Self {
        Self {
            db,
            vault,
            version: IPC_PROTOCOL_VERSION.to_string(),
            started_at: Instant::now(),
            log_buffers: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Handle a single IPC request and produce a response.
    async fn handle_request(&self, request: IpcRequest) -> IpcResponse {
        match request {
            IpcRequest::Handshake(hs) => self.handle_handshake(hs),

            IpcRequest::GetCredentials { server_id } => {
                self.handle_get_credentials(&server_id).await
            }

            IpcRequest::StoreCredentials { server_id, credential } => {
                self.handle_store_credentials(&server_id, &credential).await
            }

            IpcRequest::DeleteCredentials { server_id } => {
                self.handle_delete_credentials(&server_id).await
            }

            IpcRequest::GetToolFilter { server_id, tool_id } => {
                let tid = tool_id.as_deref().unwrap_or("_all");
                self.handle_get_tool_filter(&server_id, tid)
            }

            IpcRequest::SetToolFilter { server_id, tool_name, enabled, tool_id } => {
                let tid = tool_id.as_deref().unwrap_or("_all");
                self.handle_set_tool_filter(&server_id, tid, &tool_name, enabled)
            }

            IpcRequest::SetToolFilterBulk { server_id, enabled_tools, tool_id } => {
                let tid = tool_id.as_deref().unwrap_or("_all");
                self.handle_set_tool_filter_bulk(&server_id, tid, &enabled_tools)
            }

            IpcRequest::RegisterServer { server_id, display_name, auth_type, upstream_url } => {
                self.handle_register_server(&server_id, &display_name, &auth_type, upstream_url.as_deref())
            }

            IpcRequest::UnregisterServer { server_id } => {
                self.handle_unregister_server(&server_id)
            }

            IpcRequest::ListServers => {
                self.handle_list_servers()
            }

            IpcRequest::ListTools { server_id } => {
                self.handle_list_tools(&server_id)
            }

            IpcRequest::GetToolSchema { server_id, tool_name } => {
                self.handle_get_tool_schema(&server_id, &tool_name)
            }

            IpcRequest::CallTool { server_id, tool_name, arguments } => {
                self.handle_call_tool(&server_id, &tool_name, &arguments).await
            }

            IpcRequest::SubmitLog { event } => {
                let mut buffers = self.log_buffers.lock().unwrap();
                let buf = buffers.entry(event.server_id.clone()).or_default();
                buf.push_back(event);
                while buf.len() > MAX_LOG_EVENTS_PER_SERVER {
                    buf.pop_front();
                }
                IpcResponse::Ok { message: None }
            }

            IpcRequest::GetProxyLogs { server_id } => {
                let buffers = self.log_buffers.lock().unwrap();
                let events = buffers
                    .get(&server_id)
                    .map(|buf| buf.iter().cloned().collect())
                    .unwrap_or_default();
                IpcResponse::ProxyLogs { server_id, events }
            }

            IpcRequest::Ping => {
                let uptime = self.started_at.elapsed().as_secs();
                IpcResponse::Pong {
                    uptime_secs: uptime,
                    daemon_version: self.version.clone(),
                }
            }
        }
    }

    fn handle_handshake(&self, hs: HandshakeRequest) -> IpcResponse {
        if versions_compatible(&hs.version, &self.version) {
            IpcResponse::HandshakeOk {
                daemon_version: self.version.clone(),
            }
        } else {
            IpcResponse::HandshakeError {
                daemon_version: self.version.clone(),
                min_client_version: self.version.clone(),
                message: format!(
                    "Brightwing {} v{} is incompatible with daemon v{}. \
                     Please update binaries from the Brightwing app.",
                    match hs.client {
                        ClientType::Proxy => "proxy",
                        ClientType::CliShim => "bw",
                        ClientType::Gui => "gui",
                    },
                    hs.version,
                    self.version,
                ),
            }
        }
    }

    /// Try to read an API key from the encrypted vault.
    async fn try_get_api_key_from_vault(&self, server_id: &str) -> Option<std::collections::HashMap<String, String>> {
        let vault = self.vault.as_ref()?;
        let vault_key = format!("apikey:{}", server_id);
        match vault.retrieve(&vault_key).await {
            Ok(Some(data)) => {
                serde_json::from_slice::<std::collections::HashMap<String, String>>(&data).ok()
            }
            _ => None,
        }
    }

    /// Try to read an OAuth token set from the encrypted vault.
    async fn try_get_oauth_from_vault(&self, server_id: &str) -> Option<OAuthTokenSet> {
        let vault = self.vault.as_ref()?;
        let vault_key = format!("oauth:{}", server_id);
        match vault.retrieve(&vault_key).await {
            Ok(Some(data)) => serde_json::from_slice::<OAuthTokenSet>(&data).ok(),
            _ => None,
        }
    }

    /// Store an OAuth token set in the vault and update SQLite metadata.
    async fn store_oauth_token_set(&self, server_id: &str, token_set: &OAuthTokenSet) {
        // Store in vault
        if let Some(vault) = self.vault.as_ref() {
            let vault_key = format!("oauth:{}", server_id);
            if let Ok(json) = serde_json::to_vec(token_set) {
                let _ = vault.store(&vault_key, &json).await;
            }
        }

        // Update SQLite with metadata only
        let meta = brightwing_mcp_manager_lib::oauth::types::OAuthTokenMeta::from(token_set);
        if let Ok(meta_json) = serde_json::to_string(&meta) {
            let _ = self.db.store_oauth_token_set(server_id, &meta_json);
        }
    }

    async fn handle_get_credentials(&self, server_id: &str) -> IpcResponse {
        // Look up proxy server to determine auth type
        let server = match self.db.get_proxy_server(server_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                return IpcResponse::CredentialError {
                    server_id: server_id.to_string(),
                    error: CredentialError {
                        code: CredentialErrorCode::NotFound,
                        message: format!("Server '{}' not registered in Brightwing", server_id),
                    },
                };
            }
            Err(e) => {
                return IpcResponse::CredentialError {
                    server_id: server_id.to_string(),
                    error: CredentialError {
                        code: CredentialErrorCode::Internal,
                        message: format!("Database error: {}", e),
                    },
                };
            }
        };

        let credential = match server.auth_type.as_str() {
            "oauth" => {
                // Try vault first, fall back to SQLite for legacy data
                let ts = match self.try_get_oauth_from_vault(server_id).await {
                    Some(ts) => ts,
                    None => {
                        // Fallback: try SQLite (pre-migration data)
                        match self.db.get_oauth_token_set(server_id) {
                            Ok(Some(token_json)) => {
                                match serde_json::from_str::<OAuthTokenSet>(&token_json) {
                                    Ok(ts) => ts,
                                    Err(_) => {
                                        return IpcResponse::CredentialError {
                                            server_id: server_id.to_string(),
                                            error: CredentialError {
                                                code: CredentialErrorCode::AuthExpired,
                                                message: "No OAuth tokens found. Please authenticate in Brightwing.".to_string(),
                                            },
                                        };
                                    }
                                }
                            }
                            _ => {
                                return IpcResponse::CredentialError {
                                    server_id: server_id.to_string(),
                                    error: CredentialError {
                                        code: CredentialErrorCode::AuthExpired,
                                        message: "No OAuth tokens found. Please authenticate in Brightwing.".to_string(),
                                    },
                                };
                            }
                        }
                    }
                };

                // Check if token is expired
                if let Some(ref expires_at) = ts.expires_at {
                    if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                        if exp < chrono::Utc::now() {
                            return IpcResponse::CredentialError {
                                server_id: server_id.to_string(),
                                error: CredentialError {
                                    code: CredentialErrorCode::AuthExpired,
                                    message: "OAuth token expired. Please re-authenticate in Brightwing.".to_string(),
                                },
                            };
                        }
                    }
                }
                Credential::OAuth(OAuthCredential {
                    access_token: ts.access_token,
                    url: server.upstream_url.unwrap_or(ts.server_url),
                    expires_at: ts.expires_at,
                })
            }
            "api_key" => {
                // Try vault first, fall back to SQLite
                let api_key_env = self.try_get_api_key_from_vault(server_id).await
                    .or_else(|| {
                        self.db.get_api_key(server_id).ok().flatten().map(|k| k.env)
                    });

                match api_key_env {
                    Some(env) => {
                        Credential::ApiKey(ApiKeyCredential {
                            env,
                            command: server.upstream_command.clone(),
                            args: server.upstream_args.as_ref().map(|a| {
                                a.split_whitespace().map(String::from).collect()
                            }),
                            url: server.upstream_url.clone(),
                            api_key_injection: server.api_key_injection.clone(),
                        })
                    }
                    None => {
                        return IpcResponse::CredentialError {
                            server_id: server_id.to_string(),
                            error: CredentialError {
                                code: CredentialErrorCode::NotFound,
                                message: "No API key stored. Please add one in Brightwing.".to_string(),
                            },
                        };
                    }
                }
            }
            "none" => Credential::None,
            other => {
                return IpcResponse::CredentialError {
                    server_id: server_id.to_string(),
                    error: CredentialError {
                        code: CredentialErrorCode::Internal,
                        message: format!("Unknown auth type: {}", other),
                    },
                };
            }
        };

        IpcResponse::Credentials {
            server_id: server_id.to_string(),
            credential,
        }
    }

    async fn handle_store_credentials(&self, server_id: &str, credential: &Credential) -> IpcResponse {
        match credential {
            Credential::OAuth(oauth) => {
                // Build a minimal OAuthTokenSet from the IPC credential
                let token_set = OAuthTokenSet {
                    access_token: oauth.access_token.clone(),
                    refresh_token: None,
                    token_type: "Bearer".to_string(),
                    expires_in: None,
                    expires_at: oauth.expires_at.clone(),
                    scope: None,
                    token_endpoint: String::new(),
                    client_id: String::new(),
                    client_secret: None,
                    server_url: oauth.url.clone(),
                };
                self.store_oauth_token_set(server_id, &token_set).await;
                IpcResponse::Ok {
                    message: Some(format!("OAuth credentials stored for '{}'", server_id)),
                }
            }
            Credential::ApiKey(api_key) => {
                match self.db.store_api_key(server_id, &api_key.env) {
                    Ok(()) => IpcResponse::Ok {
                        message: Some(format!("API key stored for '{}'", server_id)),
                    },
                    Err(e) => IpcResponse::Error {
                        code: "db_error".to_string(),
                        message: format!("Failed to store credentials: {}", e),
                    },
                }
            }
            Credential::None => {
                IpcResponse::Ok {
                    message: Some(format!("No credentials to store for '{}'", server_id)),
                }
            }
        }
    }

    async fn handle_delete_credentials(&self, server_id: &str) -> IpcResponse {
        // Delete from vault
        if let Some(vault) = self.vault.as_ref() {
            let _ = vault.delete(&format!("oauth:{}", server_id)).await;
            let _ = vault.delete(&format!("apikey:{}", server_id)).await;
        }
        // Delete from SQLite
        let _ = self.db.delete_oauth_token_set(server_id);
        let _ = self.db.delete_api_key(server_id);
        IpcResponse::Ok {
            message: Some(format!("Credentials deleted for '{}'", server_id)),
        }
    }

    fn handle_get_tool_filter(&self, server_id: &str, tool_id: &str) -> IpcResponse {
        match self.db.get_tool_filter(server_id, tool_id) {
            Ok(entries) => {
                let enabled_tools: Vec<String> = entries
                    .iter()
                    .filter(|e| e.enabled)
                    .map(|e| e.tool_name.clone())
                    .collect();
                let total_tools = entries.len() as u32;
                let token_filtered: u32 = entries.iter().filter(|e| e.enabled).map(|e| e.token_estimate as u32).sum();
                let token_full: u32 = entries.iter().map(|e| e.token_estimate as u32).sum();
                IpcResponse::ToolFilter {
                    server_id: server_id.to_string(),
                    enabled_tools,
                    total_tools,
                    token_estimate_filtered: token_filtered,
                    token_estimate_full: token_full,
                }
            }
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to retrieve tool filter: {}", e),
            },
        }
    }

    fn handle_set_tool_filter(&self, server_id: &str, tool_id: &str, tool_name: &str, enabled: bool) -> IpcResponse {
        match self.db.set_tool_filter(server_id, tool_id, tool_name, enabled, 0) {
            Ok(()) => IpcResponse::Ok { message: None },
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to update tool filter: {}", e),
            },
        }
    }

    fn handle_set_tool_filter_bulk(&self, server_id: &str, tool_id: &str, enabled_tools: &[String]) -> IpcResponse {
        match self.db.set_tool_filter_bulk(server_id, tool_id, enabled_tools) {
            Ok(()) => IpcResponse::Ok { message: None },
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to update tool filter: {}", e),
            },
        }
    }

    fn handle_register_server(
        &self,
        server_id: &str,
        display_name: &str,
        auth_type: &str,
        upstream_url: Option<&str>,
    ) -> IpcResponse {
        match self.db.register_proxy_server(server_id, display_name, auth_type, upstream_url, None, None, None) {
            Ok(()) => IpcResponse::Ok {
                message: Some(format!("Server '{}' registered", server_id)),
            },
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to register server: {}", e),
            },
        }
    }

    fn handle_unregister_server(&self, server_id: &str) -> IpcResponse {
        match self.db.unregister_proxy_server(server_id) {
            Ok(()) => IpcResponse::Ok {
                message: Some(format!("Server '{}' unregistered", server_id)),
            },
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to unregister server: {}", e),
            },
        }
    }

    fn handle_list_servers(&self) -> IpcResponse {
        match self.db.get_proxy_servers() {
            Ok(servers) => {
                let infos: Vec<proxy_common::ipc::ServerInfo> = servers.into_iter().map(|s| {
                    let filter = self.db.get_tool_filter(&s.server_id, "_all").unwrap_or_default();
                    let enabled: Vec<_> = filter.iter().filter(|e| e.enabled).collect();
                    let token_estimate: u32 = enabled.iter().map(|e| e.token_estimate).sum();

                    // Determine auth status
                    let auth_status = match s.auth_type.as_str() {
                        "oauth" => {
                            match self.db.get_oauth_token_set(&s.server_id) {
                                Ok(Some(json)) => {
                                    // Parse as generic JSON to extract expires_at (works for both metadata and full token set)
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
                                        let expired = val.get("expires_at")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                                            .map_or(false, |dt| dt < chrono::Utc::now());
                                        if expired { "expired".to_string() } else { "connected".to_string() }
                                    } else {
                                        "pending".to_string()
                                    }
                                }
                                _ => "pending".to_string(),
                            }
                        }
                        "api_key" => {
                            match self.db.get_api_key(&s.server_id) {
                                Ok(Some(_)) => "connected".to_string(),
                                _ => "pending".to_string(),
                            }
                        }
                        "none" => "connected".to_string(),
                        _ => "unknown".to_string(),
                    };

                    proxy_common::ipc::ServerInfo {
                        server_id: s.server_id,
                        display_name: s.display_name,
                        tool_count: filter.len() as u32,
                        auth_type: s.auth_type,
                        auth_status,
                        token_estimate,
                    }
                }).collect();
                IpcResponse::ServerList { servers: infos }
            }
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to list servers: {}", e),
            },
        }
    }

    fn handle_list_tools(&self, server_id: &str) -> IpcResponse {
        match self.db.get_cached_tools(server_id) {
            Ok(cached) => {
                let tools: Vec<proxy_common::ipc::ToolInfo> = cached.into_iter().map(|ct| {
                    let schema: serde_json::Value = serde_json::from_str(&ct.input_schema).unwrap_or_default();
                    let parameters = extract_parameters(&schema);
                    proxy_common::ipc::ToolInfo {
                        name: ct.tool_name,
                        description: ct.description,
                        parameters,
                    }
                }).collect();
                IpcResponse::ToolList {
                    server_id: server_id.to_string(),
                    tools,
                }
            }
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to list tools: {}", e),
            },
        }
    }

    fn handle_get_tool_schema(&self, server_id: &str, tool_name: &str) -> IpcResponse {
        match self.db.get_cached_tools(server_id) {
            Ok(cached) => {
                if let Some(ct) = cached.iter().find(|t| t.tool_name == tool_name) {
                    let schema: serde_json::Value = serde_json::from_str(&ct.input_schema).unwrap_or_default();
                    let parameters = extract_parameters(&schema);
                    IpcResponse::ToolSchema {
                        name: ct.tool_name.clone(),
                        description: ct.description.clone(),
                        parameters,
                    }
                } else {
                    IpcResponse::Error {
                        code: "not_found".to_string(),
                        message: format!("Tool '{}/{}' not found in cache", server_id, tool_name),
                    }
                }
            }
            Err(e) => IpcResponse::Error {
                code: "db_error".to_string(),
                message: format!("Failed to get tool schema: {}", e),
            },
        }
    }

    async fn handle_call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> IpcResponse {
        let start = Instant::now();

        // Get server info
        let server = match self.db.get_proxy_server(server_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                return IpcResponse::Error {
                    code: "not_found".to_string(),
                    message: format!("Server '{}' not registered", server_id),
                };
            }
            Err(e) => {
                return IpcResponse::Error {
                    code: "db_error".to_string(),
                    message: format!("Database error: {}", e),
                };
            }
        };

        let upstream_url = match &server.upstream_url {
            Some(url) => url.clone(),
            None => {
                return IpcResponse::Error {
                    code: "not_supported".to_string(),
                    message: "call_tool only supports HTTP upstream servers".to_string(),
                };
            }
        };

        // Build auth header
        let auth_header = match server.auth_type.as_str() {
            "oauth" => {
                self.try_get_oauth_from_vault(server_id)
                    .await
                    .map(|ts| format!("Bearer {}", ts.access_token))
                    .or_else(|| {
                        // Fallback to SQLite for legacy data
                        self.db.get_oauth_token_set(server_id).ok().flatten()
                            .and_then(|json| serde_json::from_str::<OAuthTokenSet>(&json).ok())
                            .map(|ts| format!("Bearer {}", ts.access_token))
                    })
            }
            "api_key" => {
                let injection = server.api_key_injection.as_deref().unwrap_or("bearer");
                if injection.starts_with("query_param:") {
                    // Key will be appended to URL as query param, not as header
                    None
                } else {
                    let env = self.try_get_api_key_from_vault(server_id).await
                        .or_else(|| self.db.get_api_key(server_id).ok().flatten().map(|k| k.env));
                    env.and_then(|e| e.values().next().map(|v| format!("Bearer {}", v)))
                }
            }
            _ => None,
        };

        // For query_param injection, append API key to the upstream URL
        let effective_url = if server.auth_type == "api_key" {
            if let Some(injection) = &server.api_key_injection {
                if let Some(param_name) = injection.strip_prefix("query_param:") {
                    let env = self.try_get_api_key_from_vault(server_id).await
                        .or_else(|| self.db.get_api_key(server_id).ok().flatten().map(|k| k.env));
                    if let Some(key_value) = env.and_then(|e| e.values().next().cloned()) {
                        let sep = if upstream_url.contains('?') { "&" } else { "?" };
                        format!("{}{}{}={}", upstream_url, sep, param_name, urlencoding::encode(&key_value))
                    } else {
                        upstream_url.clone()
                    }
                } else {
                    upstream_url.clone()
                }
            } else {
                upstream_url.clone()
            }
        } else {
            upstream_url.clone()
        };

        // Send tools/call via JSON-RPC
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let client = reqwest::Client::new();
        let mut req = client.post(&effective_url).json(&rpc_request);
        if let Some(auth) = &auth_header {
            req = req.header("Authorization", auth.as_str());
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                return IpcResponse::ToolResult {
                    content: vec![proxy_common::ipc::ContentBlock {
                        content_type: "text".to_string(),
                        text: format!("Upstream request failed: {}", e),
                    }],
                    is_error: true,
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                };
            }
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                return IpcResponse::ToolResult {
                    content: vec![proxy_common::ipc::ContentBlock {
                        content_type: "text".to_string(),
                        text: format!("Failed to parse response: {}", e),
                    }],
                    is_error: true,
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                };
            }
        };

        let latency_ms = start.elapsed().as_millis() as u64;

        // Check for JSON-RPC error
        if let Some(error) = body.get("error") {
            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return IpcResponse::ToolResult {
                content: vec![proxy_common::ipc::ContentBlock {
                    content_type: "text".to_string(),
                    text: msg.to_string(),
                }],
                is_error: true,
                latency_ms: Some(latency_ms),
            };
        }

        // Parse result.content
        let result = body.get("result").cloned().unwrap_or_default();
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        let content_blocks = result
            .get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|block| {
                        proxy_common::ipc::ContentBlock {
                            content_type: block.get("type").and_then(|t| t.as_str()).unwrap_or("text").to_string(),
                            text: block.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| {
                // Fallback: wrap the entire result as text
                vec![proxy_common::ipc::ContentBlock {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&result).unwrap_or_default(),
                }]
            });

        IpcResponse::ToolResult {
            content: content_blocks,
            is_error,
            latency_ms: Some(latency_ms),
        }
    }
}

// ─── Parameter extraction ────────────────────────────────────────────────────

/// Extract parameter info from a JSON Schema object.
fn extract_parameters(schema: &serde_json::Value) -> Vec<proxy_common::ipc::ParameterInfo> {
    let mut params = Vec::new();
    let required: Vec<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in properties {
            let param_type = prop
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("string")
                .to_string();
            let description = prop
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let default = prop
                .get("default")
                .map(|d| d.to_string());

            params.push(proxy_common::ipc::ParameterInfo {
                name: name.clone(),
                param_type,
                description,
                required: required.contains(name),
                default,
            });
        }
    }

    // Sort: required first, then alphabetical
    params.sort_by(|a, b| {
        b.required.cmp(&a.required).then(a.name.cmp(&b.name))
    });

    params
}

// ─── Token Refresh Scheduler ─────────────────────────────────────────────────

/// Check if an OAuth token expires within the given threshold.
fn is_expiring_soon(token_set: &OAuthTokenSet, threshold_minutes: i64) -> bool {
    match &token_set.expires_at {
        Some(expires_at_str) => {
            if let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(expires_at_str) {
                let threshold = chrono::Utc::now() + chrono::Duration::minutes(threshold_minutes);
                expires_at < threshold
            } else {
                false
            }
        }
        None => false,
    }
}

/// Refresh all OAuth tokens that are expiring soon.
async fn refresh_expiring_tokens(state: &DaemonState) {
    let servers = match state.db.get_proxy_servers() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("brightwing-authd: refresh scheduler: failed to list servers: {}", e);
            return;
        }
    };

    for server in servers {
        if server.auth_type != "oauth" {
            continue;
        }

        // Read full token set from vault (or fall back to SQLite for legacy data)
        let token_set = match state.try_get_oauth_from_vault(&server.server_id).await {
            Some(ts) => ts,
            None => {
                // Fallback to SQLite
                let token_json = match state.db.get_oauth_token_set(&server.server_id) {
                    Ok(Some(json)) => json,
                    _ => continue,
                };
                match serde_json::from_str::<OAuthTokenSet>(&token_json) {
                    Ok(ts) => ts,
                    Err(_) => continue,
                }
            }
        };

        // Skip if no refresh token
        if token_set.refresh_token.is_none() {
            continue;
        }

        // Refresh if expiring within 10 minutes
        if !is_expiring_soon(&token_set, 10) {
            continue;
        }

        eprintln!("brightwing-authd: refreshing token for {}", server.server_id);

        match refresh_token(&token_set).await {
            Ok(new_set) => {
                state.store_oauth_token_set(&server.server_id, &new_set).await;
                eprintln!("brightwing-authd: refreshed token for {}", server.server_id);
            }
            Err(e) => {
                eprintln!("brightwing-authd: failed to refresh token for {}: {}", server.server_id, e);
            }
        }
    }
}

/// Background task that periodically checks and refreshes expiring tokens.
async fn token_refresh_loop(state: Arc<DaemonState>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
    loop {
        interval.tick().await;
        refresh_expiring_tokens(&state).await;
    }
}

// ─── Daemon Lifecycle ────────────────────────────────────────────────────────

fn default_data_dir() -> PathBuf {
    proxy_common::transport::default_data_dir()
}

fn default_socket_path() -> PathBuf {
    proxy_common::transport::default_socket_path()
}

/// Get the default PID file path.
fn default_pid_path() -> PathBuf {
    default_data_dir().join("authd.pid")
}

/// Write PID file.
fn write_pid_file(path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{}", std::process::id()))
}

/// Remove PID file if it contains our PID.
fn remove_pid_file(path: &PathBuf) {
    if let Ok(contents) = std::fs::read_to_string(path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            if pid == std::process::id() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Check if a daemon is already running by reading the PID file and checking the process.
fn is_daemon_running(pid_path: &PathBuf) -> Option<u32> {
    let contents = std::fs::read_to_string(pid_path).ok()?;
    let pid = contents.trim().parse::<u32>().ok()?;

    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result == 0 { Some(pid) } else { None }
    }
    #[cfg(not(unix))]
    {
        Some(pid)
    }
}

/// Handle a single client connection.
async fn handle_client(
    stream: IpcServerStream,
    state: Arc<DaemonState>,
) {
    let (mut lines, mut writer) = stream.into_split();

    while let Ok(Some(line)) = lines.next_line().await {
        let request: IpcRequest = match decode_message(line.as_bytes()) {
            Ok(req) => req,
            Err(e) => {
                let err_resp = IpcResponse::Error {
                    code: "parse_error".to_string(),
                    message: format!("Failed to parse request: {}", e),
                };
                let _ = writer.write_all(&encode_message(&err_resp).unwrap_or_default()).await;
                continue;
            }
        };

        let response = state.handle_request(request).await;
        let bytes = match encode_message(&response) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("brightwing-authd: failed to serialize response: {}", e);
                continue;
            }
        };

        if writer.write_all(&bytes).await.is_err() {
            break; // Client disconnected
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_socket_path);

    let pid_path = default_pid_path();

    // Check for already-running daemon
    if let Some(pid) = is_daemon_running(&pid_path) {
        eprintln!(
            "brightwing-authd: another instance is already running (PID {}). Exiting.",
            pid
        );
        std::process::exit(1);
    }

    // Ensure parent directory exists (not applicable for Windows named pipes)
    #[cfg(not(windows))]
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(windows)]
    {
        let data_dir = default_data_dir();
        std::fs::create_dir_all(&data_dir)?;
    }

    // Remove stale socket file (Unix only — named pipes don't leave files)
    #[cfg(not(windows))]
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Write PID file
    write_pid_file(&pid_path)?;

    // Open the shared SQLite database
    let db = Arc::new(Database::new().expect("Failed to open database"));

    // Open encrypted vault for API key retrieval
    let vault: Option<Arc<dyn proxy_common::vault::VaultBackend>> =
        match brightwing_mcp_manager_lib::vault_init::open_vault() {
            Ok(v) => {
                eprintln!("brightwing-authd: encrypted vault opened successfully");
                Some(v)
            }
            Err(e) => {
                eprintln!("brightwing-authd: WARNING: failed to open vault: {}. Using SQLite fallback.", e);
                None
            }
        };

    // Run migrations (idempotent)
    if let Some(ref v) = vault {
        brightwing_mcp_manager_lib::vault_init::migrate_api_keys_to_vault(&db, v.as_ref()).await;
        brightwing_mcp_manager_lib::vault_init::migrate_oauth_tokens_to_vault(&db, v.as_ref()).await;
    }

    let state = Arc::new(DaemonState::new(Arc::clone(&db), vault));

    let mut listener = IpcListener::bind(&socket_path)?;

    // Set socket permissions to owner-only (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    }

    eprintln!(
        "brightwing-authd v{} listening on {} (PID {})",
        IPC_PROTOCOL_VERSION,
        socket_path.display(),
        std::process::id()
    );

    // Spawn token refresh background task
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            token_refresh_loop(state).await;
        });
    }

    // Graceful shutdown on SIGTERM / ctrl-c
    let socket_path_clone = socket_path.clone();
    let pid_path_clone = pid_path.clone();

    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

            tokio::select! {
                _ = ctrl_c => {},
                _ = sigterm.recv() => {},
            }
        }

        #[cfg(not(unix))]
        {
            ctrl_c.await.ok();
        }

        eprintln!("brightwing-authd: shutting down gracefully...");
        #[cfg(not(windows))]
        let _ = std::fs::remove_file(&socket_path_clone);
        remove_pid_file(&pid_path_clone);
        std::process::exit(0);
    });

    loop {
        match listener.accept().await {
            Ok(stream) => {
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    handle_client(stream, state).await;
                });
            }
            Err(e) => {
                eprintln!("brightwing-authd: accept error: {}", e);
            }
        }
    }
}
