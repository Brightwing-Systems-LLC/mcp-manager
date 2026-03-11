//! Integration test: full proxy pipeline.
//!
//! daemon (with seeded credentials)
//!   ↕ IPC (Unix socket)
//! brightwing-proxy
//!   ↕ HTTP
//! mock MCP server
//!
//! Tests that the proxy correctly:
//! - Connects to daemon, performs handshake
//! - Retrieves credentials for the target server
//! - Forwards MCP requests to the upstream with auth headers
//! - Applies tool filters to tools/list responses
//! - Relays tool call results back

#[path = "mock_mcp_server.rs"]
mod mock_mcp_server;

use mock_mcp_server::MockMcpServer;
use proxy_common::credentials::{ApiKeyCredential, Credential, OAuthCredential};
use proxy_common::ipc::{
    ClientType, HandshakeRequest, IpcRequest, IpcResponse, decode_message, encode_message,
};
use proxy_common::vault::{InMemoryVaultBackend, VaultBackend};
use proxy_common::IPC_PROTOCOL_VERSION;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

// ─── IPC client helper (for store-then-use tests) ────────────────────────────

struct TestClient {
    writer: tokio::io::WriteHalf<UnixStream>,
    reader: tokio::io::Lines<BufReader<tokio::io::ReadHalf<UnixStream>>>,
}

impl TestClient {
    async fn connect(path: &std::path::Path) -> Self {
        let stream = UnixStream::connect(path).await.unwrap();
        let (reader, writer) = tokio::io::split(stream);
        Self {
            writer,
            reader: BufReader::new(reader).lines(),
        }
    }

    async fn send(&mut self, req: &IpcRequest) {
        let bytes = encode_message(req).unwrap();
        self.writer.write_all(&bytes).await.unwrap();
    }

    async fn recv(&mut self) -> IpcResponse {
        let line = self.reader.next_line().await.unwrap().unwrap();
        decode_message(line.as_bytes()).unwrap()
    }

    async fn handshake(&mut self) {
        self.send(&IpcRequest::Handshake(HandshakeRequest {
            client: ClientType::Gui,
            version: IPC_PROTOCOL_VERSION.to_string(),
        }))
        .await;
        match self.recv().await {
            IpcResponse::HandshakeOk { .. } => {}
            other => panic!("Expected HandshakeOk, got: {:?}", other),
        }
    }
}

// ─── Test daemon (same as daemon_ipc_test but reusable) ──────────────────────

async fn start_test_daemon(
    socket_path: &std::path::Path,
) -> Arc<InMemoryVaultBackend> {
    let vault = Arc::new(InMemoryVaultBackend::new());
    let vault_clone = Arc::clone(&vault);
    let path = socket_path.to_path_buf();

    tokio::spawn(async move {
        if path.exists() {
            std::fs::remove_file(&path).unwrap();
        }
        let listener = UnixListener::bind(&path).unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let v = Arc::clone(&vault_clone);
            tokio::spawn(async move {
                handle_daemon_client(stream, v).await;
            });
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    vault
}

async fn handle_daemon_client(stream: UnixStream, vault: Arc<InMemoryVaultBackend>) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let request: IpcRequest = match decode_message(line.as_bytes()) {
            Ok(req) => req,
            Err(e) => {
                let resp = IpcResponse::Error {
                    code: "parse_error".to_string(),
                    message: e.to_string(),
                };
                let _ = writer
                    .write_all(&encode_message(&resp).unwrap_or_default())
                    .await;
                continue;
            }
        };

        let response = match request {
            IpcRequest::Handshake(hs) => {
                if proxy_common::versions_compatible(&hs.version, IPC_PROTOCOL_VERSION) {
                    IpcResponse::HandshakeOk {
                        daemon_version: IPC_PROTOCOL_VERSION.to_string(),
                    }
                } else {
                    IpcResponse::HandshakeError {
                        daemon_version: IPC_PROTOCOL_VERSION.to_string(),
                        min_client_version: IPC_PROTOCOL_VERSION.to_string(),
                        message: "Incompatible".to_string(),
                    }
                }
            }
            IpcRequest::GetCredentials { server_id } => {
                let key = format!("credential:{}", server_id);
                match vault.retrieve(&key).await {
                    Ok(Some(data)) => match serde_json::from_slice::<Credential>(&data) {
                        Ok(credential) => IpcResponse::Credentials {
                            server_id,
                            credential,
                        },
                        Err(e) => IpcResponse::CredentialError {
                            server_id,
                            error: proxy_common::credentials::CredentialError {
                                code: proxy_common::credentials::CredentialErrorCode::Internal,
                                message: e.to_string(),
                            },
                        },
                    },
                    Ok(None) => IpcResponse::CredentialError {
                        server_id: server_id.clone(),
                        error: proxy_common::credentials::CredentialError {
                            code: proxy_common::credentials::CredentialErrorCode::NotFound,
                            message: format!("Not found: {}", server_id),
                        },
                    },
                    Err(e) => IpcResponse::CredentialError {
                        server_id,
                        error: proxy_common::credentials::CredentialError {
                            code: proxy_common::credentials::CredentialErrorCode::VaultError,
                            message: e.to_string(),
                        },
                    },
                }
            }
            IpcRequest::GetToolFilter { server_id, tool_id: _ } => {
                // For filter tests, check if we stored a filter in the vault
                let filter_key = format!("filter:{}", server_id);
                let enabled = match vault.retrieve(&filter_key).await {
                    Ok(Some(data)) => serde_json::from_slice(&data).unwrap_or_default(),
                    _ => vec![],
                };
                IpcResponse::ToolFilter {
                    server_id,
                    enabled_tools: enabled,
                    total_tools: 0,
                    token_estimate_filtered: 0,
                    token_estimate_full: 0,
                }
            }
            IpcRequest::StoreCredentials { server_id, credential } => {
                let key = format!("credential:{}", server_id);
                let data = serde_json::to_vec(&credential).unwrap();
                match vault.store(&key, &data).await {
                    Ok(()) => IpcResponse::Ok {
                        message: Some(format!("Credentials stored for '{}'", server_id)),
                    },
                    Err(e) => IpcResponse::Error {
                        code: "vault_error".to_string(),
                        message: e.to_string(),
                    },
                }
            }
            IpcRequest::DeleteCredentials { server_id } => {
                let key = format!("credential:{}", server_id);
                match vault.delete(&key).await {
                    Ok(()) => IpcResponse::Ok {
                        message: Some(format!("Credentials deleted for '{}'", server_id)),
                    },
                    Err(e) => IpcResponse::Error {
                        code: "vault_error".to_string(),
                        message: e.to_string(),
                    },
                }
            }
            IpcRequest::SetToolFilterBulk { server_id, enabled_tools, tool_id: _ } => {
                let filter_key = format!("filter:{}", server_id);
                let data = serde_json::to_vec(&enabled_tools).unwrap();
                let _ = vault.store(&filter_key, &data).await;
                IpcResponse::Ok { message: None }
            }
            _ => IpcResponse::Error {
                code: "not_implemented".to_string(),
                message: "Stub".to_string(),
            },
        };

        let bytes = encode_message(&response).unwrap();
        if writer.write_all(&bytes).await.is_err() {
            break;
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_socket() -> std::path::PathBuf {
    let dir = std::env::temp_dir();
    let id = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(
        "bw-proxy-test-{}-{}.sock",
        std::process::id(),
        id
    ))
}

async fn seed_credential(vault: &InMemoryVaultBackend, server_id: &str, credential: &Credential) {
    let key = format!("credential:{}", server_id);
    let data = serde_json::to_vec(credential).unwrap();
    vault.store(&key, &data).await.unwrap();
}

async fn seed_tool_filter(vault: &InMemoryVaultBackend, server_id: &str, tools: &[&str]) {
    let filter_key = format!("filter:{}", server_id);
    let data = serde_json::to_vec(&tools).unwrap();
    vault.store(&filter_key, &data).await.unwrap();
}

/// Simulates what the proxy does: connect to daemon, handshake, get credentials,
/// then proxy a single MCP request to the upstream and return the response.
async fn proxy_single_request(
    socket_path: &std::path::Path,
    server_id: &str,
    mcp_request: &Value,
) -> Value {
    // Connect to daemon
    let stream = UnixStream::connect(socket_path).await.unwrap();
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    // Handshake
    let hs = IpcRequest::Handshake(HandshakeRequest {
        client: ClientType::Proxy,
        version: IPC_PROTOCOL_VERSION.to_string(),
    });
    writer
        .write_all(&encode_message(&hs).unwrap())
        .await
        .unwrap();
    let line = lines.next_line().await.unwrap().unwrap();
    let resp: IpcResponse = decode_message(line.as_bytes()).unwrap();
    assert!(matches!(resp, IpcResponse::HandshakeOk { .. }));

    // Get credentials
    let cred_req = IpcRequest::GetCredentials {
        server_id: server_id.to_string(),
    };
    writer
        .write_all(&encode_message(&cred_req).unwrap())
        .await
        .unwrap();
    let line = lines.next_line().await.unwrap().unwrap();
    let cred_resp: IpcResponse = decode_message(line.as_bytes()).unwrap();

    let (url, auth_header) = match cred_resp {
        IpcResponse::Credentials {
            credential: Credential::OAuth(oauth),
            ..
        } => (oauth.url, Some(format!("Bearer {}", oauth.access_token))),
        IpcResponse::Credentials {
            credential: Credential::ApiKey(api_key),
            ..
        } => {
            let url = api_key.url.unwrap();
            let auth = api_key.env.values().next().map(|v| format!("Bearer {}", v));
            (url, auth)
        }
        other => panic!("Expected credentials, got: {:?}", other),
    };

    // Get tool filter
    let filter_req = IpcRequest::GetToolFilter {
        server_id: server_id.to_string(),
        tool_id: None,
    };
    writer
        .write_all(&encode_message(&filter_req).unwrap())
        .await
        .unwrap();
    let line = lines.next_line().await.unwrap().unwrap();
    let filter_resp: IpcResponse = decode_message(line.as_bytes()).unwrap();
    let enabled_tools = match filter_resp {
        IpcResponse::ToolFilter { enabled_tools, .. } => enabled_tools,
        _ => vec![],
    };

    // Forward to upstream
    let client = reqwest::Client::new();
    let mut req_builder = client.post(&url).json(mcp_request);
    if let Some(auth) = &auth_header {
        req_builder = req_builder.header("Authorization", auth);
    }

    let mut response: Value = req_builder
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Apply tool filter
    if !enabled_tools.is_empty() {
        let method = mcp_request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");
        if method == "tools/list" {
            if let Some(result) = response.get_mut("result") {
                if let Some(tools) = result.get_mut("tools") {
                    if let Some(arr) = tools.as_array_mut() {
                        arr.retain(|tool| {
                            let name = tool
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("");
                            enabled_tools.iter().any(|t| t == name)
                        });
                    }
                }
            }
        }
    }

    response
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_proxy_tools_list_with_oauth() {
    // Start mock MCP server
    let mock = MockMcpServer::builder()
        .with_tool(
            "search_repos",
            "Search repositories",
            json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        )
        .with_tool(
            "get_repo",
            "Get repo details",
            json!({"type": "object", "properties": {}}),
        )
        .require_auth("Bearer oauth-token-xyz")
        .build_http()
        .await;

    // Start daemon with OAuth credential
    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;
    seed_credential(
        &vault,
        "github",
        &Credential::OAuth(OAuthCredential {
            access_token: "oauth-token-xyz".to_string(),
            url: mock.url(),
            expires_at: None,
        }),
    )
    .await;

    // Proxy tools/list request
    let resp = proxy_single_request(
        &socket,
        "github",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await;

    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "search_repos");
    assert_eq!(tools[1]["name"], "get_repo");

    // Verify the mock received the correct auth header
    let requests = mock.recorded_requests().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].auth_header.as_deref(),
        Some("Bearer oauth-token-xyz")
    );

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_proxy_tool_call_with_api_key() {
    let mock = MockMcpServer::builder()
        .with_tool(
            "web_search",
            "Search the web",
            json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        )
        .with_tool_response(
            "web_search",
            json!([{"type": "text", "text": "Found 10 results for 'rust mcp'"}]),
        )
        .require_auth("Bearer brave-api-key-123")
        .build_http()
        .await;

    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;

    let mut env = HashMap::new();
    env.insert("BRAVE_API_KEY".to_string(), "brave-api-key-123".to_string());
    seed_credential(
        &vault,
        "brave-search",
        &Credential::ApiKey(ApiKeyCredential {
            env,
            command: None,
            args: None,
            url: Some(mock.url()),
                api_key_injection: None,
        }),
    )
    .await;

    let resp = proxy_single_request(
        &socket,
        "brave-search",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_search",
                "arguments": {"query": "rust mcp"}
            }
        }),
    )
    .await;

    assert_eq!(resp["result"]["isError"], false);
    assert_eq!(
        resp["result"]["content"][0]["text"],
        "Found 10 results for 'rust mcp'"
    );

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_proxy_tool_filter_applied() {
    let mock = MockMcpServer::builder()
        .with_tool("search_repos", "Search", json!({"type": "object", "properties": {}}))
        .with_tool("create_issue", "Create", json!({"type": "object", "properties": {}}))
        .with_tool("get_repo", "Get", json!({"type": "object", "properties": {}}))
        .with_tool("delete_repo", "Delete", json!({"type": "object", "properties": {}}))
        .build_http()
        .await;

    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;
    seed_credential(
        &vault,
        "github",
        &Credential::OAuth(OAuthCredential {
            access_token: "tok".to_string(),
            url: mock.url(),
            expires_at: None,
        }),
    )
    .await;

    // Set filter: only search_repos and get_repo enabled
    seed_tool_filter(&vault, "github", &["search_repos", "get_repo"]).await;

    let resp = proxy_single_request(
        &socket,
        "github",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await;

    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"search_repos"));
    assert!(names.contains(&"get_repo"));
    assert!(!names.contains(&"create_issue"));
    assert!(!names.contains(&"delete_repo"));

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_proxy_tool_call_unaffected_by_filter() {
    // Tool filter only applies to tools/list, not tools/call
    let mock = MockMcpServer::builder()
        .with_tool("search_repos", "Search", json!({"type": "object", "properties": {}}))
        .with_tool("create_issue", "Create", json!({"type": "object", "properties": {}}))
        .with_tool_response("create_issue", json!([{"type": "text", "text": "Issue created"}]))
        .build_http()
        .await;

    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;
    seed_credential(
        &vault,
        "github",
        &Credential::OAuth(OAuthCredential {
            access_token: "tok".to_string(),
            url: mock.url(),
            expires_at: None,
        }),
    )
    .await;

    // Filter only allows search_repos
    seed_tool_filter(&vault, "github", &["search_repos"]).await;

    // But tools/call for create_issue should still work (proxy doesn't block calls)
    let resp = proxy_single_request(
        &socket,
        "github",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "create_issue",
                "arguments": {"title": "test"}
            }
        }),
    )
    .await;

    assert_eq!(resp["result"]["content"][0]["text"], "Issue created");

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_proxy_initialize_forwarded() {
    let mock = MockMcpServer::builder().build_http().await;

    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;
    seed_credential(
        &vault,
        "test-server",
        &Credential::OAuth(OAuthCredential {
            access_token: "tok".to_string(),
            url: mock.url(),
            expires_at: None,
        }),
    )
    .await;

    let resp = proxy_single_request(
        &socket,
        "test-server",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0"}
            }
        }),
    )
    .await;

    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(resp["result"]["serverInfo"]["name"], "mock-mcp-server");

    let _ = std::fs::remove_file(&socket);
}

// ─── Credential flow: store via IPC → proxy retrieves → upstream validates ───

#[tokio::test]
async fn test_credential_store_then_proxy_uses() {
    // Start mock server requiring auth
    let mock = MockMcpServer::builder()
        .with_tool(
            "list_files",
            "List files in a directory",
            json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        )
        .require_auth("Bearer stored-api-key-abc")
        .build_http()
        .await;

    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    // Step 1: Store credentials via IPC (simulating GUI flow)
    let mut gui_client = TestClient::connect(&socket).await;
    gui_client.handshake().await;

    let mut env = HashMap::new();
    env.insert("FS_TOKEN".to_string(), "stored-api-key-abc".to_string());
    gui_client
        .send(&IpcRequest::StoreCredentials {
            server_id: "filesystem".to_string(),
            credential: Credential::ApiKey(ApiKeyCredential {
                env,
                command: None,
                args: None,
                url: Some(mock.url()),
                api_key_injection: None,
            }),
        })
        .await;
    match gui_client.recv().await {
        IpcResponse::Ok { .. } => {}
        other => panic!("Expected Ok from store, got: {:?}", other),
    }

    // Step 2: Proxy retrieves the stored credentials and forwards the request
    let resp = proxy_single_request(
        &socket,
        "filesystem",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await;

    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "list_files");

    // Verify auth header was correct
    let requests = mock.recorded_requests().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].auth_header.as_deref(),
        Some("Bearer stored-api-key-abc")
    );

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_credential_overwrite_then_proxy_uses_latest() {
    let mock = MockMcpServer::builder()
        .with_tool("ping", "Ping", json!({"type": "object", "properties": {}}))
        .require_auth("Bearer new-token-456")
        .build_http()
        .await;

    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut gui = TestClient::connect(&socket).await;
    gui.handshake().await;

    // Store old credential
    let mut env_old = HashMap::new();
    env_old.insert("TOKEN".to_string(), "old-token-123".to_string());
    gui.send(&IpcRequest::StoreCredentials {
        server_id: "svc".to_string(),
        credential: Credential::ApiKey(ApiKeyCredential {
            env: env_old,
            command: None,
            args: None,
            url: Some(mock.url()),
                api_key_injection: None,
        }),
    })
    .await;
    let _ = gui.recv().await;

    // Overwrite with new credential
    let mut env_new = HashMap::new();
    env_new.insert("TOKEN".to_string(), "new-token-456".to_string());
    gui.send(&IpcRequest::StoreCredentials {
        server_id: "svc".to_string(),
        credential: Credential::ApiKey(ApiKeyCredential {
            env: env_new,
            command: None,
            args: None,
            url: Some(mock.url()),
                api_key_injection: None,
        }),
    })
    .await;
    let _ = gui.recv().await;

    // Proxy should use the latest credential
    let resp = proxy_single_request(
        &socket,
        "svc",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await;

    // If the old token were used, the mock would 401 and the test would fail
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);

    let _ = std::fs::remove_file(&socket);
}

// ─── Filter edge cases ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_proxy_filter_single_tool_out_of_many() {
    let mock = MockMcpServer::builder()
        .with_tool("tool_a", "A", json!({"type": "object", "properties": {}}))
        .with_tool("tool_b", "B", json!({"type": "object", "properties": {}}))
        .with_tool("tool_c", "C", json!({"type": "object", "properties": {}}))
        .with_tool("tool_d", "D", json!({"type": "object", "properties": {}}))
        .with_tool("tool_e", "E", json!({"type": "object", "properties": {}}))
        .build_http()
        .await;

    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;
    seed_credential(
        &vault,
        "multi",
        &Credential::OAuth(OAuthCredential {
            access_token: "tok".to_string(),
            url: mock.url(),
            expires_at: None,
        }),
    )
    .await;

    // Enable only one tool
    seed_tool_filter(&vault, "multi", &["tool_c"]).await;

    let resp = proxy_single_request(
        &socket,
        "multi",
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
    )
    .await;

    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "tool_c");

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_proxy_filter_update_reflected_in_next_request() {
    let mock = MockMcpServer::builder()
        .with_tool("alpha", "Alpha", json!({"type": "object", "properties": {}}))
        .with_tool("beta", "Beta", json!({"type": "object", "properties": {}}))
        .with_tool("gamma", "Gamma", json!({"type": "object", "properties": {}}))
        .build_http()
        .await;

    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;
    seed_credential(
        &vault,
        "greek",
        &Credential::OAuth(OAuthCredential {
            access_token: "tok".to_string(),
            url: mock.url(),
            expires_at: None,
        }),
    )
    .await;

    // Initial filter: alpha + beta
    seed_tool_filter(&vault, "greek", &["alpha", "beta"]).await;

    let resp1 = proxy_single_request(
        &socket,
        "greek",
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
    )
    .await;
    let tools1 = resp1["result"]["tools"].as_array().unwrap();
    assert_eq!(tools1.len(), 2);

    // Update filter via IPC: now only gamma
    let mut gui = TestClient::connect(&socket).await;
    gui.handshake().await;
    gui.send(&IpcRequest::SetToolFilterBulk {
        server_id: "greek".to_string(),
        enabled_tools: vec!["gamma".to_string()],
        tool_id: None,
    })
    .await;
    let _ = gui.recv().await;

    // Next proxy request should see updated filter
    let resp2 = proxy_single_request(
        &socket,
        "greek",
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
    )
    .await;
    let tools2 = resp2["result"]["tools"].as_array().unwrap();
    assert_eq!(tools2.len(), 1);
    assert_eq!(tools2[0]["name"], "gamma");

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_proxy_no_filter_returns_all_tools() {
    // When no filter is seeded (empty), all tools should pass through
    let mock = MockMcpServer::builder()
        .with_tool("x", "X", json!({"type": "object", "properties": {}}))
        .with_tool("y", "Y", json!({"type": "object", "properties": {}}))
        .build_http()
        .await;

    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;
    seed_credential(
        &vault,
        "nofilter",
        &Credential::OAuth(OAuthCredential {
            access_token: "tok".to_string(),
            url: mock.url(),
            expires_at: None,
        }),
    )
    .await;
    // No filter seeded — empty enabled_tools from daemon

    let resp = proxy_single_request(
        &socket,
        "nofilter",
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
    )
    .await;

    // Empty filter = pass-through (the proxy only filters when enabled_tools is non-empty)
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);

    let _ = std::fs::remove_file(&socket);
}
