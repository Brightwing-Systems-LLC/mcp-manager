//! Brightwing Proxy — transparent MCP proxy with tool filtering.
//!
//! Acts as a stdio MCP server that AI tools (Claude Desktop, Cursor, etc.) spawn.
//! Connects to the Brightwing auth daemon for credentials and tool filters,
//! then proxies MCP traffic to the upstream server.
//!
//! Usage: brightwing-proxy --server <server-id> [--socket <path>] [--verbose]

use proxy_common::credentials::Credential;
use proxy_common::ipc::{
    ClientType, IpcRequest, IpcResponse, ProxyLogEvent, ProxyLogEventType,
};
use proxy_common::transport::DaemonClient;

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// ─── Logging helper ──────────────────────────────────────────────────────────

fn make_log_event(
    server_id: &str,
    client_name: Option<&str>,
    event_type: ProxyLogEventType,
    method: Option<&str>,
    status: Option<&str>,
    error_message: Option<&str>,
    detail: Option<&str>,
) -> ProxyLogEvent {
    ProxyLogEvent {
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        event_type,
        server_id: server_id.to_string(),
        client_name: client_name.map(|s| s.to_string()),
        method: method.map(|s| s.to_string()),
        status: status.map(|s| s.to_string()),
        error_message: error_message.map(|s| s.to_string()),
        detail: detail.map(|s| s.to_string()),
    }
}

/// Fire-and-forget: send a log event to the daemon. Never blocks the proxy.
async fn emit_log(daemon: &mut DaemonClient, event: ProxyLogEvent) {
    let _ = daemon.send(&IpcRequest::SubmitLog { event }).await;
    // Read the Ok response to keep the protocol in sync, but don't block long
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        daemon.recv(),
    ).await;
}

// ─── CLI args ────────────────────────────────────────────────────────────────

struct ProxyArgs {
    server_id: String,
    socket_path: PathBuf,
    verbose: bool,
}

fn parse_args() -> Result<ProxyArgs, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut server_id = None;
    let mut socket_path = None;
    let mut verbose = false;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--server" => {
                i += 1;
                server_id = Some(
                    args.get(i)
                        .ok_or("--server requires a value")?
                        .clone(),
                );
            }
            "--socket" => {
                i += 1;
                socket_path = Some(PathBuf::from(
                    args.get(i).ok_or("--socket requires a value")?,
                ));
            }
            "--verbose" => verbose = true,
            "--help" | "-h" => {
                eprintln!("brightwing-proxy — Brightwing MCP auth proxy");
                eprintln!();
                eprintln!("USAGE:");
                eprintln!("    brightwing-proxy --server <server-id> [OPTIONS]");
                eprintln!();
                eprintln!("OPTIONS:");
                eprintln!("    --server <id>      Server ID to proxy (required)");
                eprintln!("    --socket <path>    Override daemon socket path");
                eprintln!("    --verbose          Print debug info to stderr");
                std::process::exit(0);
            }
            other => return Err(format!("Unknown argument: {}", other)),
        }
        i += 1;
    }

    let server_id = server_id.ok_or("--server is required")?;
    let socket_path = socket_path.unwrap_or_else(default_socket_path);

    Ok(ProxyArgs {
        server_id,
        socket_path,
        verbose,
    })
}

fn default_socket_path() -> PathBuf {
    proxy_common::transport::default_socket_path()
}

// ─── Daemon IPC helpers ─────────────────────────────────────────────────────

async fn get_credentials(daemon: &mut DaemonClient, server_id: &str) -> Result<Credential, String> {
    daemon.send(&IpcRequest::GetCredentials {
        server_id: server_id.to_string(),
    }).await?;

    match daemon.recv().await? {
        IpcResponse::Credentials { credential, .. } => Ok(credential),
        IpcResponse::CredentialError { error, .. } => {
            Err(format!("{:?}: {}", error.code, error.message))
        }
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

async fn get_tool_filter(daemon: &mut DaemonClient, server_id: &str, tool_id: Option<&str>) -> Result<Vec<String>, String> {
    daemon.send(&IpcRequest::GetToolFilter {
        server_id: server_id.to_string(),
        tool_id: tool_id.map(|s| s.to_string()),
    }).await?;

    match daemon.recv().await? {
        IpcResponse::ToolFilter { enabled_tools, .. } => Ok(enabled_tools),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Map MCP client names (from initialize handshake) to Brightwing tool IDs.
fn client_name_to_tool_id(client_name: &str) -> Option<&'static str> {
    let lower = client_name.to_lowercase();
    if lower.contains("claude") && (lower.contains("code") || lower.contains("cli")) {
        Some("claude_code")
    } else if lower.contains("claude") && (lower.contains("desktop") || lower == "claude-ai" || lower == "claude") {
        Some("claude_desktop")
    } else if lower.contains("cursor") {
        Some("cursor")
    } else if lower.contains("codex") || lower.contains("openai") {
        Some("codex")
    } else if lower.contains("windsurf") {
        Some("windsurf")
    } else if lower.contains("gemini") {
        Some("gemini_cli")
    } else if lower.contains("vscode") || lower.contains("visual studio") || lower.contains("copilot") {
        Some("vscode")
    } else {
        None
    }
}

// ─── MCP JSON-RPC proxy ─────────────────────────────────────────────────────

/// Send a single request to the upstream and return the HTTP response.
async fn send_upstream(
    client: &reqwest::Client,
    upstream_url: &str,
    request: &serde_json::Value,
    auth_header: Option<&str>,
    session_id: Option<&str>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut req_builder = client
        .post(upstream_url)
        .header("Accept", "application/json, text/event-stream")
        .json(request);
    if let Some(auth) = auth_header {
        req_builder = req_builder.header("Authorization", auth);
    }
    if let Some(sid) = session_id {
        req_builder = req_builder.header("Mcp-Session-Id", sid);
    }
    req_builder.send().await
}

/// Forward MCP traffic between stdin/stdout and an upstream HTTP MCP server.
/// On 401, re-fetches credentials from the daemon and retries once.
async fn run_http_proxy(
    args: &ProxyArgs,
    upstream_url: &str,
    mut auth_header: Option<String>,
    mut tool_filter: Vec<String>,
    daemon: &mut DaemonClient,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();
    // Track MCP session ID from Streamable HTTP servers
    let mut session_id: Option<String> = None;
    // Track the MCP client name from the initialize handshake
    let mut client_name: Option<String> = None;
    // Whether we've refreshed the tool filter with per-app identity
    let mut filter_refreshed = false;

    if args.verbose {
        eprintln!(
            "brightwing-proxy: proxying {} → {}",
            args.server_id, upstream_url
        );
    }

    // Log: connected
    emit_log(daemon, make_log_event(
        &args.server_id, None, ProxyLogEventType::Connect, None, None, None,
        Some(&format!("Proxying to {}", upstream_url)),
    )).await;

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("brightwing-proxy: invalid JSON from stdin: {}", e);
                continue;
            }
        };

        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("unknown").to_string();

        // Capture client name from the MCP initialize handshake
        if method == "initialize" {
            if let Some(name) = request.pointer("/params/clientInfo/name").and_then(|v| v.as_str()) {
                client_name = Some(name.to_string());
                // Refresh tool filter with per-app identity
                if !filter_refreshed {
                    if let Some(tid) = client_name_to_tool_id(name) {
                        if let Ok(new_filter) = get_tool_filter(daemon, &args.server_id, Some(tid)).await {
                            tool_filter = new_filter;
                            if args.verbose {
                                eprintln!("brightwing-proxy: refreshed tool filter for app '{}'", tid);
                            }
                        }
                    }
                    filter_refreshed = true;
                }
            }
        }

        let cn = client_name.as_deref();

        if args.verbose {
            eprintln!("brightwing-proxy: → {}", method);
        }

        // Log: request
        emit_log(daemon, make_log_event(
            &args.server_id, cn, ProxyLogEventType::Request, Some(&method), None, None, None,
        )).await;

        // Notifications (no "id" field) — fire-and-forget to upstream, no response to stdout
        let is_notification = request.get("id").is_none();

        // Forward to upstream
        let upstream_response = match send_upstream(
            &client,
            upstream_url,
            &request,
            auth_header.as_deref(),
            session_id.as_deref(),
        )
        .await
        {
            Ok(resp) => resp,
            Err(e) => {
                // Log: upstream error
                emit_log(daemon, make_log_event(
                    &args.server_id, cn, ProxyLogEventType::Error, Some(&method), None,
                    Some(&format!("{}", e)), None,
                )).await;
                if !is_notification {
                    let error_resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": {
                            "code": -32000,
                            "message": format!("Upstream server error: {}", e)
                        }
                    });
                    let mut out = serde_json::to_vec(&error_resp).unwrap_or_default();
                    out.push(b'\n');
                    let _ = stdout.write_all(&out).await;
                }
                continue;
            }
        };

        let http_status = upstream_response.status().as_u16().to_string();

        // On 401: re-fetch credentials from daemon and retry once
        let upstream_response = if upstream_response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if args.verbose {
                eprintln!("brightwing-proxy: got 401, re-fetching credentials from daemon");
            }
            // Log: re-auth
            emit_log(daemon, make_log_event(
                &args.server_id, cn, ProxyLogEventType::Session, Some(&method), Some("401"), None,
                Some("Re-fetching credentials"),
            )).await;
            match get_credentials(daemon, &args.server_id).await {
                Ok(new_cred) => {
                    let new_auth = auth_header_from_credential(&new_cred);
                    auth_header = new_auth.clone();
                    match send_upstream(&client, upstream_url, &request, new_auth.as_deref(), session_id.as_deref()).await {
                        Ok(resp) => resp,
                        Err(e) => {
                            emit_log(daemon, make_log_event(
                                &args.server_id, cn, ProxyLogEventType::Error, Some(&method), None,
                                Some(&format!("Retry failed: {}", e)), None,
                            )).await;
                            if !is_notification {
                                let error_resp = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": request.get("id"),
                                    "error": {
                                        "code": -32000,
                                        "message": format!("Upstream retry failed: {}", e)
                                    }
                                });
                                let mut out = serde_json::to_vec(&error_resp).unwrap_or_default();
                                out.push(b'\n');
                                let _ = stdout.write_all(&out).await;
                            }
                            continue;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("brightwing-proxy: credential re-fetch failed: {}", e);
                    // Use the original 401 response
                    upstream_response
                }
            }
        } else {
            upstream_response
        };

        // Capture Mcp-Session-Id from response headers (set by Streamable HTTP servers)
        if let Some(sid) = upstream_response.headers().get("mcp-session-id") {
            if let Ok(sid_str) = sid.to_str() {
                if session_id.is_none() {
                    if args.verbose {
                        eprintln!("brightwing-proxy: acquired session ID");
                    }
                    emit_log(daemon, make_log_event(
                        &args.server_id, cn, ProxyLogEventType::Session, None, None, None,
                        Some("Session established"),
                    )).await;
                }
                session_id = Some(sid_str.to_string());
            }
        }

        // For notifications, don't write a response to stdout
        if is_notification {
            // Consume the response body but don't forward
            let _ = upstream_response.bytes().await;
            if args.verbose {
                eprintln!("brightwing-proxy: ← {} (notification, no response)", method);
            }
            // Log: response for notification
            emit_log(daemon, make_log_event(
                &args.server_id, cn, ProxyLogEventType::Response, Some(&method), Some(&http_status), None,
                Some("notification"),
            )).await;
            continue;
        }

        // Parse response — handle both JSON and SSE (text/event-stream) formats
        let content_type = upstream_response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let mut response: serde_json::Value = if content_type.contains("text/event-stream") {
            // SSE format: look for "data: {...}" lines
            let body_text = match upstream_response.text().await {
                Ok(t) => t,
                Err(e) => {
                    emit_log(daemon, make_log_event(
                        &args.server_id, cn, ProxyLogEventType::Error, Some(&method), Some(&http_status),
                        Some(&format!("Failed to read SSE response: {}", e)), None,
                    )).await;
                    let error_resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": { "code": -32000, "message": format!("Failed to read upstream SSE response: {}", e) }
                    });
                    let mut out = serde_json::to_vec(&error_resp).unwrap_or_default();
                    out.push(b'\n');
                    let _ = stdout.write_all(&out).await;
                    continue;
                }
            };
            // Extract first JSON object from SSE data lines
            let mut parsed = None;
            for line in body_text.lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data.starts_with('{') {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            parsed = Some(v);
                            break;
                        }
                    }
                }
            }
            match parsed {
                Some(v) => v,
                None => {
                    emit_log(daemon, make_log_event(
                        &args.server_id, cn, ProxyLogEventType::Error, Some(&method), Some(&http_status),
                        Some("No JSON-RPC message found in SSE response"), None,
                    )).await;
                    let error_resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": { "code": -32000, "message": "No JSON-RPC message found in upstream SSE response" }
                    });
                    let mut out = serde_json::to_vec(&error_resp).unwrap_or_default();
                    out.push(b'\n');
                    let _ = stdout.write_all(&out).await;
                    continue;
                }
            }
        } else {
            match upstream_response.json().await {
                Ok(v) => v,
                Err(e) => {
                    emit_log(daemon, make_log_event(
                        &args.server_id, cn, ProxyLogEventType::Error, Some(&method), Some(&http_status),
                        Some(&format!("Failed to parse response: {}", e)), None,
                    )).await;
                    let error_resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": { "code": -32000, "message": format!("Failed to parse upstream response: {}", e) }
                    });
                    let mut out = serde_json::to_vec(&error_resp).unwrap_or_default();
                    out.push(b'\n');
                    let _ = stdout.write_all(&out).await;
                    continue;
                }
            }
        };

        // Check if the response is a JSON-RPC error
        let is_error = response.get("error").is_some();

        // Apply tool filter to tools/list responses
        if !tool_filter.is_empty() {
            if method == "tools/list" {
                apply_tool_filter(&mut response, &tool_filter);
            }
        }

        if args.verbose {
            eprintln!("brightwing-proxy: ← {} response", method);
        }

        // Log: response
        if is_error {
            let err_msg = response.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            emit_log(daemon, make_log_event(
                &args.server_id, cn, ProxyLogEventType::Error, Some(&method), Some(&http_status),
                Some(err_msg), None,
            )).await;
        } else {
            emit_log(daemon, make_log_event(
                &args.server_id, cn, ProxyLogEventType::Response, Some(&method), Some(&http_status), None, None,
            )).await;
        }

        let mut out = serde_json::to_vec(&response).unwrap_or_default();
        out.push(b'\n');
        if stdout.write_all(&out).await.is_err() {
            break; // AI tool closed stdin
        }
    }

    // Log: disconnect
    emit_log(daemon, make_log_event(
        &args.server_id, client_name.as_deref(), ProxyLogEventType::Disconnect, None, None, None,
        Some("Client disconnected"),
    )).await;

    Ok(())
}

/// Extract an Authorization header from a credential.
fn auth_header_from_credential(credential: &Credential) -> Option<String> {
    match credential {
        Credential::OAuth(oauth) => Some(format!("Bearer {}", oauth.access_token)),
        Credential::ApiKey(api_key) => {
            // If using query_param injection, don't send a Bearer header
            if let Some(ref injection) = api_key.api_key_injection {
                if injection.starts_with("query_param:") {
                    return None;
                }
            }
            api_key.env.values().next().map(|v| format!("Bearer {}", v))
        }
        Credential::None => None,
    }
}

/// Apply query_param API key injection to a URL.
fn apply_query_param_auth(url: &str, credential: &Credential) -> String {
    if let Credential::ApiKey(api_key) = credential {
        if let Some(ref injection) = api_key.api_key_injection {
            if let Some(param_name) = injection.strip_prefix("query_param:") {
                if let Some(key_value) = api_key.env.values().next() {
                    let sep = if url.contains('?') { "&" } else { "?" };
                    return format!("{}{}{}={}", url, sep, param_name, urlencoding::encode(key_value));
                }
            }
        }
    }
    url.to_string()
}

/// Tool names that are always retained regardless of filter settings.
/// The PatchworkMCP `feedback` tool must survive filtering so agents can
/// report gaps even when the user has narrowed the tool set.
const PROTECTED_TOOLS: &[&str] = &["feedback"];

/// Filter tools/list response to only include enabled tools.
/// Tools in PROTECTED_TOOLS are always retained.
fn apply_tool_filter(response: &mut serde_json::Value, enabled_tools: &[String]) {
    if let Some(result) = response.get_mut("result") {
        if let Some(tools) = result.get_mut("tools") {
            if let Some(arr) = tools.as_array_mut() {
                arr.retain(|tool| {
                    let name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    PROTECTED_TOOLS.contains(&name)
                        || enabled_tools.iter().any(|t| t == name)
                });
            }
        }
    }
}

/// Forward MCP traffic between stdin/stdout and an upstream stdio child process.
/// Injects env vars from the credential store and applies tool filtering.
async fn run_stdio_proxy(
    args: &ProxyArgs,
    command: &str,
    child_args: &[String],
    env: &HashMap<String, String>,
    tool_filter: Vec<String>,
    daemon: &mut DaemonClient,
) -> Result<(), String> {
    use tokio::process::Command;

    if args.verbose {
        eprintln!(
            "brightwing-proxy: stdio mode: {} {}",
            command,
            child_args.join(" ")
        );
    }

    // Spawn the upstream process with injected env vars
    let mut child = Command::new(command)
        .args(child_args)
        .envs(env.iter())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {}", command, e))?;

    let child_stdin = child.stdin.take().ok_or("Failed to open child stdin")?;
    let child_stdout = child.stdout.take().ok_or("Failed to open child stdout")?;

    let mut our_stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut child_reader = BufReader::new(child_stdout).lines();
    let mut child_writer = child_stdin;
    let mut our_stdout = tokio::io::stdout();

    let mut tool_filter = tool_filter;
    let mut filter_refreshed = false;
    loop {
        tokio::select! {
            // Read from our stdin (AI tool) → write to child stdin
            line = our_stdin.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() { continue; }

                        // Capture client name from initialize request for per-app filtering
                        if !filter_refreshed {
                            if let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) {
                                if req.get("method").and_then(|m| m.as_str()) == Some("initialize") {
                                    if let Some(name) = req.pointer("/params/clientInfo/name").and_then(|v| v.as_str()) {
                                        if let Some(tid) = client_name_to_tool_id(name) {
                                            if args.verbose {
                                                eprintln!("brightwing-proxy: client {} → tool_id {}", name, tid);
                                            }
                                            if let Ok(new_filter) = get_tool_filter(daemon, &args.server_id, Some(tid)).await {
                                                if !new_filter.is_empty() {
                                                    tool_filter = new_filter;
                                                }
                                            }
                                        }
                                        filter_refreshed = true;
                                    }
                                }
                            }
                        }

                        if args.verbose {
                            if let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) {
                                if let Some(method) = req.get("method").and_then(|m| m.as_str()) {
                                    eprintln!("brightwing-proxy: → {}", method);
                                }
                            }
                        }
                        let mut bytes = line.into_bytes();
                        bytes.push(b'\n');
                        if child_writer.write_all(&bytes).await.is_err() {
                            break; // Child closed stdin
                        }
                    }
                    Ok(None) => {
                        // AI tool closed stdin — kill child
                        child.kill().await.ok();
                        break;
                    }
                    Err(_) => break,
                }
            }
            // Read from child stdout → write to our stdout (AI tool)
            line = child_reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let mut response: serde_json::Value = match serde_json::from_str(&line) {
                            Ok(v) => v,
                            Err(_) => {
                                // Pass through non-JSON lines
                                let mut bytes = line.into_bytes();
                                bytes.push(b'\n');
                                let _ = our_stdout.write_all(&bytes).await;
                                continue;
                            }
                        };

                        // Apply tool filter to tools/list responses
                        if !tool_filter.is_empty() {
                            if response.get("result").and_then(|r| r.get("tools")).is_some() {
                                apply_tool_filter(&mut response, &tool_filter);
                            }
                        }

                        if args.verbose {
                            eprintln!("brightwing-proxy: ← response");
                        }

                        let mut out = serde_json::to_vec(&response).unwrap_or_default();
                        out.push(b'\n');
                        if our_stdout.write_all(&out).await.is_err() {
                            child.kill().await.ok();
                            break;
                        }
                    }
                    Ok(None) => break, // Child closed stdout
                    Err(_) => break,
                }
            }
            // Child process exited
            status = child.wait() => {
                match status {
                    Ok(s) => {
                        if args.verbose {
                            eprintln!("brightwing-proxy: child exited with {}", s);
                        }
                        if !s.success() {
                            std::process::exit(s.code().unwrap_or(1));
                        }
                    }
                    Err(e) => {
                        eprintln!("brightwing-proxy: child wait error: {}", e);
                        std::process::exit(1);
                    }
                }
                break;
            }
        }
    }

    Ok(())
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("brightwing-proxy: error: {}", e);
            eprintln!("Run with --help for usage.");
            std::process::exit(1);
        }
    };

    // Connect to daemon
    let mut daemon = match DaemonClient::connect(&args.socket_path).await {
        Ok(d) => d,
        Err(e) => {
            // Output a JSON-RPC error so the AI tool gets a clear message
            let error = serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32000,
                    "message": format!("Brightwing daemon not available: {}", e)
                }
            });
            eprintln!("brightwing-proxy: {}", e);
            println!("{}", error);
            std::process::exit(1);
        }
    };

    // Handshake
    if let Err(e) = daemon.handshake(ClientType::Proxy).await {
        eprintln!("brightwing-proxy: handshake failed: {}", e);
        std::process::exit(1);
    }

    if args.verbose {
        eprintln!("brightwing-proxy: connected to daemon");
    }

    // Get credentials
    let credential = match get_credentials(&mut daemon, &args.server_id).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("brightwing-proxy: credential error: {}", e);
            std::process::exit(1);
        }
    };

    // Get tool filter (global default — will be refreshed per-app after initialize handshake)
    let tool_filter = get_tool_filter(&mut daemon, &args.server_id, None)
        .await
        .unwrap_or_default();

    // Determine proxy mode: HTTP or stdio
    match &credential {
        Credential::OAuth(oauth) => {
            let result = run_http_proxy(
                &args,
                &oauth.url,
                Some(format!("Bearer {}", oauth.access_token)),
                tool_filter,
                &mut daemon,
            ).await;
            if let Err(e) = result {
                eprintln!("brightwing-proxy: {}", e);
                std::process::exit(1);
            }
        }
        Credential::ApiKey(api_key) => {
            if let Some(ref url) = api_key.url {
                let effective_url = apply_query_param_auth(url, &credential);
                let auth = auth_header_from_credential(&credential);
                let result = run_http_proxy(&args, &effective_url, auth, tool_filter, &mut daemon).await;
                if let Err(e) = result {
                    eprintln!("brightwing-proxy: {}", e);
                    std::process::exit(1);
                }
            } else if let Some(ref command) = api_key.command {
                let child_args = api_key.args.clone().unwrap_or_default();
                let result = run_stdio_proxy(
                    &args,
                    command,
                    &child_args,
                    &api_key.env,
                    tool_filter,
                    &mut daemon,
                ).await;
                if let Err(e) = result {
                    eprintln!("brightwing-proxy: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!(
                    "brightwing-proxy: server {} has no URL or command configured",
                    args.server_id
                );
                std::process::exit(1);
            }
        }
        Credential::None => {
            eprintln!(
                "brightwing-proxy: server {} has no auth configured",
                args.server_id
            );
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_tool_filter_retains_enabled() {
        let mut response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [
                    { "name": "search_repos", "description": "Search" },
                    { "name": "create_issue", "description": "Create" },
                    { "name": "get_repo", "description": "Get" }
                ]
            }
        });

        let enabled = vec!["search_repos".to_string(), "get_repo".to_string()];
        apply_tool_filter(&mut response, &enabled);

        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "search_repos");
        assert_eq!(tools[1]["name"], "get_repo");
    }

    #[test]
    fn test_apply_tool_filter_empty_filter_removes_all() {
        let mut response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [
                    { "name": "search_repos", "description": "Search" }
                ]
            }
        });

        // Empty enabled list but apply_tool_filter is only called when filter is non-empty
        // in the proxy code. Test the raw function behavior:
        let enabled: Vec<String> = vec![];
        apply_tool_filter(&mut response, &enabled);

        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 0);
    }

    #[test]
    fn test_apply_tool_filter_no_result() {
        let mut response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32601, "message": "Method not found" }
        });

        let enabled = vec!["search_repos".to_string()];
        // Should not panic on error responses
        apply_tool_filter(&mut response, &enabled);
    }

    #[test]
    fn test_apply_tool_filter_retains_feedback_tool() {
        let mut response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [
                    { "name": "search_repos", "description": "Search" },
                    { "name": "feedback", "description": "Report limitations" },
                    { "name": "create_issue", "description": "Create" }
                ]
            }
        });

        // Only enable search_repos — feedback should survive anyway
        let enabled = vec!["search_repos".to_string()];
        apply_tool_filter(&mut response, &enabled);

        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "search_repos");
        assert_eq!(tools[1]["name"], "feedback");
    }

    #[test]
    fn test_parse_args_server_required() {
        // We can't easily test parse_args because it reads std::env::args,
        // but we can test the tool filter logic directly.
    }
}
