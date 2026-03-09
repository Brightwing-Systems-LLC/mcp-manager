//! Mock MCP Server for integration testing.
//!
//! A lightweight HTTP server that implements just enough of the MCP protocol
//! to test the proxy. Supports configurable tools, auth requirements, and
//! canned responses.
//!
//! Usage:
//! ```rust,no_run
//! let server = MockMcpServer::builder()
//!     .with_tool("search_repos", "Search GitHub repositories", json!({
//!         "type": "object",
//!         "properties": { "query": { "type": "string" } },
//!         "required": ["query"]
//!     }))
//!     .with_tool_response("search_repos", json!([{"type": "text", "text": "Found 42 repos"}]))
//!     .require_auth("Bearer test-token")
//!     .build_http()
//!     .await;
//!
//! // server.url() returns "http://127.0.0.1:{port}"
//! // server.port() returns the bound port
//! ```

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// ─── Builder ─────────────────────────────────────────────────────────────────

pub struct MockMcpServerBuilder {
    tools: Vec<MockTool>,
    tool_responses: HashMap<String, Value>,
    required_auth: Option<String>,
    server_name: String,
    server_version: String,
}

struct MockTool {
    name: String,
    description: String,
    input_schema: Value,
}

impl MockMcpServerBuilder {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            tool_responses: HashMap::new(),
            required_auth: None,
            server_name: "mock-mcp-server".to_string(),
            server_version: "1.0.0".to_string(),
        }
    }

    pub fn with_tool(mut self, name: &str, description: &str, input_schema: Value) -> Self {
        self.tools.push(MockTool {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
        });
        self
    }

    pub fn with_tool_response(mut self, tool_name: &str, content: Value) -> Self {
        self.tool_responses
            .insert(tool_name.to_string(), content);
        self
    }

    pub fn require_auth(mut self, expected: &str) -> Self {
        self.required_auth = Some(expected.to_string());
        self
    }

    #[allow(dead_code)]
    pub fn server_name(mut self, name: &str) -> Self {
        self.server_name = name.to_string();
        self
    }

    pub async fn build_http(self) -> MockMcpServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let state = Arc::new(MockServerState {
            tools: self.tools,
            tool_responses: self.tool_responses,
            required_auth: self.required_auth,
            server_name: self.server_name,
            server_version: self.server_version,
            requests: Mutex::new(Vec::new()),
        });

        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let state = Arc::clone(&state_clone);
                    tokio::spawn(async move {
                        handle_http_connection(stream, state).await;
                    });
                }
            }
        });

        MockMcpServer { port, state }
    }
}

// ─── Server state ────────────────────────────────────────────────────────────

struct MockServerState {
    tools: Vec<MockTool>,
    tool_responses: HashMap<String, Value>,
    required_auth: Option<String>,
    server_name: String,
    server_version: String,
    requests: Mutex<Vec<RecordedRequest>>,
}

#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub body: Value,
    pub auth_header: Option<String>,
}

pub struct MockMcpServer {
    port: u16,
    state: Arc<MockServerState>,
}

impl MockMcpServer {
    pub fn builder() -> MockMcpServerBuilder {
        MockMcpServerBuilder::new()
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get all recorded requests for assertions.
    pub async fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.state.requests.lock().await.clone()
    }
}

// ─── HTTP handling ───────────────────────────────────────────────────────────

async fn handle_http_connection(
    stream: tokio::net::TcpStream,
    state: Arc<MockServerState>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    // Simple HTTP/1.1 request parsing loop (handles keep-alive)
    loop {
        // Read request line
        let mut request_line = String::new();
        match buf_reader.read_line(&mut request_line).await {
            Ok(0) => break, // Connection closed
            Ok(_) => {}
            Err(_) => break,
        }

        if request_line.trim().is_empty() {
            break;
        }

        // Read headers
        let mut headers = HashMap::new();
        let mut content_length: usize = 0;
        loop {
            let mut header_line = String::new();
            match buf_reader.read_line(&mut header_line).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let trimmed = header_line.trim();
            if trimmed.is_empty() {
                break; // End of headers
            }
            if let Some((key, value)) = trimmed.split_once(':') {
                let key_lower = key.trim().to_lowercase();
                let value_trimmed = value.trim().to_string();
                if key_lower == "content-length" {
                    content_length = value_trimmed.parse().unwrap_or(0);
                }
                headers.insert(key_lower, value_trimmed);
            }
        }

        // Read body
        let mut body_bytes = vec![0u8; content_length];
        if content_length > 0 {
            use tokio::io::AsyncReadExt;
            if buf_reader.read_exact(&mut body_bytes).await.is_err() {
                break;
            }
        }

        let body: Value = if content_length > 0 {
            serde_json::from_slice(&body_bytes).unwrap_or(Value::Null)
        } else {
            Value::Null
        };

        let auth_header = headers.get("authorization").cloned();

        // Record the request
        {
            let method = body
                .get("method")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            state.requests.lock().await.push(RecordedRequest {
                method,
                body: body.clone(),
                auth_header: auth_header.clone(),
            });
        }

        // Check auth
        if let Some(ref expected) = state.required_auth {
            match &auth_header {
                Some(provided) if provided == expected => {} // OK
                _ => {
                    let error_resp = json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "error": {
                            "code": -32001,
                            "message": "Unauthorized"
                        }
                    });
                    let resp_body = serde_json::to_vec(&error_resp).unwrap();
                    let http_resp = format!(
                        "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
                        resp_body.len()
                    );
                    let _ = writer.write_all(http_resp.as_bytes()).await;
                    let _ = writer.write_all(&resp_body).await;
                    continue;
                }
            }
        }

        // Handle JSON-RPC methods
        let response = handle_jsonrpc(&body, &state);
        let resp_body = serde_json::to_vec(&response).unwrap();

        let http_resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            resp_body.len()
        );
        if writer.write_all(http_resp.as_bytes()).await.is_err() {
            break;
        }
        if writer.write_all(&resp_body).await.is_err() {
            break;
        }
    }
}

fn handle_jsonrpc(request: &Value, state: &MockServerState) -> Value {
    let method = request
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let id = request.get("id").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": state.server_name,
                        "version": state.server_version
                    }
                }
            })
        }

        "notifications/initialized" => {
            // Notification, no response needed but we return empty for HTTP
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            })
        }

        "ping" => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            })
        }

        "tools/list" => {
            let tools: Vec<Value> = state
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema
                    })
                })
                .collect();

            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": tools
                }
            })
        }

        "tools/call" => {
            let tool_name = request
                .get("params")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");

            if let Some(content) = state.tool_responses.get(tool_name) {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": content,
                        "isError": false
                    }
                })
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Unknown tool: {}", tool_name)
                    }
                })
            }
        }

        _ => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            })
        }
    }
}

// ─── Tests for the mock server itself ────────────────────────────────────────

#[tokio::test]
async fn test_mock_server_initialize() {
    let server = MockMcpServer::builder()
        .build_http()
        .await;

    let client = reqwest::Client::new();
    let resp: Value = client
        .post(&server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1.0" }
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["result"]["serverInfo"]["name"], "mock-mcp-server");
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
}

#[tokio::test]
async fn test_mock_server_tools_list() {
    let server = MockMcpServer::builder()
        .with_tool(
            "search_repos",
            "Search GitHub repositories",
            json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            }),
        )
        .with_tool(
            "get_repo",
            "Get repository details",
            json!({
                "type": "object",
                "properties": { "owner": { "type": "string" }, "repo": { "type": "string" } },
                "required": ["owner", "repo"]
            }),
        )
        .build_http()
        .await;

    let client = reqwest::Client::new();
    let resp: Value = client
        .post(&server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "search_repos");
    assert_eq!(tools[1]["name"], "get_repo");
}

#[tokio::test]
async fn test_mock_server_tool_call() {
    let server = MockMcpServer::builder()
        .with_tool(
            "search_repos",
            "Search",
            json!({"type": "object", "properties": {}}),
        )
        .with_tool_response(
            "search_repos",
            json!([{"type": "text", "text": "Found 42 repositories"}]),
        )
        .build_http()
        .await;

    let client = reqwest::Client::new();
    let resp: Value = client
        .post(&server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "search_repos",
                "arguments": { "query": "mcp" }
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["result"]["isError"], false);
    let content = resp["result"]["content"].as_array().unwrap();
    assert_eq!(content[0]["text"], "Found 42 repositories");
}

#[tokio::test]
async fn test_mock_server_unknown_tool() {
    let server = MockMcpServer::builder().build_http().await;

    let client = reqwest::Client::new();
    let resp: Value = client
        .post(&server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": { "name": "nonexistent", "arguments": {} }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(resp["error"]["message"]
        .as_str()
        .unwrap()
        .contains("nonexistent"));
}

#[tokio::test]
async fn test_mock_server_auth_required() {
    let server = MockMcpServer::builder()
        .require_auth("Bearer test-token-123")
        .with_tool(
            "search",
            "Search",
            json!({"type": "object", "properties": {}}),
        )
        .build_http()
        .await;

    let client = reqwest::Client::new();

    // Without auth — should fail
    let resp = client
        .post(&server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);

    // With wrong auth — should fail
    let resp = client
        .post(&server.url())
        .header("Authorization", "Bearer wrong-token")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);

    // With correct auth — should succeed
    let resp: Value = client
        .post(&server.url())
        .header("Authorization", "Bearer test-token-123")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
}

#[tokio::test]
async fn test_mock_server_records_requests() {
    let server = MockMcpServer::builder()
        .require_auth("Bearer tok")
        .build_http()
        .await;

    let client = reqwest::Client::new();
    let _ = client
        .post(&server.url())
        .header("Authorization", "Bearer tok")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap();

    let requests = server.recorded_requests().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "tools/list");
    assert_eq!(
        requests[0].auth_header.as_deref(),
        Some("Bearer tok")
    );
}

#[tokio::test]
async fn test_mock_server_ping() {
    let server = MockMcpServer::builder().build_http().await;

    let client = reqwest::Client::new();
    let resp: Value = client
        .post(&server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "ping",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["id"], 99);
    assert!(resp["result"].is_object());
}

#[tokio::test]
async fn test_mock_server_unknown_method() {
    let server = MockMcpServer::builder().build_http().await;

    let client = reqwest::Client::new();
    let resp: Value = client
        .post(&server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "nonexistent/method",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["error"]["code"], -32601);
}

#[tokio::test]
async fn test_mock_server_multiple_requests_one_connection() {
    let server = MockMcpServer::builder()
        .with_tool(
            "tool_a",
            "Tool A",
            json!({"type": "object", "properties": {}}),
        )
        .with_tool_response("tool_a", json!([{"type": "text", "text": "Result A"}]))
        .build_http()
        .await;

    let client = reqwest::Client::new();

    // reqwest uses connection pooling, so these should reuse the connection
    for i in 0..5 {
        let resp: Value = client
            .post(&server.url())
            .json(&json!({
                "jsonrpc": "2.0",
                "id": i,
                "method": "tools/call",
                "params": { "name": "tool_a", "arguments": {} }
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        assert_eq!(resp["result"]["content"][0]["text"], "Result A");
    }

    assert_eq!(server.recorded_requests().await.len(), 5);
}
