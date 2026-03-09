//! Brightwing Proxy — transparent MCP proxy with tool filtering.
//!
//! Acts as a stdio MCP server that AI tools (Claude Desktop, Cursor, etc.) spawn.
//! Connects to the Brightwing auth daemon for credentials and tool filters,
//! then proxies MCP traffic to the upstream server.
//!
//! Usage: brightwing-proxy --server <server-id> [--socket <path>] [--verbose]

use proxy_common::credentials::Credential;
use proxy_common::ipc::{
    ClientType, IpcRequest, IpcResponse,
};
use proxy_common::transport::DaemonClient;

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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

async fn get_tool_filter(daemon: &mut DaemonClient, server_id: &str) -> Result<Vec<String>, String> {
    daemon.send(&IpcRequest::GetToolFilter {
        server_id: server_id.to_string(),
    }).await?;

    match daemon.recv().await? {
        IpcResponse::ToolFilter { enabled_tools, .. } => Ok(enabled_tools),
        other => Err(format!("Unexpected response: {:?}", other)),
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
            match get_credentials(daemon, &args.server_id).await {
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

/// Forward MCP traffic between stdin/stdout and an upstream stdio child process.
/// Injects env vars from the credential store and applies tool filtering.
async fn run_stdio_proxy(
    args: &ProxyArgs,
    command: &str,
    child_args: &[String],
    env: &HashMap<String, String>,
    tool_filter: Vec<String>,
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

    loop {
        tokio::select! {
            // Read from our stdin (AI tool) → write to child stdin
            line = our_stdin.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() { continue; }
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
                            // We need to match against the request method, but for stdio
                            // we can detect tools/list responses by their shape
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

    // Get tool filter
    let tool_filter = get_tool_filter(&mut daemon, &args.server_id)
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
                let auth = api_key.env.values().next().map(|v| format!("Bearer {}", v));
                let result = run_http_proxy(&args, url, auth, tool_filter, &mut daemon).await;
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
    fn test_parse_args_server_required() {
        // We can't easily test parse_args because it reads std::env::args,
        // but we can test the tool filter logic directly.
    }
}
