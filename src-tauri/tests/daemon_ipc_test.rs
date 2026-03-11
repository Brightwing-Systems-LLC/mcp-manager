//! Integration test: daemon IPC server handles client connections correctly.
//!
//! Tests the full flow: client connects to daemon via Unix socket, performs handshake,
//! and exchanges IPC messages.

use proxy_common::credentials::{ApiKeyCredential, Credential, CredentialErrorCode};
use proxy_common::ipc::{
    ClientType, HandshakeRequest, IpcRequest, IpcResponse, decode_message, encode_message,
};
use proxy_common::vault::InMemoryVaultBackend;
use proxy_common::IPC_PROTOCOL_VERSION;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

// ─── Minimal daemon server (extracted from brightwing_authd.rs) ──────────────

/// Start a test daemon on the given socket path. Returns the vault for seeding credentials.
async fn start_test_daemon(socket_path: &std::path::Path) -> Arc<InMemoryVaultBackend> {
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
                handle_test_client(stream, v).await;
            });
        }
    });

    // Wait for socket to be ready
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    vault
}

/// Seed a credential into the vault.
async fn seed_credential(vault: &InMemoryVaultBackend, server_id: &str, credential: &Credential) {
    let key = format!("credential:{}", server_id);
    let data = serde_json::to_vec(credential).unwrap();
    vault.store(&key, &data).await.unwrap();
}

async fn handle_test_client(stream: UnixStream, vault: Arc<InMemoryVaultBackend>) {
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
                        message: "Incompatible version".to_string(),
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
                                code: CredentialErrorCode::Internal,
                                message: e.to_string(),
                            },
                        },
                    },
                    Ok(None) => IpcResponse::CredentialError {
                        server_id: server_id.clone(),
                        error: proxy_common::credentials::CredentialError {
                            code: CredentialErrorCode::NotFound,
                            message: format!("Server '{}' not found", server_id),
                        },
                    },
                    Err(e) => IpcResponse::CredentialError {
                        server_id,
                        error: proxy_common::credentials::CredentialError {
                            code: CredentialErrorCode::VaultError,
                            message: e.to_string(),
                        },
                    },
                }
            }
            IpcRequest::GetToolFilter { server_id, tool_id: _ } => {
                let key = format!("filter:{}", server_id);
                match vault.retrieve(&key).await {
                    Ok(Some(data)) => {
                        if let Ok(filter) = serde_json::from_slice::<serde_json::Value>(&data) {
                            IpcResponse::ToolFilter {
                                server_id,
                                enabled_tools: filter["enabled_tools"]
                                    .as_array()
                                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                    .unwrap_or_default(),
                                total_tools: filter["total_tools"].as_u64().unwrap_or(0) as u32,
                                token_estimate_filtered: filter["token_estimate_filtered"].as_u64().unwrap_or(0) as u32,
                                token_estimate_full: filter["token_estimate_full"].as_u64().unwrap_or(0) as u32,
                            }
                        } else {
                            IpcResponse::ToolFilter {
                                server_id,
                                enabled_tools: vec![],
                                total_tools: 0,
                                token_estimate_filtered: 0,
                                token_estimate_full: 0,
                            }
                        }
                    }
                    _ => IpcResponse::ToolFilter {
                        server_id,
                        enabled_tools: vec![],
                        total_tools: 0,
                        token_estimate_filtered: 0,
                        token_estimate_full: 0,
                    },
                }
            },
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
            },
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
            },
            IpcRequest::SetToolFilter { server_id, tool_name, enabled, tool_id: _ } => {
                let key = format!("filter:{}", server_id);
                let mut filter: serde_json::Value = match vault.retrieve(&key).await {
                    Ok(Some(data)) => serde_json::from_slice(&data).unwrap_or(serde_json::json!({"enabled_tools": []})),
                    _ => serde_json::json!({"enabled_tools": []}),
                };
                let tools = filter["enabled_tools"].as_array_mut().unwrap();
                if enabled {
                    if !tools.iter().any(|t| t.as_str() == Some(&tool_name)) {
                        tools.push(serde_json::Value::String(tool_name));
                    }
                } else {
                    tools.retain(|t| t.as_str() != Some(&tool_name));
                }
                let data = serde_json::to_vec(&filter).unwrap();
                let _ = vault.store(&key, &data).await;
                IpcResponse::Ok { message: None }
            },
            IpcRequest::SetToolFilterBulk { server_id, enabled_tools, tool_id: _ } => {
                let key = format!("filter:{}", server_id);
                let filter = serde_json::json!({"enabled_tools": enabled_tools});
                let data = serde_json::to_vec(&filter).unwrap();
                let _ = vault.store(&key, &data).await;
                IpcResponse::Ok { message: None }
            },
            IpcRequest::RegisterServer { server_id, .. } => {
                IpcResponse::Ok {
                    message: Some(format!("Server '{}' registered", server_id)),
                }
            },
            IpcRequest::UnregisterServer { server_id } => {
                IpcResponse::Ok {
                    message: Some(format!("Server '{}' unregistered", server_id)),
                }
            },
            IpcRequest::ListServers => IpcResponse::ServerList { servers: vec![] },
            IpcRequest::ListTools { server_id } => IpcResponse::ToolList {
                server_id,
                tools: vec![],
            },
            _ => IpcResponse::Error {
                code: "not_implemented".to_string(),
                message: "Not yet implemented".to_string(),
            },
        };

        let bytes = encode_message(&response).unwrap();
        if writer.write_all(&bytes).await.is_err() {
            break;
        }
    }
}

// ─── IPC client helper ──────────────────────────────────────────────────────

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
            client: ClientType::Proxy,
            version: IPC_PROTOCOL_VERSION.to_string(),
        }))
        .await;

        match self.recv().await {
            IpcResponse::HandshakeOk { .. } => {}
            other => panic!("Expected HandshakeOk, got: {:?}", other),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

use proxy_common::vault::VaultBackend;

use std::sync::atomic::{AtomicU64, Ordering};
static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_socket() -> std::path::PathBuf {
    let dir = std::env::temp_dir();
    let id = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!("bw-test-{}-{}.sock", std::process::id(), id))
}

#[tokio::test]
async fn test_handshake_success() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_handshake_version_mismatch() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client
        .send(&IpcRequest::Handshake(HandshakeRequest {
            client: ClientType::Proxy,
            version: "99.0.0".to_string(),
        }))
        .await;

    match client.recv().await {
        IpcResponse::HandshakeError { message, .. } => {
            assert!(message.contains("Incompatible"));
        }
        other => panic!("Expected HandshakeError, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_get_credentials_found() {
    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;

    // Seed a credential before connecting
    let mut env = HashMap::new();
    env.insert("GITHUB_TOKEN".to_string(), "ghp_test123".to_string());
    let cred = Credential::ApiKey(ApiKeyCredential {
        env,
        command: Some("npx".to_string()),
        args: Some(vec!["-y".to_string(), "server-github".to_string()]),
        url: None,
                api_key_injection: None,
    });
    seed_credential(&vault, "github", &cred).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    client
        .send(&IpcRequest::GetCredentials {
            server_id: "github".to_string(),
        })
        .await;

    match client.recv().await {
        IpcResponse::Credentials {
            server_id,
            credential,
        } => {
            assert_eq!(server_id, "github");
            assert_eq!(credential, cred);
        }
        other => panic!("Expected Credentials, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_get_credentials_not_found() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    client
        .send(&IpcRequest::GetCredentials {
            server_id: "nonexistent".to_string(),
        })
        .await;

    match client.recv().await {
        IpcResponse::CredentialError { server_id, error } => {
            assert_eq!(server_id, "nonexistent");
            assert_eq!(error.code, CredentialErrorCode::NotFound);
        }
        other => panic!("Expected CredentialError, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_multiple_requests_same_connection() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    // Send multiple requests on the same connection
    client.send(&IpcRequest::ListServers).await;
    match client.recv().await {
        IpcResponse::ServerList { servers } => assert!(servers.is_empty()),
        other => panic!("Expected ServerList, got: {:?}", other),
    }

    client
        .send(&IpcRequest::GetToolFilter {
            server_id: "test".to_string(),
            tool_id: None,
        })
        .await;
    match client.recv().await {
        IpcResponse::ToolFilter { server_id, .. } => assert_eq!(server_id, "test"),
        other => panic!("Expected ToolFilter, got: {:?}", other),
    }

    client
        .send(&IpcRequest::ListTools {
            server_id: "test".to_string(),
        })
        .await;
    match client.recv().await {
        IpcResponse::ToolList { server_id, tools } => {
            assert_eq!(server_id, "test");
            assert!(tools.is_empty());
        }
        other => panic!("Expected ToolList, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_concurrent_clients() {
    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;

    // Seed a credential
    let mut env = HashMap::new();
    env.insert("KEY".to_string(), "val".to_string());
    seed_credential(
        &vault,
        "srv",
        &Credential::ApiKey(ApiKeyCredential {
            env,
            command: None,
            args: None,
            url: Some("http://example.com".to_string()),
                api_key_injection: None,
        }),
    )
    .await;

    // Connect two clients simultaneously
    let socket2 = socket.clone();
    let socket3 = socket.clone();

    let h1 = tokio::spawn(async move {
        let mut c = TestClient::connect(&socket2).await;
        c.handshake().await;
        c.send(&IpcRequest::GetCredentials {
            server_id: "srv".to_string(),
        })
        .await;
        matches!(c.recv().await, IpcResponse::Credentials { .. })
    });

    let h2 = tokio::spawn(async move {
        let mut c = TestClient::connect(&socket3).await;
        c.handshake().await;
        c.send(&IpcRequest::ListServers).await;
        matches!(c.recv().await, IpcResponse::ServerList { .. })
    });

    assert!(h1.await.unwrap());
    assert!(h2.await.unwrap());

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_malformed_request() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let stream = UnixStream::connect(&socket).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // Send garbage
    writer.write_all(b"not valid json\n").await.unwrap();

    let line = lines.next_line().await.unwrap().unwrap();
    let resp: IpcResponse = decode_message(line.as_bytes()).unwrap();
    match resp {
        IpcResponse::Error { code, .. } => assert_eq!(code, "parse_error"),
        other => panic!("Expected Error, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_store_and_retrieve_credentials() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    // Store credentials via IPC
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "sk-test123".to_string());
    let cred = Credential::ApiKey(ApiKeyCredential {
        env,
        command: None,
        args: None,
        url: Some("https://brave-mcp.example.com".to_string()),
                api_key_injection: None,
    });

    client
        .send(&IpcRequest::StoreCredentials {
            server_id: "brave".to_string(),
            credential: cred.clone(),
        })
        .await;

    match client.recv().await {
        IpcResponse::Ok { message } => {
            assert!(message.unwrap().contains("brave"));
        }
        other => panic!("Expected Ok, got: {:?}", other),
    }

    // Now retrieve them
    client
        .send(&IpcRequest::GetCredentials {
            server_id: "brave".to_string(),
        })
        .await;

    match client.recv().await {
        IpcResponse::Credentials { server_id, credential } => {
            assert_eq!(server_id, "brave");
            assert_eq!(credential, cred);
        }
        other => panic!("Expected Credentials, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_delete_credentials() {
    let socket = temp_socket();
    let vault = start_test_daemon(&socket).await;

    // Seed a credential directly
    let mut env = HashMap::new();
    env.insert("TOKEN".to_string(), "xxx".to_string());
    seed_credential(
        &vault,
        "deleteme",
        &Credential::ApiKey(ApiKeyCredential {
            env,
            command: None,
            args: None,
            url: None,
                api_key_injection: None,
        }),
    )
    .await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    // Delete via IPC
    client
        .send(&IpcRequest::DeleteCredentials {
            server_id: "deleteme".to_string(),
        })
        .await;

    match client.recv().await {
        IpcResponse::Ok { .. } => {}
        other => panic!("Expected Ok, got: {:?}", other),
    }

    // Verify it's gone
    client
        .send(&IpcRequest::GetCredentials {
            server_id: "deleteme".to_string(),
        })
        .await;

    match client.recv().await {
        IpcResponse::CredentialError { error, .. } => {
            assert_eq!(error.code, CredentialErrorCode::NotFound);
        }
        other => panic!("Expected CredentialError, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_tool_filter_bulk_and_retrieve() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    // Set bulk filter
    client
        .send(&IpcRequest::SetToolFilterBulk {
            server_id: "github".to_string(),
            enabled_tools: vec!["search_repos".to_string(), "get_repo".to_string()],
            tool_id: None,
        })
        .await;

    match client.recv().await {
        IpcResponse::Ok { .. } => {}
        other => panic!("Expected Ok, got: {:?}", other),
    }

    // Retrieve filter
    client
        .send(&IpcRequest::GetToolFilter {
            server_id: "github".to_string(),
            tool_id: None,
        })
        .await;

    match client.recv().await {
        IpcResponse::ToolFilter {
            server_id,
            enabled_tools,
            ..
        } => {
            assert_eq!(server_id, "github");
            assert_eq!(enabled_tools.len(), 2);
            assert!(enabled_tools.contains(&"search_repos".to_string()));
            assert!(enabled_tools.contains(&"get_repo".to_string()));
        }
        other => panic!("Expected ToolFilter, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_credential_store_overwrite() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    // Store initial credential
    let mut env1 = HashMap::new();
    env1.insert("TOKEN".to_string(), "first-value".to_string());
    client
        .send(&IpcRequest::StoreCredentials {
            server_id: "overwrite-test".to_string(),
            credential: Credential::ApiKey(ApiKeyCredential {
                env: env1,
                command: None,
                args: None,
                url: None,
                api_key_injection: None,
            }),
        })
        .await;
    let _ = client.recv().await;

    // Overwrite with new credential
    let mut env2 = HashMap::new();
    env2.insert("TOKEN".to_string(), "second-value".to_string());
    env2.insert("EXTRA".to_string(), "extra-val".to_string());
    let cred2 = Credential::ApiKey(ApiKeyCredential {
        env: env2,
        command: Some("node".to_string()),
        args: Some(vec!["server.js".to_string()]),
        url: None,
                api_key_injection: None,
    });
    client
        .send(&IpcRequest::StoreCredentials {
            server_id: "overwrite-test".to_string(),
            credential: cred2.clone(),
        })
        .await;
    let _ = client.recv().await;

    // Retrieve — should be the latest
    client
        .send(&IpcRequest::GetCredentials {
            server_id: "overwrite-test".to_string(),
        })
        .await;

    match client.recv().await {
        IpcResponse::Credentials { credential, .. } => {
            assert_eq!(credential, cred2);
        }
        other => panic!("Expected Credentials, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_tool_filter_toggle_reenable() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    // Bulk set: a, b, c
    client
        .send(&IpcRequest::SetToolFilterBulk {
            server_id: "srv".to_string(),
            enabled_tools: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            tool_id: None,
        })
        .await;
    let _ = client.recv().await;

    // Disable b
    client
        .send(&IpcRequest::SetToolFilter {
            server_id: "srv".to_string(),
            tool_name: "b".to_string(),
            enabled: false,
            tool_id: None,
        })
        .await;
    let _ = client.recv().await;

    // Re-enable b
    client
        .send(&IpcRequest::SetToolFilter {
            server_id: "srv".to_string(),
            tool_name: "b".to_string(),
            enabled: true,
            tool_id: None,
        })
        .await;
    let _ = client.recv().await;

    // Verify b is back
    client
        .send(&IpcRequest::GetToolFilter {
            server_id: "srv".to_string(),
            tool_id: None,
        })
        .await;

    match client.recv().await {
        IpcResponse::ToolFilter { enabled_tools, .. } => {
            assert_eq!(enabled_tools.len(), 3);
            assert!(enabled_tools.contains(&"a".to_string()));
            assert!(enabled_tools.contains(&"b".to_string()));
            assert!(enabled_tools.contains(&"c".to_string()));
        }
        other => panic!("Expected ToolFilter, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn test_tool_filter_toggle_single() {
    let socket = temp_socket();
    let _vault = start_test_daemon(&socket).await;

    let mut client = TestClient::connect(&socket).await;
    client.handshake().await;

    // Start with bulk set
    client
        .send(&IpcRequest::SetToolFilterBulk {
            server_id: "github".to_string(),
            enabled_tools: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            tool_id: None,
        })
        .await;
    let _ = client.recv().await;

    // Disable tool "b"
    client
        .send(&IpcRequest::SetToolFilter {
            server_id: "github".to_string(),
            tool_name: "b".to_string(),
            enabled: false,
            tool_id: None,
        })
        .await;
    let _ = client.recv().await;

    // Retrieve and check
    client
        .send(&IpcRequest::GetToolFilter {
            server_id: "github".to_string(),
            tool_id: None,
        })
        .await;

    match client.recv().await {
        IpcResponse::ToolFilter { enabled_tools, .. } => {
            assert_eq!(enabled_tools.len(), 2);
            assert!(enabled_tools.contains(&"a".to_string()));
            assert!(enabled_tools.contains(&"c".to_string()));
            assert!(!enabled_tools.contains(&"b".to_string()));
        }
        other => panic!("Expected ToolFilter, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket);
}
