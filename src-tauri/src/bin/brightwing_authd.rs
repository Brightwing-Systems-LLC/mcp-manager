//! Brightwing Auth Daemon — serves credentials and tool metadata to proxy and CLI shim instances.
//!
//! Listens on a Unix domain socket (macOS/Linux) or named pipe (Windows) and handles:
//! - Version handshake
//! - Credential retrieval from VaultBackend
//! - Tool filter queries
//! - Server/tool listing for the `bw` CLI shim
//! - Tool calls forwarded to upstream MCP servers
//! - Ping/Pong health checks
//! - PID file management and graceful shutdown

use proxy_common::ipc::{
    ClientType, HandshakeRequest, IpcRequest, IpcResponse,
    decode_message, encode_message,
};
use proxy_common::credentials::{
    Credential, CredentialError, CredentialErrorCode,
};
use proxy_common::vault::{InMemoryVaultBackend, VaultBackend};
use proxy_common::{IPC_PROTOCOL_VERSION, versions_compatible};

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

/// Daemon state shared across all client connections.
struct DaemonState {
    vault: Arc<dyn VaultBackend>,
    version: String,
    started_at: Instant,
}

impl DaemonState {
    fn new(vault: Arc<dyn VaultBackend>) -> Self {
        Self {
            vault,
            version: IPC_PROTOCOL_VERSION.to_string(),
            started_at: Instant::now(),
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

            IpcRequest::GetToolFilter { server_id } => {
                self.handle_get_tool_filter(&server_id).await
            }

            IpcRequest::SetToolFilter { server_id, tool_name, enabled } => {
                self.handle_set_tool_filter(&server_id, &tool_name, enabled).await
            }

            IpcRequest::SetToolFilterBulk { server_id, enabled_tools } => {
                self.handle_set_tool_filter_bulk(&server_id, &enabled_tools).await
            }

            IpcRequest::RegisterServer { server_id, display_name, auth_type, upstream_url } => {
                self.handle_register_server(&server_id, &display_name, &auth_type, upstream_url.as_deref()).await
            }

            IpcRequest::UnregisterServer { server_id } => {
                self.handle_unregister_server(&server_id).await
            }

            IpcRequest::ListServers => {
                self.handle_list_servers().await
            }

            IpcRequest::ListTools { server_id } => {
                // Stub: Phase 5 will populate from cached tool schemas
                IpcResponse::ToolList {
                    server_id,
                    tools: vec![],
                }
            }

            IpcRequest::GetToolSchema { server_id, tool_name } => {
                IpcResponse::Error {
                    code: "not_found".to_string(),
                    message: format!("Tool {}/{} not found", server_id, tool_name),
                }
            }

            IpcRequest::CallTool { server_id, tool_name, .. } => {
                // Stub: Phase 5 will implement upstream forwarding
                IpcResponse::Error {
                    code: "not_implemented".to_string(),
                    message: format!("call_tool for {}/{} not yet implemented", server_id, tool_name),
                }
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

    async fn handle_get_credentials(&self, server_id: &str) -> IpcResponse {
        let key = format!("credential:{}", server_id);
        match self.vault.retrieve(&key).await {
            Ok(Some(data)) => {
                match serde_json::from_slice::<Credential>(&data) {
                    Ok(credential) => IpcResponse::Credentials {
                        server_id: server_id.to_string(),
                        credential,
                    },
                    Err(e) => IpcResponse::CredentialError {
                        server_id: server_id.to_string(),
                        error: CredentialError {
                            code: CredentialErrorCode::Internal,
                            message: format!("Failed to deserialize credential: {}", e),
                        },
                    },
                }
            }
            Ok(None) => IpcResponse::CredentialError {
                server_id: server_id.to_string(),
                error: CredentialError {
                    code: CredentialErrorCode::NotFound,
                    message: format!("Server '{}' not registered in Brightwing", server_id),
                },
            },
            Err(e) => IpcResponse::CredentialError {
                server_id: server_id.to_string(),
                error: CredentialError {
                    code: CredentialErrorCode::VaultError,
                    message: format!("Vault error: {}", e),
                },
            },
        }
    }

    async fn handle_store_credentials(&self, server_id: &str, credential: &Credential) -> IpcResponse {
        let key = format!("credential:{}", server_id);
        let data = match serde_json::to_vec(credential) {
            Ok(d) => d,
            Err(e) => return IpcResponse::Error {
                code: "serialize_error".to_string(),
                message: format!("Failed to serialize credential: {}", e),
            },
        };
        match self.vault.store(&key, &data).await {
            Ok(()) => IpcResponse::Ok {
                message: Some(format!("Credentials stored for '{}'", server_id)),
            },
            Err(e) => IpcResponse::Error {
                code: "vault_error".to_string(),
                message: format!("Failed to store credentials: {}", e),
            },
        }
    }

    async fn handle_delete_credentials(&self, server_id: &str) -> IpcResponse {
        let key = format!("credential:{}", server_id);
        match self.vault.delete(&key).await {
            Ok(()) => IpcResponse::Ok {
                message: Some(format!("Credentials deleted for '{}'", server_id)),
            },
            Err(e) => IpcResponse::Error {
                code: "vault_error".to_string(),
                message: format!("Failed to delete credentials: {}", e),
            },
        }
    }

    async fn handle_get_tool_filter(&self, server_id: &str) -> IpcResponse {
        let key = format!("filter:{}", server_id);
        match self.vault.retrieve(&key).await {
            Ok(Some(data)) => {
                // Filter stored as JSON: { "enabled_tools": [...], "total_tools": N, ... }
                match serde_json::from_slice::<ToolFilterData>(&data) {
                    Ok(filter) => IpcResponse::ToolFilter {
                        server_id: server_id.to_string(),
                        enabled_tools: filter.enabled_tools,
                        total_tools: filter.total_tools,
                        token_estimate_filtered: filter.token_estimate_filtered,
                        token_estimate_full: filter.token_estimate_full,
                    },
                    Err(_) => IpcResponse::ToolFilter {
                        server_id: server_id.to_string(),
                        enabled_tools: vec![],
                        total_tools: 0,
                        token_estimate_filtered: 0,
                        token_estimate_full: 0,
                    },
                }
            }
            Ok(None) => {
                // No filter set — return empty (all tools enabled by default)
                IpcResponse::ToolFilter {
                    server_id: server_id.to_string(),
                    enabled_tools: vec![],
                    total_tools: 0,
                    token_estimate_filtered: 0,
                    token_estimate_full: 0,
                }
            }
            Err(e) => IpcResponse::Error {
                code: "vault_error".to_string(),
                message: format!("Failed to retrieve tool filter: {}", e),
            },
        }
    }

    async fn handle_set_tool_filter(&self, server_id: &str, tool_name: &str, enabled: bool) -> IpcResponse {
        let key = format!("filter:{}", server_id);

        // Read existing filter or create new
        let mut filter = match self.vault.retrieve(&key).await {
            Ok(Some(data)) => serde_json::from_slice::<ToolFilterData>(&data).unwrap_or_default(),
            _ => ToolFilterData::default(),
        };

        if enabled {
            if !filter.enabled_tools.contains(&tool_name.to_string()) {
                filter.enabled_tools.push(tool_name.to_string());
            }
        } else {
            filter.enabled_tools.retain(|t| t != tool_name);
        }

        let data = serde_json::to_vec(&filter).unwrap();
        match self.vault.store(&key, &data).await {
            Ok(()) => IpcResponse::Ok { message: None },
            Err(e) => IpcResponse::Error {
                code: "vault_error".to_string(),
                message: format!("Failed to update tool filter: {}", e),
            },
        }
    }

    async fn handle_set_tool_filter_bulk(&self, server_id: &str, enabled_tools: &[String]) -> IpcResponse {
        let key = format!("filter:{}", server_id);

        // Read existing filter to preserve totals
        let mut filter = match self.vault.retrieve(&key).await {
            Ok(Some(data)) => serde_json::from_slice::<ToolFilterData>(&data).unwrap_or_default(),
            _ => ToolFilterData::default(),
        };

        filter.enabled_tools = enabled_tools.to_vec();

        let data = serde_json::to_vec(&filter).unwrap();
        match self.vault.store(&key, &data).await {
            Ok(()) => IpcResponse::Ok { message: None },
            Err(e) => IpcResponse::Error {
                code: "vault_error".to_string(),
                message: format!("Failed to update tool filter: {}", e),
            },
        }
    }

    async fn handle_register_server(
        &self,
        server_id: &str,
        display_name: &str,
        auth_type: &str,
        upstream_url: Option<&str>,
    ) -> IpcResponse {
        // Store server metadata in vault
        let key = format!("server:{}", server_id);
        let meta = ServerMeta {
            server_id: server_id.to_string(),
            display_name: display_name.to_string(),
            auth_type: auth_type.to_string(),
            upstream_url: upstream_url.map(String::from),
        };
        let data = serde_json::to_vec(&meta).unwrap();
        match self.vault.store(&key, &data).await {
            Ok(()) => IpcResponse::Ok {
                message: Some(format!("Server '{}' registered", server_id)),
            },
            Err(e) => IpcResponse::Error {
                code: "vault_error".to_string(),
                message: format!("Failed to register server: {}", e),
            },
        }
    }

    async fn handle_unregister_server(&self, server_id: &str) -> IpcResponse {
        // Remove server metadata, credentials, and filter from vault
        let keys = [
            format!("server:{}", server_id),
            format!("credential:{}", server_id),
            format!("filter:{}", server_id),
        ];
        for key in &keys {
            if let Err(e) = self.vault.delete(key).await {
                return IpcResponse::Error {
                    code: "vault_error".to_string(),
                    message: format!("Failed to clean up server data: {}", e),
                };
            }
        }
        IpcResponse::Ok {
            message: Some(format!("Server '{}' unregistered", server_id)),
        }
    }

    async fn handle_list_servers(&self) -> IpcResponse {
        // List all registered servers from vault
        match self.vault.list_keys("server:").await {
            Ok(keys) => {
                let mut servers = Vec::new();
                for key in keys {
                    if let Ok(Some(data)) = self.vault.retrieve(&key).await {
                        if let Ok(meta) = serde_json::from_slice::<ServerMeta>(&data) {
                            servers.push(proxy_common::ipc::ServerInfo {
                                server_id: meta.server_id,
                                display_name: meta.display_name,
                                tool_count: 0, // TODO: populate from cache
                                auth_type: meta.auth_type,
                                auth_status: "unknown".to_string(), // TODO: check credential status
                                token_estimate: 0,
                            });
                        }
                    }
                }
                IpcResponse::ServerList { servers }
            }
            Err(e) => IpcResponse::Error {
                code: "vault_error".to_string(),
                message: format!("Failed to list servers: {}", e),
            },
        }
    }
}

/// Internal types for vault storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct ToolFilterData {
    enabled_tools: Vec<String>,
    #[serde(default)]
    total_tools: u32,
    #[serde(default)]
    token_estimate_filtered: u32,
    #[serde(default)]
    token_estimate_full: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ServerMeta {
    server_id: String,
    display_name: String,
    auth_type: String,
    upstream_url: Option<String>,
}

/// Get the default data directory for the current platform.
fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/Application Support/com.brightwing.mcp-manager")
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
            .join("Brightwing")
    }
}

/// Get the default socket path for the current platform.
fn default_socket_path() -> PathBuf {
    let dir = default_data_dir();
    #[cfg(not(target_os = "windows"))]
    {
        dir.join("authd.sock")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"\\.\pipe\brightwing-authd")
    }
}

/// Get the default PID file path.
fn default_pid_path() -> PathBuf {
    default_data_dir().join("authd.pid")
}

/// Write PID file. Returns the path written.
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

    // Check if process is alive (signal 0 = check existence)
    #[cfg(unix)]
    {
        // kill -0 checks if process exists without sending a signal
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result == 0 {
            Some(pid)
        } else {
            None
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, assume alive if PID file exists
        Some(pid)
    }
}

/// Handle a single client connection.
async fn handle_client(
    stream: tokio::net::UnixStream,
    state: Arc<DaemonState>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

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

    // Ensure parent directory exists
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove stale socket file
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Write PID file
    write_pid_file(&pid_path)?;

    // For Phase 1, use in-memory vault. Phase 2 swaps to StrongholdBackend.
    let vault: Arc<dyn VaultBackend> = Arc::new(InMemoryVaultBackend::new());
    let state = Arc::new(DaemonState::new(vault));

    let listener = UnixListener::bind(&socket_path)?;

    // Set socket permissions to owner-only (0600)
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
        // Clean up socket and PID file
        let _ = std::fs::remove_file(&socket_path_clone);
        remove_pid_file(&pid_path_clone);
        std::process::exit(0);
    });

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
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
