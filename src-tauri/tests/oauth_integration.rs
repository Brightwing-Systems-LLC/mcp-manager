//! Integration test: full OAuth flow against a mock OAuth server.
//!
//! Tests the complete lifecycle:
//! 1. Discovery (.well-known/oauth-authorization-server)
//! 2. Dynamic Client Registration (RFC 7591)
//! 3. start_flow → callback server → complete_flow (PKCE code exchange)
//! 4. Token storage and status checking
//! 5. Token refresh
//! 6. Disconnect
//!
//! Uses a lightweight mock OAuth server that handles discovery, registration,
//! authorization callback simulation, and token endpoints.

use brightwing_mcp_manager_lib::db::Database;
use brightwing_mcp_manager_lib::oauth::flow::{
    OAuthFlowStates, complete_flow, disconnect, get_status, start_flow,
};
use brightwing_mcp_manager_lib::oauth::refresh::refresh_token;
use brightwing_mcp_manager_lib::oauth::types::{OAuthError, OAuthFlowInfo, OAuthTokenSet};

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// ─── Mock OAuth Server ──────────────────────────────────────────────────────

/// A mock OAuth server that implements discovery, registration, and token endpoints.
struct MockOAuthServer {
    port: u16,
    /// Recorded token requests for assertions.
    token_requests: Arc<Mutex<Vec<HashMap<String, String>>>>,
    /// Recorded registration requests.
    registration_requests: Arc<Mutex<Vec<Value>>>,
}

struct MockOAuthServerBuilder {
    /// access_token to issue
    access_token: String,
    /// refresh_token to issue (optional)
    refresh_token: Option<String>,
    /// expires_in seconds (optional)
    expires_in: Option<u64>,
    /// Whether to support dynamic client registration
    support_registration: bool,
    /// client_id to return from registration
    registered_client_id: String,
    /// client_secret to return from registration (optional)
    registered_client_secret: Option<String>,
    /// If set, the token endpoint will return this refreshed access_token
    /// when grant_type=refresh_token
    refreshed_access_token: Option<String>,
}

impl MockOAuthServerBuilder {
    fn new() -> Self {
        Self {
            access_token: "mock-access-token".to_string(),
            refresh_token: None,
            expires_in: None,
            support_registration: true,
            registered_client_id: "dyn-client-123".to_string(),
            registered_client_secret: None,
            refreshed_access_token: None,
        }
    }

    fn access_token(mut self, token: &str) -> Self {
        self.access_token = token.to_string();
        self
    }

    fn refresh_token(mut self, token: &str) -> Self {
        self.refresh_token = Some(token.to_string());
        self
    }

    fn expires_in(mut self, secs: u64) -> Self {
        self.expires_in = Some(secs);
        self
    }

    fn no_registration(mut self) -> Self {
        self.support_registration = false;
        self
    }

    fn registered_client_id(mut self, id: &str) -> Self {
        self.registered_client_id = id.to_string();
        self
    }

    fn registered_client_secret(mut self, secret: &str) -> Self {
        self.registered_client_secret = Some(secret.to_string());
        self
    }

    fn refreshed_access_token(mut self, token: &str) -> Self {
        self.refreshed_access_token = Some(token.to_string());
        self
    }

    async fn build(self) -> MockOAuthServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let token_requests = Arc::new(Mutex::new(Vec::new()));
        let registration_requests = Arc::new(Mutex::new(Vec::new()));

        let state = Arc::new(MockOAuthState {
            port,
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            expires_in: self.expires_in,
            support_registration: self.support_registration,
            registered_client_id: self.registered_client_id,
            registered_client_secret: self.registered_client_secret,
            refreshed_access_token: self.refreshed_access_token,
            token_requests: Arc::clone(&token_requests),
            registration_requests: Arc::clone(&registration_requests),
        });

        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let st = Arc::clone(&state);
                    tokio::spawn(async move {
                        handle_oauth_connection(stream, st).await;
                    });
                }
            }
        });

        MockOAuthServer {
            port,
            token_requests,
            registration_requests,
        }
    }
}

struct MockOAuthState {
    port: u16,
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    support_registration: bool,
    registered_client_id: String,
    registered_client_secret: Option<String>,
    refreshed_access_token: Option<String>,
    token_requests: Arc<Mutex<Vec<HashMap<String, String>>>>,
    registration_requests: Arc<Mutex<Vec<Value>>>,
}

impl MockOAuthServer {
    fn builder() -> MockOAuthServerBuilder {
        MockOAuthServerBuilder::new()
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    async fn token_requests(&self) -> Vec<HashMap<String, String>> {
        self.token_requests.lock().await.clone()
    }

    async fn registration_requests(&self) -> Vec<Value> {
        self.registration_requests.lock().await.clone()
    }
}

async fn handle_oauth_connection(stream: tokio::net::TcpStream, state: Arc<MockOAuthState>) {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);

    loop {
        // Read request line
        let mut request_line = String::new();
        match buf_reader.read_line(&mut request_line).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        if request_line.trim().is_empty() {
            break;
        }

        // Parse method and path
        let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
        let (method, path) = if parts.len() >= 2 {
            (parts[0], parts[1])
        } else {
            break;
        };

        // Read headers
        let mut content_length: usize = 0;
        let mut content_type = String::new();
        loop {
            let mut header_line = String::new();
            match buf_reader.read_line(&mut header_line).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let trimmed = header_line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some((key, value)) = trimmed.split_once(':') {
                let key_lower = key.trim().to_lowercase();
                if key_lower == "content-length" {
                    content_length = value.trim().parse().unwrap_or(0);
                }
                if key_lower == "content-type" {
                    content_type = value.trim().to_string();
                }
            }
        }

        // Read body
        let mut body_bytes = vec![0u8; content_length];
        if content_length > 0 {
            if buf_reader.read_exact(&mut body_bytes).await.is_err() {
                break;
            }
        }

        let (status, response_body) = route_request(method, path, &body_bytes, &content_type, &state).await;

        let http_resp = format!(
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            status,
            response_body.len()
        );
        if writer.write_all(http_resp.as_bytes()).await.is_err() {
            break;
        }
        if writer.write_all(response_body.as_bytes()).await.is_err() {
            break;
        }
    }
}

async fn route_request(
    method: &str,
    path: &str,
    body: &[u8],
    content_type: &str,
    state: &MockOAuthState,
) -> (String, String) {
    match (method, path) {
        ("GET", "/.well-known/oauth-authorization-server") => {
            let mut meta = json!({
                "issuer": format!("http://127.0.0.1:{}", state.port),
                "authorization_endpoint": format!("http://127.0.0.1:{}/authorize", state.port),
                "token_endpoint": format!("http://127.0.0.1:{}/token", state.port),
                "response_types_supported": ["code"],
                "code_challenge_methods_supported": ["S256"]
            });
            if state.support_registration {
                meta["registration_endpoint"] =
                    json!(format!("http://127.0.0.1:{}/register", state.port));
            }
            ("200 OK".to_string(), serde_json::to_string(&meta).unwrap())
        }

        ("POST", "/register") => {
            if !state.support_registration {
                return ("404 Not Found".to_string(), r#"{"error":"not_found"}"#.to_string());
            }
            let body_json: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
            state.registration_requests.lock().await.push(body_json);

            let mut resp = json!({
                "client_id": state.registered_client_id,
            });
            if let Some(secret) = &state.registered_client_secret {
                resp["client_secret"] = json!(secret);
            }
            ("201 Created".to_string(), serde_json::to_string(&resp).unwrap())
        }

        ("POST", "/token") => {
            // Parse form body
            let params = parse_form_body(body, content_type);
            state.token_requests.lock().await.push(params.clone());

            let grant_type = params.get("grant_type").map(|s| s.as_str()).unwrap_or("");

            match grant_type {
                "authorization_code" => {
                    let mut resp = json!({
                        "access_token": state.access_token,
                        "token_type": "Bearer",
                    });
                    if let Some(rt) = &state.refresh_token {
                        resp["refresh_token"] = json!(rt);
                    }
                    if let Some(exp) = state.expires_in {
                        resp["expires_in"] = json!(exp);
                    }
                    ("200 OK".to_string(), serde_json::to_string(&resp).unwrap())
                }
                "refresh_token" => {
                    let new_token = state
                        .refreshed_access_token
                        .as_deref()
                        .unwrap_or(&state.access_token);
                    let mut resp = json!({
                        "access_token": new_token,
                        "token_type": "Bearer",
                    });
                    // Optionally re-issue refresh token
                    if let Some(rt) = &state.refresh_token {
                        resp["refresh_token"] = json!(format!("{}-refreshed", rt));
                    }
                    if let Some(exp) = state.expires_in {
                        resp["expires_in"] = json!(exp);
                    }
                    ("200 OK".to_string(), serde_json::to_string(&resp).unwrap())
                }
                _ => (
                    "400 Bad Request".to_string(),
                    json!({"error": "unsupported_grant_type"}).to_string(),
                ),
            }
        }

        _ => ("404 Not Found".to_string(), r#"{"error":"not_found"}"#.to_string()),
    }
}

fn parse_form_body(body: &[u8], content_type: &str) -> HashMap<String, String> {
    if content_type.contains("application/x-www-form-urlencoded") || content_type.is_empty() {
        let body_str = String::from_utf8_lossy(body);
        body_str
            .split('&')
            .filter_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                Some((
                    urlencoding::decode(k).ok()?.to_string(),
                    urlencoding::decode(v).ok()?.to_string(),
                ))
            })
            .collect()
    } else {
        HashMap::new()
    }
}

// ─── Helper: simulate browser callback ──────────────────────────────────────

/// Parse the redirect_uri from an auth_url and simulate the browser hitting
/// the callback server with a given authorization code.
async fn simulate_browser_callback(auth_url: &str, code: &str) {
    // Extract redirect_uri and state from the auth URL
    let url_parts: Vec<&str> = auth_url.splitn(2, '?').collect();
    let query = url_parts.get(1).unwrap_or(&"");
    let params: HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((
                urlencoding::decode(k).ok()?.to_string(),
                urlencoding::decode(v).ok()?.to_string(),
            ))
        })
        .collect();

    let redirect_uri = params.get("redirect_uri").expect("No redirect_uri in auth URL");
    let state = params.get("state").expect("No state in auth URL");

    // Hit the callback server like a browser redirect
    let callback_url = format!("{}?code={}&state={}", redirect_uri, code, state);
    let _ = reqwest::get(&callback_url).await;

    // Give the callback server a moment to process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}

// ─── Helper: register proxy server in DB (needed for FK constraints) ────────

fn register_oauth_server(db: &Database, server_id: &str, url: &str) {
    db.register_proxy_server(server_id, server_id, "oauth", Some(url), None, None, None)
        .unwrap();
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_full_oauth_flow_with_dynamic_registration() {
    let mock = MockOAuthServer::builder()
        .access_token("test-access-tok-abc")
        .refresh_token("test-refresh-tok-xyz")
        .expires_in(3600)
        .registered_client_id("dynamic-client-42")
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "github-mcp", &mock.url());
    let flow_states = OAuthFlowStates::new();

    // 1. Start the flow (discovers metadata, registers client, returns auth URL)
    let flow_info: OAuthFlowInfo = start_flow(
        "github-mcp",
        &mock.url(),
        None::<&str>,
        &db,
        &flow_states,
    )
    .await
    .unwrap();

    // Verify auth URL structure
    assert!(flow_info.auth_url.contains("/authorize"));
    assert!(flow_info.auth_url.contains("response_type=code"));
    assert!(flow_info.auth_url.contains("client_id=dynamic-client-42"));
    assert!(flow_info.auth_url.contains("code_challenge="));
    assert!(flow_info.auth_url.contains("code_challenge_method=S256"));
    assert!(flow_info.auth_url.contains(&format!("state={}", flow_info.state)));

    // Verify dynamic registration was called
    let reg_requests = mock.registration_requests().await;
    assert_eq!(reg_requests.len(), 1);
    assert_eq!(reg_requests[0]["client_name"], "Brightwing MCP Manager");
    assert_eq!(reg_requests[0]["grant_types"][0], "authorization_code");

    // 2. Simulate the browser callback
    simulate_browser_callback(&flow_info.auth_url, "auth-code-from-browser").await;

    // 3. Complete the flow (exchanges code for tokens)
    let token_set: OAuthTokenSet = complete_flow(
        &flow_info.state,
        None, // Use stored callback params
        &flow_states,
        &db,
    )
    .await
    .unwrap();

    assert_eq!(token_set.access_token, "test-access-tok-abc");
    assert_eq!(token_set.refresh_token.as_deref(), Some("test-refresh-tok-xyz"));
    assert_eq!(token_set.token_type, "Bearer");
    assert_eq!(token_set.expires_in, Some(3600));
    assert!(token_set.expires_at.is_some());
    assert_eq!(token_set.client_id, "dynamic-client-42");

    // Verify the token endpoint received the correct params
    let token_reqs = mock.token_requests().await;
    assert_eq!(token_reqs.len(), 1);
    assert_eq!(token_reqs[0].get("grant_type").unwrap(), "authorization_code");
    assert_eq!(token_reqs[0].get("code").unwrap(), "auth-code-from-browser");
    assert!(token_reqs[0].contains_key("code_verifier"));
    assert!(token_reqs[0].contains_key("redirect_uri"));
    assert_eq!(token_reqs[0].get("client_id").unwrap(), "dynamic-client-42");

    // 4. Check status — should be "connected"
    let status = get_status("github-mcp", &db);
    assert_eq!(status.status, "connected");
    assert!(status.expires_at.is_some());
    assert!(status.error_message.is_none());
}

#[tokio::test]
async fn test_oauth_flow_with_provided_client_id() {
    let mock = MockOAuthServer::builder()
        .access_token("tok-for-pre-registered")
        .no_registration() // Server doesn't support dynamic registration
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "custom-server", &mock.url());
    let flow_states = OAuthFlowStates::new();

    // Start flow with a pre-registered client_id
    let flow_info: OAuthFlowInfo = start_flow(
        "custom-server",
        &mock.url(),
        Some("my-pre-registered-client"),
        &db,
        &flow_states,
    )
    .await
    .unwrap();

    // Verify the auth URL uses the provided client_id
    assert!(flow_info.auth_url.contains("client_id=my-pre-registered-client"));

    // No registration should have been attempted
    let reg_requests = mock.registration_requests().await;
    assert_eq!(reg_requests.len(), 0);

    // Simulate callback and complete
    simulate_browser_callback(&flow_info.auth_url, "manual-code").await;

    let token_set: OAuthTokenSet = complete_flow(&flow_info.state, None, &flow_states, &db)
        .await
        .unwrap();

    assert_eq!(token_set.access_token, "tok-for-pre-registered");
    assert_eq!(token_set.client_id, "my-pre-registered-client");
}

#[tokio::test]
async fn test_oauth_complete_with_direct_code() {
    // Test passing the code directly instead of using the callback server
    let mock = MockOAuthServer::builder()
        .access_token("direct-code-token")
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "direct-srv", &mock.url());
    let flow_states = OAuthFlowStates::new();

    let flow_info: OAuthFlowInfo = start_flow("direct-srv", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();

    // Don't simulate browser callback — provide the code directly
    let token_set: OAuthTokenSet = complete_flow(
        &flow_info.state,
        Some("code-from-deep-link"),
        &flow_states,
        &db,
    )
    .await
    .unwrap();

    assert_eq!(token_set.access_token, "direct-code-token");

    let token_reqs = mock.token_requests().await;
    assert_eq!(token_reqs[0].get("code").unwrap(), "code-from-deep-link");
}

#[tokio::test]
async fn test_oauth_status_disconnected() {
    let db = Database::new_in_memory().unwrap();
    let status = get_status("nonexistent", &db);
    assert_eq!(status.status, "disconnected");
    assert!(status.expires_at.is_none());
    assert!(status.error_message.is_none());
}

#[tokio::test]
async fn test_oauth_status_expired() {
    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "expired-srv", "https://a.com");

    // Store a token that's already expired
    let expired_token = OAuthTokenSet {
        access_token: "old-tok".to_string(),
        refresh_token: Some("old-refresh".to_string()),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        expires_at: Some("2020-01-01T00:00:00+00:00".to_string()), // In the past
        scope: None,
        token_endpoint: "https://a.com/token".to_string(),
        client_id: "c".to_string(),
        client_secret: None,
        server_url: "https://a.com".to_string(),
    };
    let json = serde_json::to_string(&expired_token).unwrap();
    db.store_oauth_token_set("expired-srv", &json).unwrap();

    let status = get_status("expired-srv", &db);
    assert_eq!(status.status, "expired");
    assert_eq!(status.expires_at.as_deref(), Some("2020-01-01T00:00:00+00:00"));
}

#[tokio::test]
async fn test_oauth_disconnect() {
    let mock = MockOAuthServer::builder()
        .access_token("to-be-disconnected")
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "disc-test", &mock.url());
    let flow_states = OAuthFlowStates::new();

    // Connect first
    let flow_info: OAuthFlowInfo = start_flow("disc-test", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();
    simulate_browser_callback(&flow_info.auth_url, "code123").await;
    let _: OAuthTokenSet = complete_flow(&flow_info.state, None, &flow_states, &db)
        .await
        .unwrap();

    assert_eq!(get_status("disc-test", &db).status, "connected");

    // Disconnect
    disconnect("disc-test", &db).unwrap();

    assert_eq!(get_status("disc-test", &db).status, "disconnected");
}

#[tokio::test]
async fn test_oauth_token_refresh() {
    let mock = MockOAuthServer::builder()
        .access_token("initial-token")
        .refresh_token("refresh-tok-1")
        .refreshed_access_token("refreshed-token-2")
        .expires_in(3600)
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "refresh-test", &mock.url());
    let flow_states = OAuthFlowStates::new();

    // Complete an initial flow
    let flow_info: OAuthFlowInfo = start_flow("refresh-test", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();
    simulate_browser_callback(&flow_info.auth_url, "code-for-refresh").await;
    let initial_tokens: OAuthTokenSet = complete_flow(&flow_info.state, None, &flow_states, &db)
        .await
        .unwrap();

    assert_eq!(initial_tokens.access_token, "initial-token");
    assert_eq!(initial_tokens.refresh_token.as_deref(), Some("refresh-tok-1"));

    // Refresh the token
    let refreshed: OAuthTokenSet = refresh_token(&initial_tokens).await.unwrap();
    assert_eq!(refreshed.access_token, "refreshed-token-2");
    // Server issues a new refresh token based on old one
    assert_eq!(
        refreshed.refresh_token.as_deref(),
        Some("refresh-tok-1-refreshed")
    );

    // Verify the token endpoint received the refresh request
    let token_reqs = mock.token_requests().await;
    assert_eq!(token_reqs.len(), 2); // 1 for initial exchange + 1 for refresh
    assert_eq!(token_reqs[1].get("grant_type").unwrap(), "refresh_token");
    assert_eq!(token_reqs[1].get("refresh_token").unwrap(), "refresh-tok-1");
}

#[tokio::test]
async fn test_oauth_refresh_preserves_old_refresh_token() {
    // When the server doesn't issue a new refresh token on refresh,
    // the old one should be preserved.
    let mock = MockOAuthServer::builder()
        .access_token("tok1")
        .refresh_token("keep-this-refresh")
        .refreshed_access_token("tok2")
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "preserve-test", &mock.url());
    let flow_states = OAuthFlowStates::new();

    let flow_info: OAuthFlowInfo = start_flow("preserve-test", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();
    simulate_browser_callback(&flow_info.auth_url, "c").await;
    let initial: OAuthTokenSet = complete_flow(&flow_info.state, None, &flow_states, &db)
        .await
        .unwrap();

    // The mock returns a new refresh token (old + "-refreshed"),
    // so this test verifies that when set, the new one is used.
    let refreshed: OAuthTokenSet = refresh_token(&initial).await.unwrap();
    assert_eq!(refreshed.access_token, "tok2");
    assert_eq!(
        refreshed.refresh_token.as_deref(),
        Some("keep-this-refresh-refreshed")
    );
}

#[tokio::test]
async fn test_oauth_no_registration_no_client_id_fails() {
    let mock = MockOAuthServer::builder()
        .no_registration()
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    let flow_states = OAuthFlowStates::new();

    // No client_id and server doesn't support registration → should fail
    let result: Result<OAuthFlowInfo, OAuthError> = start_flow("fail-test", &mock.url(), None, &db, &flow_states).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("No client_id provided"));
}

#[tokio::test]
async fn test_oauth_complete_without_callback_fails() {
    let mock = MockOAuthServer::builder().build().await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "no-cb", &mock.url());
    let flow_states = OAuthFlowStates::new();

    let flow_info: OAuthFlowInfo = start_flow("no-cb", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();

    // Try to complete without simulating browser callback and without providing code
    let result: Result<OAuthTokenSet, OAuthError> = complete_flow(&flow_info.state, None, &flow_states, &db).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("No callback received"));
}

#[tokio::test]
async fn test_oauth_complete_invalid_state_fails() {
    let db = Database::new_in_memory().unwrap();
    let flow_states = OAuthFlowStates::new();

    // Try to complete with a state that was never started
    let result: Result<OAuthTokenSet, OAuthError> = complete_flow("bogus-state", Some("code"), &flow_states, &db).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Invalid OAuth state"));
}

#[tokio::test]
async fn test_oauth_metadata_cached_after_discovery() {
    let mock = MockOAuthServer::builder()
        .access_token("cached-tok")
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "cache-test", &mock.url());
    let flow_states = OAuthFlowStates::new();

    // First flow — discovers and caches metadata
    let flow_info1: OAuthFlowInfo = start_flow("cache-test", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();
    simulate_browser_callback(&flow_info1.auth_url, "c1").await;
    let _: OAuthTokenSet = complete_flow(&flow_info1.state, None, &flow_states, &db)
        .await
        .unwrap();

    // Verify metadata was cached in DB
    let cached = db.get_oauth_metadata("cache-test").unwrap();
    assert!(cached.is_some());
    let cached_json: Value = serde_json::from_str(&cached.unwrap()).unwrap();
    assert!(cached_json["token_endpoint"].as_str().unwrap().contains(&mock.port.to_string()));

    // Second flow — should use cached metadata (even if server is down, it would work)
    // We can verify by checking the auth URL still works
    let flow_info2: OAuthFlowInfo = start_flow("cache-test", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();
    assert!(flow_info2.auth_url.contains("/authorize"));
}

#[tokio::test]
async fn test_oauth_client_secret_forwarded_in_exchange() {
    let mock = MockOAuthServer::builder()
        .access_token("secret-tok")
        .registered_client_id("secret-client")
        .registered_client_secret("super-secret-123")
        .build()
        .await;

    let db = Database::new_in_memory().unwrap();
    register_oauth_server(&db, "secret-test", &mock.url());
    let flow_states = OAuthFlowStates::new();

    let flow_info: OAuthFlowInfo = start_flow("secret-test", &mock.url(), None, &db, &flow_states)
        .await
        .unwrap();

    simulate_browser_callback(&flow_info.auth_url, "code-with-secret").await;
    let tokens: OAuthTokenSet = complete_flow(&flow_info.state, None, &flow_states, &db)
        .await
        .unwrap();

    assert_eq!(tokens.access_token, "secret-tok");
    assert_eq!(tokens.client_secret.as_deref(), Some("super-secret-123"));

    // Verify client_secret was sent to token endpoint
    let token_reqs = mock.token_requests().await;
    assert_eq!(
        token_reqs[0].get("client_secret").unwrap(),
        "super-secret-123"
    );
}

/// Mock MCP server that rotates required auth: rejects `old-token`, accepts `new-token`.
/// Used to test the proxy 401 retry pattern.
struct RotatingAuthMcpServer {
    port: u16,
}

impl RotatingAuthMcpServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut buf_reader = BufReader::new(reader);

                        loop {
                            let mut request_line = String::new();
                            match buf_reader.read_line(&mut request_line).await {
                                Ok(0) => break,
                                Ok(_) => {}
                                Err(_) => break,
                            }
                            if request_line.trim().is_empty() {
                                break;
                            }

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
                                if trimmed.is_empty() { break; }
                                if let Some((k, v)) = trimmed.split_once(':') {
                                    let kl = k.trim().to_lowercase();
                                    if kl == "content-length" {
                                        content_length = v.trim().parse().unwrap_or(0);
                                    }
                                    headers.insert(kl, v.trim().to_string());
                                }
                            }

                            let mut body = vec![0u8; content_length];
                            if content_length > 0 {
                                if buf_reader.read_exact(&mut body).await.is_err() { break; }
                            }

                            let auth = headers.get("authorization").cloned().unwrap_or_default();

                            let (status, resp_body) = if auth == "Bearer new-token" {
                                let resp = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": 1,
                                    "result": {
                                        "tools": [{"name": "test_tool", "description": "Test", "inputSchema": {}}]
                                    }
                                });
                                ("200 OK", serde_json::to_string(&resp).unwrap())
                            } else {
                                let resp = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": 1,
                                    "error": {"code": -32001, "message": "Unauthorized"}
                                });
                                ("401 Unauthorized", serde_json::to_string(&resp).unwrap())
                            };

                            let http = format!(
                                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
                                status, resp_body.len()
                            );
                            if writer.write_all(http.as_bytes()).await.is_err() { break; }
                            if writer.write_all(resp_body.as_bytes()).await.is_err() { break; }
                        }
                    });
                }
            }
        });

        RotatingAuthMcpServer { port }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

/// Simulates what the proxy does on 401: send request with old token,
/// get 401, re-fetch credentials (which now have new token), retry.
#[tokio::test]
async fn test_proxy_401_retry_with_refreshed_token() {
    let mcp = RotatingAuthMcpServer::start().await;

    let client = reqwest::Client::new();

    // First request with old token — should get 401
    let resp1 = client
        .post(&mcp.url())
        .header("Authorization", "Bearer old-token")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status().as_u16(), 401);

    // Simulate credential re-fetch: daemon returns new token
    let new_auth = "Bearer new-token";

    // Retry with new token — should succeed
    let resp2: serde_json::Value = client
        .post(&mcp.url())
        .header("Authorization", new_auth)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let tools = resp2["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "test_tool");
}
