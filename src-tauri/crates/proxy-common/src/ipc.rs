use crate::credentials::{Credential, CredentialError};
use serde::{Deserialize, Serialize};

// ─── Handshake ───────────────────────────────────────────────────────────────

/// Sent by proxy/bw on connect. Daemon checks version compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeRequest {
    pub client: ClientType,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Proxy,
    CliShim,
    Gui,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum HandshakeResponse {
    #[serde(rename = "handshake_ok")]
    Ok { daemon_version: String },

    #[serde(rename = "handshake_error")]
    Error {
        daemon_version: String,
        min_client_version: String,
        message: String,
    },
}

// ─── Request / Response envelope ─────────────────────────────────────────────

/// All messages from client → daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action")]
pub enum IpcRequest {
    // Handshake
    #[serde(rename = "handshake")]
    Handshake(HandshakeRequest),

    // Credential management
    #[serde(rename = "get_credentials")]
    GetCredentials { server_id: String },

    // Tool filtering
    #[serde(rename = "get_tool_filter")]
    GetToolFilter {
        server_id: String,
        /// App identifier (e.g. "claude_code", "cursor"). Defaults to "_all" if absent.
        #[serde(default)]
        tool_id: Option<String>,
    },

    // CLI shim: list all servers
    #[serde(rename = "list_servers")]
    ListServers,

    // CLI shim: list tools for a server
    #[serde(rename = "list_tools")]
    ListTools { server_id: String },

    // CLI shim: get schema for a single tool
    #[serde(rename = "get_tool_schema")]
    GetToolSchema {
        server_id: String,
        tool_name: String,
    },

    // Call a tool (both proxy and bw shim)
    #[serde(rename = "call_tool")]
    CallTool {
        server_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },

    // ─── Credential management (GUI → daemon) ───────────────────────────

    /// Store or update credentials for a server.
    #[serde(rename = "store_credentials")]
    StoreCredentials {
        server_id: String,
        credential: Credential,
    },

    /// Delete credentials for a server.
    #[serde(rename = "delete_credentials")]
    DeleteCredentials { server_id: String },

    // ─── Tool filter management (GUI → daemon) ──────────────────────────

    /// Set enabled/disabled status for a single tool.
    #[serde(rename = "set_tool_filter")]
    SetToolFilter {
        server_id: String,
        tool_name: String,
        enabled: bool,
        /// App identifier. Defaults to "_all" if absent.
        #[serde(default)]
        tool_id: Option<String>,
    },

    /// Bulk-set the entire tool filter for a server (replaces all).
    #[serde(rename = "set_tool_filter_bulk")]
    SetToolFilterBulk {
        server_id: String,
        enabled_tools: Vec<String>,
        /// App identifier. Defaults to "_all" if absent.
        #[serde(default)]
        tool_id: Option<String>,
    },

    /// Register a new proxy server in the daemon.
    #[serde(rename = "register_server")]
    RegisterServer {
        server_id: String,
        display_name: String,
        /// "oauth", "api_key", or "none"
        auth_type: String,
        /// Upstream MCP server URL (for HTTP transport)
        #[serde(skip_serializing_if = "Option::is_none")]
        upstream_url: Option<String>,
    },

    /// Unregister a proxy server from the daemon.
    #[serde(rename = "unregister_server")]
    UnregisterServer { server_id: String },

    /// Submit a log event from the proxy to the daemon.
    #[serde(rename = "submit_log")]
    SubmitLog { event: ProxyLogEvent },

    /// Retrieve recent log events for a server.
    #[serde(rename = "get_proxy_logs")]
    GetProxyLogs { server_id: String },

    /// Health check / keepalive.
    #[serde(rename = "ping")]
    Ping,
}

/// All messages from daemon → client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum IpcResponse {
    // Handshake responses
    #[serde(rename = "handshake_ok")]
    HandshakeOk { daemon_version: String },

    #[serde(rename = "handshake_error")]
    HandshakeError {
        daemon_version: String,
        min_client_version: String,
        message: String,
    },

    // Credential responses (flattens the Credential enum)
    #[serde(rename = "credentials")]
    Credentials {
        server_id: String,
        credential: Credential,
    },

    #[serde(rename = "credential_error")]
    CredentialError {
        server_id: String,
        error: CredentialError,
    },

    // Tool filter response
    #[serde(rename = "tool_filter")]
    ToolFilter {
        server_id: String,
        enabled_tools: Vec<String>,
        total_tools: u32,
        token_estimate_filtered: u32,
        token_estimate_full: u32,
    },

    // Server list (bw list)
    #[serde(rename = "server_list")]
    ServerList { servers: Vec<ServerInfo> },

    // Tool list (bw list <server>)
    #[serde(rename = "tool_list")]
    ToolList {
        server_id: String,
        tools: Vec<ToolInfo>,
    },

    // Single tool schema (bw <server> <tool> --help)
    #[serde(rename = "tool_schema")]
    ToolSchema {
        name: String,
        description: String,
        parameters: Vec<ParameterInfo>,
    },

    // Tool call result
    #[serde(rename = "tool_result")]
    ToolResult {
        content: Vec<ContentBlock>,
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        latency_ms: Option<u64>,
    },

    // ─── Acknowledgement responses ──────────────────────────────────────

    /// Generic success acknowledgement (for store/delete/set operations).
    #[serde(rename = "ok")]
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    // Generic error
    #[serde(rename = "error")]
    Error { code: String, message: String },

    /// Recent proxy log events for a server.
    #[serde(rename = "proxy_logs")]
    ProxyLogs {
        server_id: String,
        events: Vec<ProxyLogEvent>,
    },

    /// Pong response to Ping. Includes daemon uptime in seconds.
    #[serde(rename = "pong")]
    Pong { uptime_secs: u64, daemon_version: String },
}

// ─── Daemon → Client push messages ──────────────────────────────────────────

/// Unsolicited messages pushed from daemon to connected clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum IpcPush {
    /// Tool filter changed for a server — proxy should refetch.
    #[serde(rename = "filter_changed")]
    FilterChanged { server_id: String },
}

// ─── Supporting types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerInfo {
    pub server_id: String,
    pub display_name: String,
    pub tool_count: u32,
    pub auth_type: String,
    pub auth_status: String,
    pub token_estimate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParameterInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParameterInfo {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

// ─── Proxy log events ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProxyLogEvent {
    pub timestamp: String,
    pub event_type: ProxyLogEventType,
    pub server_id: String,
    /// MCP client name from the initialize handshake (e.g. "gemini-cli", "claude-code")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyLogEventType {
    Connect,
    Request,
    Response,
    Error,
    Session,
    Disconnect,
}

// ─── Wire format ─────────────────────────────────────────────────────────────

/// Messages on the wire are newline-delimited JSON.
/// Each message is a single line: `<json>\n`

/// Serialize a message to a newline-delimited JSON line.
pub fn encode_message<T: Serialize>(msg: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(msg)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Deserialize a message from a JSON line (with or without trailing newline).
pub fn decode_message<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, serde_json::Error> {
    let trimmed = if data.last() == Some(&b'\n') {
        &data[..data.len() - 1]
    } else {
        data
    };
    serde_json::from_slice(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::{CredentialErrorCode, OAuthCredential};

    // ─── Handshake round-trips ───────────────────────────────────────────

    #[test]
    fn test_handshake_request_roundtrip() {
        let req = IpcRequest::Handshake(HandshakeRequest {
            client: ClientType::Proxy,
            version: "1.0.0".to_string(),
        });
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_handshake_ok_roundtrip() {
        let resp = IpcResponse::HandshakeOk {
            daemon_version: "1.0.0".to_string(),
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn test_handshake_error_roundtrip() {
        let resp = IpcResponse::HandshakeError {
            daemon_version: "2.0.0".to_string(),
            min_client_version: "2.0.0".to_string(),
            message: "Please update".to_string(),
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    // ─── Credential request/response ─────────────────────────────────────

    #[test]
    fn test_get_credentials_request_roundtrip() {
        let req = IpcRequest::GetCredentials {
            server_id: "github-mcp".to_string(),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_credentials_response_oauth() {
        let resp = IpcResponse::Credentials {
            server_id: "github-mcp".to_string(),
            credential: Credential::OAuth(OAuthCredential {
                access_token: "gho_abc123".to_string(),
                url: "https://github-mcp.example.com/mcp".to_string(),
                expires_at: None,
            }),
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn test_credential_error_response() {
        let resp = IpcResponse::CredentialError {
            server_id: "github-mcp".to_string(),
            error: crate::credentials::CredentialError {
                code: CredentialErrorCode::AuthExpired,
                message: "Token expired".to_string(),
            },
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    // ─── Tool filter ─────────────────────────────────────────────────────

    #[test]
    fn test_tool_filter_request_roundtrip() {
        let req = IpcRequest::GetToolFilter {
            server_id: "github".to_string(),
            tool_id: None,
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_tool_filter_response_roundtrip() {
        let resp = IpcResponse::ToolFilter {
            server_id: "github".to_string(),
            enabled_tools: vec!["search_repos".to_string(), "get_repo".to_string()],
            total_tools: 93,
            token_estimate_filtered: 2200,
            token_estimate_full: 55000,
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    // ─── CLI shim messages ───────────────────────────────────────────────

    #[test]
    fn test_list_servers_roundtrip() {
        let req = IpcRequest::ListServers;
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_server_list_response_roundtrip() {
        let resp = IpcResponse::ServerList {
            servers: vec![ServerInfo {
                server_id: "github".to_string(),
                display_name: "GitHub MCP Server".to_string(),
                tool_count: 5,
                auth_type: "oauth".to_string(),
                auth_status: "connected".to_string(),
                token_estimate: 2200,
            }],
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn test_list_tools_roundtrip() {
        let req = IpcRequest::ListTools {
            server_id: "github".to_string(),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_tool_list_response_roundtrip() {
        let resp = IpcResponse::ToolList {
            server_id: "github".to_string(),
            tools: vec![ToolInfo {
                name: "search_repos".to_string(),
                description: "Search GitHub repositories".to_string(),
                parameters: vec![ParameterInfo {
                    name: "query".to_string(),
                    param_type: "string".to_string(),
                    description: "Search query".to_string(),
                    required: true,
                    default: None,
                }],
            }],
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    // ─── Tool call ───────────────────────────────────────────────────────

    #[test]
    fn test_call_tool_roundtrip() {
        let req = IpcRequest::CallTool {
            server_id: "github".to_string(),
            tool_name: "search_repos".to_string(),
            arguments: serde_json::json!({"query": "mcp server", "per_page": 5}),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_tool_result_roundtrip() {
        let resp = IpcResponse::ToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: "Found 42 repositories".to_string(),
            }],
            is_error: false,
            latency_ms: Some(342),
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    // ─── Push messages ───────────────────────────────────────────────────

    #[test]
    fn test_filter_changed_push_roundtrip() {
        let push = IpcPush::FilterChanged {
            server_id: "github".to_string(),
        };
        let bytes = encode_message(&push).unwrap();
        let decoded: IpcPush = decode_message(&bytes).unwrap();
        assert_eq!(push, decoded);
    }

    // ─── Error response ──────────────────────────────────────────────────

    #[test]
    fn test_error_response_roundtrip() {
        let resp = IpcResponse::Error {
            code: "internal".to_string(),
            message: "Something went wrong".to_string(),
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    // ─── Wire format edge cases ──────────────────────────────────────────

    #[test]
    fn test_decode_without_trailing_newline() {
        let req = IpcRequest::ListServers;
        let json = serde_json::to_vec(&req).unwrap();
        // No trailing newline
        let decoded: IpcRequest = decode_message(&json).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_encode_produces_trailing_newline() {
        let req = IpcRequest::ListServers;
        let bytes = encode_message(&req).unwrap();
        assert_eq!(bytes.last(), Some(&b'\n'));
    }

    // ─── JSON shape verification ─────────────────────────────────────────

    #[test]
    fn test_request_action_tag() {
        let req = IpcRequest::GetCredentials {
            server_id: "test".to_string(),
        };
        let json: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(json["action"], "get_credentials");
        assert_eq!(json["server_id"], "test");
    }

    #[test]
    fn test_response_type_tag() {
        let resp = IpcResponse::HandshakeOk {
            daemon_version: "1.0.0".to_string(),
        };
        let json: serde_json::Value = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "handshake_ok");
    }

    // ─── Credential management messages ────────────────────────────────

    #[test]
    fn test_store_credentials_roundtrip() {
        let req = IpcRequest::StoreCredentials {
            server_id: "github".to_string(),
            credential: Credential::ApiKey(crate::credentials::ApiKeyCredential {
                env: [("GITHUB_TOKEN".to_string(), "ghp_xxx".to_string())].into(),
                command: None,
                args: None,
                url: None,
                api_key_injection: None,
            }),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_delete_credentials_roundtrip() {
        let req = IpcRequest::DeleteCredentials {
            server_id: "github".to_string(),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    // ─── Tool filter management messages ────────────────────────────────

    #[test]
    fn test_set_tool_filter_roundtrip() {
        let req = IpcRequest::SetToolFilter {
            server_id: "github".to_string(),
            tool_name: "search_repos".to_string(),
            enabled: false,
            tool_id: Some("claude_code".to_string()),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_set_tool_filter_bulk_roundtrip() {
        let req = IpcRequest::SetToolFilterBulk {
            server_id: "github".to_string(),
            enabled_tools: vec!["search_repos".to_string(), "get_repo".to_string()],
            tool_id: None,
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    // ─── Server registration messages ───────────────────────────────────

    #[test]
    fn test_register_server_roundtrip() {
        let req = IpcRequest::RegisterServer {
            server_id: "github".to_string(),
            display_name: "GitHub MCP".to_string(),
            auth_type: "oauth".to_string(),
            upstream_url: Some("https://github-mcp.example.com".to_string()),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_unregister_server_roundtrip() {
        let req = IpcRequest::UnregisterServer {
            server_id: "github".to_string(),
        };
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    // ─── Ok response ────────────────────────────────────────────────────

    #[test]
    fn test_ok_response_roundtrip() {
        let resp = IpcResponse::Ok {
            message: Some("Credentials stored".to_string()),
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn test_ping_roundtrip() {
        let req = IpcRequest::Ping;
        let bytes = encode_message(&req).unwrap();
        let decoded: IpcRequest = decode_message(&bytes).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_pong_roundtrip() {
        let resp = IpcResponse::Pong {
            uptime_secs: 3600,
            daemon_version: "1.0.0".to_string(),
        };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn test_ok_response_no_message() {
        let resp = IpcResponse::Ok { message: None };
        let bytes = encode_message(&resp).unwrap();
        let decoded: IpcResponse = decode_message(&bytes).unwrap();
        assert_eq!(resp, decoded);
    }

    // ─── All request variants parse from JSON ────────────────────────────

    #[test]
    fn test_all_request_variants_from_json() {
        let cases = vec![
            r#"{"action":"handshake","client":"proxy","version":"1.0.0"}"#,
            r#"{"action":"get_credentials","server_id":"gh"}"#,
            r#"{"action":"get_tool_filter","server_id":"gh"}"#,
            r#"{"action":"list_servers"}"#,
            r#"{"action":"list_tools","server_id":"gh"}"#,
            r#"{"action":"get_tool_schema","server_id":"gh","tool_name":"search"}"#,
            r#"{"action":"call_tool","server_id":"gh","tool_name":"search","arguments":{}}"#,
            r#"{"action":"store_credentials","server_id":"gh","credential":{"type":"none"}}"#,
            r#"{"action":"delete_credentials","server_id":"gh"}"#,
            r#"{"action":"set_tool_filter","server_id":"gh","tool_name":"t","enabled":true}"#,
            r#"{"action":"set_tool_filter","server_id":"gh","tool_name":"t","enabled":true,"tool_id":"claude_code"}"#,
            r#"{"action":"set_tool_filter_bulk","server_id":"gh","enabled_tools":["t1"]}"#,
            r#"{"action":"register_server","server_id":"gh","display_name":"GH","auth_type":"oauth"}"#,
            r#"{"action":"unregister_server","server_id":"gh"}"#,
            r#"{"action":"submit_log","event":{"timestamp":"2026-01-01T00:00:00Z","event_type":"connect","server_id":"gh"}}"#,
            r#"{"action":"get_proxy_logs","server_id":"gh"}"#,
            r#"{"action":"ping"}"#,
        ];
        for json_str in cases {
            let parsed: Result<IpcRequest, _> = serde_json::from_str(json_str);
            assert!(parsed.is_ok(), "Failed to parse: {}", json_str);
        }
    }
}
