//! Brightwing Proxy — transparent MCP proxy with tool filtering.
//!
//! Acts as a stdio MCP server that AI tools (Claude Desktop, Cursor, etc.) spawn.
//! Connects to the Brightwing auth daemon for credentials and tool filters,
//! then proxies MCP traffic to the upstream server.
//!
//! Usage: brightwing-proxy --server <server-id> [--socket <path>] [--verbose]

use proxy_common::credentials::Credential;
use proxy_common::ipc::{
    ClientType, HandshakeRequest, IpcRequest, IpcResponse, decode_message, encode_message,
};
use proxy_common::IPC_PROTOCOL_VERSION;

use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

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
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/Application Support/com.brightwing.mcp-manager/authd.sock")
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join("brightwing-authd.sock")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"\\.\pipe\brightwing-authd")
    }
}

// ─── Daemon IPC client ──────────────────────────────────────────────────────

struct DaemonClient {
    writer: tokio::io::WriteHalf<UnixStream>,
    reader: tokio::io::Lines<BufReader<tokio::io::ReadHalf<UnixStream>>>,
}

impl DaemonClient {
    async fn connect(socket_path: &PathBuf) -> Result<Self, String> {
        let stream = UnixStream::connect(socket_path)
            .await
            .map_err(|e| format!(
                "Cannot connect to Brightwing daemon at {}: {}. Is brightwing-authd running?",
                socket_path.display(), e
            ))?;

        let (reader, writer) = tokio::io::split(stream);
        let reader = BufReader::new(reader).lines();

        Ok(Self { writer, reader })
    }

    async fn send(&mut self, request: &IpcRequest) -> Result<(), String> {
        let bytes = encode_message(request).map_err(|e| format!("Serialize error: {}", e))?;
        self.writer
            .write_all(&bytes)
            .await
            .map_err(|e| format!("Write error: {}", e))
    }

    async fn recv(&mut self) -> Result<IpcResponse, String> {
        let line = self
            .reader
            .next_line()
            .await
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Daemon closed connection".to_string())?;
        decode_message(line.as_bytes()).map_err(|e| format!("Parse error: {}", e))
    }

    async fn handshake(&mut self) -> Result<(), String> {
        self.send(&IpcRequest::Handshake(HandshakeRequest {
            client: ClientType::Proxy,
            version: IPC_PROTOCOL_VERSION.to_string(),
        }))
        .await?;

        match self.recv().await? {
            IpcResponse::HandshakeOk { .. } => Ok(()),
            IpcResponse::HandshakeError { message, .. } => Err(message),
            other => Err(format!("Unexpected handshake response: {:?}", other)),
        }
    }

    async fn get_credentials(&mut self, server_id: &str) -> Result<Credential, String> {
        self.send(&IpcRequest::GetCredentials {
            server_id: server_id.to_string(),
        })
        .await?;

        match self.recv().await? {
            IpcResponse::Credentials { credential, .. } => Ok(credential),
            IpcResponse::CredentialError { error, .. } => {
                Err(format!("{}: {}", error.code.as_str(), error.message))
            }
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }

    async fn get_tool_filter(&mut self, server_id: &str) -> Result<Vec<String>, String> {
        self.send(&IpcRequest::GetToolFilter {
            server_id: server_id.to_string(),
        })
        .await?;

        match self.recv().await? {
            IpcResponse::ToolFilter { enabled_tools, .. } => Ok(enabled_tools),
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }
}

/// Helper to get string representation of credential error codes.
trait CredentialErrorCodeExt {
    fn as_str(&self) -> &str;
}

impl CredentialErrorCodeExt for proxy_common::credentials::CredentialErrorCode {
    fn as_str(&self) -> &str {
        match self {
            Self::NotFound => "not_found",
            Self::AuthExpired => "auth_expired",
            Self::VaultError => "vault_error",
            Self::Internal => "internal",
        }
    }
}

// ─── MCP JSON-RPC proxy ─────────────────────────────────────────────────────

/// Send a single request to the upstream and return the HTTP response.
async fn send_upstream(
    client: &reqwest::Client,
    upstream_url: &str,
    request: &serde_json::Value,
    auth_header: Option<&str>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut req_builder = client.post(upstream_url).json(request);
    if let Some(auth) = auth_header {
        req_builder = req_builder.header("Authorization", auth);
    }
    req_builder.send().await
}

/// Forward MCP traffic between stdin/stdout and an upstream HTTP MCP server.
/// On 401, re-fetches credentials from the daemon and retries once.
async fn run_http_proxy(
    args: &ProxyArgs,
    upstream_url: &str,
    mut auth_header: Option<String>,
    tool_filter: Vec<String>,
    daemon: &mut DaemonClient,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();

    if args.verbose {
        eprintln!(
            "brightwing-proxy: proxying {} → {}",
            args.server_id, upstream_url
        );
    }

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

        if args.verbose {
            if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
                eprintln!("brightwing-proxy: → {}", method);
            }
        }

        // Forward to upstream
        let upstream_response = match send_upstream(
            &client,
            upstream_url,
            &request,
            auth_header.as_deref(),
        )
        .await
        {
            Ok(resp) => resp,
            Err(e) => {
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
                continue;
            }
        };

        // On 401: re-fetch credentials from daemon and retry once
        let upstream_response = if upstream_response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if args.verbose {
                eprintln!("brightwing-proxy: got 401, re-fetching credentials from daemon");
            }
            match daemon.get_credentials(&args.server_id).await {
                Ok(new_cred) => {
                    let new_auth = auth_header_from_credential(&new_cred);
                    auth_header = new_auth.clone();
                    match send_upstream(&client, upstream_url, &request, new_auth.as_deref()).await {
                        Ok(resp) => resp,
                        Err(e) => {
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

        let mut response: serde_json::Value = match upstream_response.json().await {
            Ok(v) => v,
            Err(e) => {
                let error_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32000,
                        "message": format!("Failed to parse upstream response: {}", e)
                    }
                });
                let mut out = serde_json::to_vec(&error_resp).unwrap_or_default();
                out.push(b'\n');
                let _ = stdout.write_all(&out).await;
                continue;
            }
        };

        // Apply tool filter to tools/list responses
        if !tool_filter.is_empty() {
            if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
                if method == "tools/list" {
                    apply_tool_filter(&mut response, &tool_filter);
                }
            }
        }

        if args.verbose {
            if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
                eprintln!("brightwing-proxy: ← {} response", method);
            }
        }

        let mut out = serde_json::to_vec(&response).unwrap_or_default();
        out.push(b'\n');
        if stdout.write_all(&out).await.is_err() {
            break; // AI tool closed stdin
        }
    }

    Ok(())
}

/// Extract an Authorization header from a credential.
fn auth_header_from_credential(credential: &Credential) -> Option<String> {
    match credential {
        Credential::OAuth(oauth) => Some(format!("Bearer {}", oauth.access_token)),
        Credential::ApiKey(api_key) => {
            api_key.env.values().next().map(|v| format!("Bearer {}", v))
        }
        Credential::None => None,
    }
}

/// Filter tools/list response to only include enabled tools.
fn apply_tool_filter(response: &mut serde_json::Value, enabled_tools: &[String]) {
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
    if let Err(e) = daemon.handshake().await {
        eprintln!("brightwing-proxy: handshake failed: {}", e);
        std::process::exit(1);
    }

    if args.verbose {
        eprintln!("brightwing-proxy: connected to daemon");
    }

    // Get credentials
    let credential = match daemon.get_credentials(&args.server_id).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("brightwing-proxy: credential error: {}", e);
            std::process::exit(1);
        }
    };

    // Get tool filter
    let tool_filter = daemon
        .get_tool_filter(&args.server_id)
        .await
        .unwrap_or_default();

    // Extract upstream URL and auth header
    let (upstream_url, auth_header) = match &credential {
        Credential::OAuth(oauth) => {
            (oauth.url.clone(), Some(format!("Bearer {}", oauth.access_token)))
        }
        Credential::ApiKey(api_key) => {
            if let Some(ref url) = api_key.url {
                let auth = api_key.env.values().next().map(|v| format!("Bearer {}", v));
                (url.clone(), auth)
            } else {
                eprintln!(
                    "brightwing-proxy: stdio upstream proxy not yet implemented for {}",
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
    };

    // Start proxying (daemon stays connected for 401 re-auth)
    let result = run_http_proxy(&args, &upstream_url, auth_header, tool_filter, &mut daemon).await;

    if let Err(e) = result {
        eprintln!("brightwing-proxy: {}", e);
        std::process::exit(1);
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
    fn test_parse_args_server_required() {
        // We can't easily test parse_args because it reads std::env::args,
        // but we can test the tool filter logic directly.
    }
}
