//! Brightwing CLI Shim — exposes MCP servers as shell commands.
//!
//! Usage:
//!   bw list                              # All servers
//!   bw list <server>                     # Tools on a server
//!   bw <server> <tool> --help            # Usage for a tool
//!   bw <server> <tool> --key value       # Call a tool
//!   bw <server> <tool> --key value --json # Raw JSON output

use std::collections::HashMap;
use std::path::PathBuf;

use proxy_common::ipc::{
    ClientType, HandshakeRequest, IpcRequest, IpcResponse, decode_message, encode_message,
};
use proxy_common::IPC_PROTOCOL_VERSION;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream;

// ─── Parsed command ──────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum Command {
    Help,
    List { server_id: Option<String> },
    ToolHelp { server_id: String, tool_name: String },
    CallTool {
        server_id: String,
        tool_name: String,
        arguments: HashMap<String, serde_json::Value>,
        json_output: bool,
        verbose: bool,
    },
}

/// Parse CLI arguments into a Command.
fn parse_command(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Help);
    }

    match args[0].as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),

        "list" => {
            let server_id = args.get(1).cloned();
            Ok(Command::List { server_id })
        }

        _ => {
            // <server> <tool> [--params]
            let server_id = args[0].clone();

            if args.len() < 2 {
                // `bw <server>` is equivalent to `bw list <server>`
                return Ok(Command::List {
                    server_id: Some(server_id),
                });
            }

            let tool_name = args[1].clone();
            let param_args = &args[2..];

            // Check for --help flag
            if param_args.iter().any(|a| a == "--help" || a == "-h") {
                return Ok(Command::ToolHelp {
                    server_id,
                    tool_name,
                });
            }

            let (arguments, json_output, verbose) = parse_tool_params(param_args)?;

            Ok(Command::CallTool {
                server_id,
                tool_name,
                arguments,
                json_output,
                verbose,
            })
        }
    }
}

/// Parse --key value pairs into a HashMap with type coercion.
///
/// Coercion rules:
/// - `--flag` (no value or next arg is another flag) → bool true
/// - `--key 123` → integer
/// - `--key 12.5` → float
/// - `--key true` / `--key false` → bool
/// - `--key value` → string (fallback)
fn parse_tool_params(
    args: &[String],
) -> Result<(HashMap<String, serde_json::Value>, bool, bool), String> {
    let mut params = HashMap::new();
    let mut json_output = false;
    let mut verbose = false;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        if !arg.starts_with("--") {
            return Err(format!("Unexpected argument '{}'. Use --key value format.", arg));
        }

        let key = arg.trim_start_matches('-').to_string();

        // Handle global flags
        match key.as_str() {
            "json" => {
                json_output = true;
                i += 1;
                continue;
            }
            "verbose" => {
                verbose = true;
                i += 1;
                continue;
            }
            _ => {}
        }

        // Check if next arg exists and isn't another flag
        let has_value = i + 1 < args.len() && !args[i + 1].starts_with("--");

        if has_value {
            let value_str = &args[i + 1];
            let value = coerce_value(value_str);
            params.insert(key, value);
            i += 2;
        } else {
            // Boolean flag
            params.insert(key, serde_json::Value::Bool(true));
            i += 1;
        }
    }

    Ok((params, json_output, verbose))
}

/// Coerce a string value to the most specific JSON type.
fn coerce_value(s: &str) -> serde_json::Value {
    // Try bool
    if s == "true" {
        return serde_json::Value::Bool(true);
    }
    if s == "false" {
        return serde_json::Value::Bool(false);
    }

    // Try integer
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }

    // Try float
    if let Ok(f) = s.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return serde_json::Value::Number(n);
        }
    }

    // Fallback to string
    serde_json::Value::String(s.to_string())
}

// ─── Output formatting ──────────────────────────────────────────────────────

/// Format MCP tool result content blocks as human-readable text.
fn format_tool_result(content: &[serde_json::Value], is_error: bool) -> String {
    let mut output = String::new();

    for block in content {
        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(text);
        }
    }

    if is_error && !output.is_empty() {
        format!("Error: {}", output)
    } else {
        output
    }
}

/// Format a server list for human-readable output.
fn format_server_list(
    servers: &[serde_json::Value],
) -> String {
    if servers.is_empty() {
        return "No servers connected.\n\nUse the Brightwing app to add MCP servers.".to_string();
    }

    let mut out = format!(
        "Brightwing Tool Broker — {} server{} connected\n\n",
        servers.len(),
        if servers.len() == 1 { "" } else { "s" }
    );

    for server in servers {
        let id = server.get("server_id").and_then(|v| v.as_str()).unwrap_or("?");
        let tools = server.get("tool_count").and_then(|v| v.as_u64()).unwrap_or(0);
        let auth_type = server.get("auth_type").and_then(|v| v.as_str()).unwrap_or("?");
        let auth_status = server.get("auth_status").and_then(|v| v.as_str()).unwrap_or("?");
        let tokens = server.get("token_estimate").and_then(|v| v.as_u64()).unwrap_or(0);

        let auth_indicator = match auth_status {
            "connected" => "✓",
            "expired" => "✗",
            "pending" => "…",
            _ => "?",
        };

        out.push_str(&format!(
            "  {:<20} {:>3} tools   {} {:<8} tokens: ~{}\n",
            id, tools, auth_indicator, auth_type, tokens
        ));
    }

    out.push_str(&format!(
        "\nRun `bw <server>` to see tools, `bw <server> <tool> --help` for usage.\n"
    ));

    out
}

/// Format a tool list for human-readable output.
fn format_tool_list(server_id: &str, tools: &[serde_json::Value]) -> String {
    if tools.is_empty() {
        return format!("No tools available on '{}'.\n", server_id);
    }

    let mut out = format!("Tools on '{}':\n\n", server_id);

    for tool in tools {
        let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let desc = tool
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        out.push_str(&format!("  {:<30} {}\n", name, desc));
    }

    out.push_str(&format!(
        "\nRun `bw {} <tool> --help` for usage.\n",
        server_id
    ));

    out
}

/// Format tool --help output from parameter info.
fn format_tool_help(
    server_id: &str,
    tool_name: &str,
    description: &str,
    parameters: &[serde_json::Value],
) -> String {
    let mut out = format!("{}\n\nUSAGE:\n    bw {} {}", description, server_id, tool_name);

    let required: Vec<&serde_json::Value> = parameters
        .iter()
        .filter(|p| p.get("required").and_then(|v| v.as_bool()).unwrap_or(false))
        .collect();
    let optional: Vec<&serde_json::Value> = parameters
        .iter()
        .filter(|p| !p.get("required").and_then(|v| v.as_bool()).unwrap_or(false))
        .collect();

    for p in &required {
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        out.push_str(&format!(" --{} <value>", name));
    }
    if !optional.is_empty() {
        out.push_str(" [OPTIONS]");
    }
    out.push('\n');

    if !required.is_empty() {
        out.push_str("\nREQUIRED:\n");
        for p in &required {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let desc = p.get("description").and_then(|v| v.as_str()).unwrap_or("");
            out.push_str(&format!("    --{:<20} {}\n", name, desc));
        }
    }

    if !optional.is_empty() {
        out.push_str("\nOPTIONS:\n");
        for p in &optional {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let desc = p.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let default = p.get("default").and_then(|v| v.as_str());
            let suffix = match default {
                Some(d) => format!(" (default: {})", d),
                None => String::new(),
            };
            out.push_str(&format!("    --{:<20} {}{}\n", name, desc, suffix));
        }
    }

    out
}

// ─── Daemon IPC client ──────────────────────────────────────────────────────

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

#[cfg(unix)]
struct DaemonClient {
    writer: tokio::io::WriteHalf<UnixStream>,
    reader: tokio::io::Lines<BufReader<tokio::io::ReadHalf<UnixStream>>>,
}

#[cfg(unix)]
impl DaemonClient {
    async fn connect() -> Result<Self, String> {
        let socket_path = default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!(
                "Cannot connect to Brightwing daemon at {}: {}\nIs brightwing-authd running?",
                socket_path.display(), e
            ))?;

        let (reader, writer) = tokio::io::split(stream);
        let reader = BufReader::new(reader).lines();
        Ok(Self { writer, reader })
    }

    async fn send(&mut self, request: &IpcRequest) -> Result<(), String> {
        let bytes = encode_message(request).map_err(|e| format!("Serialize error: {}", e))?;
        self.writer.write_all(&bytes).await.map_err(|e| format!("Write error: {}", e))
    }

    async fn recv(&mut self) -> Result<IpcResponse, String> {
        let line = self.reader.next_line().await
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Daemon closed connection".to_string())?;
        decode_message(line.as_bytes()).map_err(|e| format!("Parse error: {}", e))
    }

    async fn handshake(&mut self) -> Result<(), String> {
        self.send(&IpcRequest::Handshake(HandshakeRequest {
            client: ClientType::CliShim,
            version: IPC_PROTOCOL_VERSION.to_string(),
        })).await?;

        match self.recv().await? {
            IpcResponse::HandshakeOk { .. } => Ok(()),
            IpcResponse::HandshakeError { message, .. } => Err(message),
            other => Err(format!("Unexpected handshake response: {:?}", other)),
        }
    }

    async fn request(&mut self, req: &IpcRequest) -> Result<IpcResponse, String> {
        self.send(req).await?;
        self.recv().await
    }
}

fn print_help() {
    eprintln!("bw — Brightwing Tool Broker CLI");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    bw <COMMAND>");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("    list                     List available servers and tools");
    eprintln!("    <server> <tool>          Call a tool on a server");
    eprintln!("    help                     Show this help");
    eprintln!();
    eprintln!("GLOBAL FLAGS:");
    eprintln!("    --json                   Output raw JSON instead of formatted text");
    eprintln!("    --verbose                Print timing and debug info to stderr");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    bw list                              # All servers");
    eprintln!("    bw list github                       # Tools on github server");
    eprintln!("    bw github search_repos --help        # Usage for a tool");
    eprintln!("    bw github search_repos --query \"mcp\" # Call a tool");
}

#[cfg(not(unix))]
fn main() {
    eprintln!("bw is not yet available on Windows.");
    std::process::exit(1);
}

#[cfg(unix)]
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let command = match parse_command(&args) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("bw: error: {}", e);
            eprintln!("Run `bw help` for usage.");
            std::process::exit(1);
        }
    };

    match command {
        Command::Help => {
            print_help();
            std::process::exit(0);
        }
        _ => {}
    }

    // Connect to daemon
    let mut daemon = match DaemonClient::connect().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("bw: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = daemon.handshake().await {
        eprintln!("bw: handshake failed: {}", e);
        std::process::exit(1);
    }

    match command {
        Command::Help => unreachable!(),

        Command::List { server_id: None } => {
            match daemon.request(&IpcRequest::ListServers).await {
                Ok(IpcResponse::ServerList { servers }) => {
                    let json_servers: Vec<serde_json::Value> = servers.into_iter().map(|s| {
                        serde_json::json!({
                            "server_id": s.server_id,
                            "tool_count": s.tool_count,
                            "auth_type": s.auth_type,
                            "auth_status": s.auth_status,
                            "token_estimate": s.token_estimate,
                        })
                    }).collect();
                    print!("{}", format_server_list(&json_servers));
                }
                Ok(IpcResponse::Error { message, .. }) => {
                    eprintln!("bw: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("bw: unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("bw: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Command::List { server_id: Some(ref id) } => {
            match daemon.request(&IpcRequest::ListTools { server_id: id.clone() }).await {
                Ok(IpcResponse::ToolList { server_id, tools }) => {
                    let json_tools: Vec<serde_json::Value> = tools.into_iter().map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                        })
                    }).collect();
                    print!("{}", format_tool_list(&server_id, &json_tools));
                }
                Ok(IpcResponse::Error { message, .. }) => {
                    eprintln!("bw: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("bw: unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("bw: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Command::ToolHelp { ref server_id, ref tool_name } => {
            match daemon.request(&IpcRequest::GetToolSchema {
                server_id: server_id.clone(),
                tool_name: tool_name.clone(),
            }).await {
                Ok(IpcResponse::ToolSchema { name: _, description, parameters }) => {
                    let json_params: Vec<serde_json::Value> = parameters.into_iter().map(|p| {
                        serde_json::json!({
                            "name": p.name,
                            "description": p.description,
                            "required": p.required,
                            "default": p.default,
                        })
                    }).collect();
                    print!("{}", format_tool_help(server_id, tool_name, &description, &json_params));
                }
                Ok(IpcResponse::Error { message, .. }) => {
                    eprintln!("bw: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("bw: unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("bw: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Command::CallTool { ref server_id, ref tool_name, ref arguments, json_output, verbose } => {
            let start = std::time::Instant::now();

            match daemon.request(&IpcRequest::CallTool {
                server_id: server_id.clone(),
                tool_name: tool_name.clone(),
                arguments: serde_json::to_value(arguments).unwrap_or_default(),
            }).await {
                Ok(IpcResponse::ToolResult { content, is_error, latency_ms }) => {
                    if verbose {
                        let latency = latency_ms.unwrap_or(0);
                        let total = start.elapsed().as_millis();
                        eprintln!("bw: upstream={}ms total={}ms", latency, total);
                    }

                    if json_output {
                        let output = serde_json::json!({
                            "content": content.iter().map(|c| {
                                serde_json::json!({ "type": c.content_type, "text": c.text })
                            }).collect::<Vec<_>>(),
                            "isError": is_error,
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
                    } else {
                        let json_content: Vec<serde_json::Value> = content.iter().map(|c| {
                            serde_json::json!({ "type": c.content_type, "text": c.text })
                        }).collect();
                        let output = format_tool_result(&json_content, is_error);
                        if !output.is_empty() {
                            println!("{}", output);
                        }
                    }

                    if is_error {
                        std::process::exit(1);
                    }
                }
                Ok(IpcResponse::Error { message, .. }) => {
                    eprintln!("bw: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("bw: unexpected response: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("bw: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    // ─── parse_command ───────────────────────────────────────────────────

    #[test]
    fn test_empty_args_is_help() {
        assert_eq!(parse_command(&[]).unwrap(), Command::Help);
    }

    #[test]
    fn test_help_command() {
        assert_eq!(parse_command(&args("help")).unwrap(), Command::Help);
        assert_eq!(parse_command(&args("--help")).unwrap(), Command::Help);
        assert_eq!(parse_command(&args("-h")).unwrap(), Command::Help);
    }

    #[test]
    fn test_list_all() {
        assert_eq!(
            parse_command(&args("list")).unwrap(),
            Command::List { server_id: None }
        );
    }

    #[test]
    fn test_list_server() {
        assert_eq!(
            parse_command(&args("list github")).unwrap(),
            Command::List {
                server_id: Some("github".to_string())
            }
        );
    }

    #[test]
    fn test_server_only_is_list() {
        assert_eq!(
            parse_command(&args("github")).unwrap(),
            Command::List {
                server_id: Some("github".to_string())
            }
        );
    }

    #[test]
    fn test_tool_help() {
        assert_eq!(
            parse_command(&args("github search_repos --help")).unwrap(),
            Command::ToolHelp {
                server_id: "github".to_string(),
                tool_name: "search_repos".to_string(),
            }
        );
    }

    #[test]
    fn test_tool_call_with_params() {
        let cmd = parse_command(&args("github search_repos --query mcp --per_page 5")).unwrap();
        match cmd {
            Command::CallTool {
                server_id,
                tool_name,
                arguments,
                json_output,
                verbose,
            } => {
                assert_eq!(server_id, "github");
                assert_eq!(tool_name, "search_repos");
                assert_eq!(
                    arguments.get("query"),
                    Some(&serde_json::Value::String("mcp".to_string()))
                );
                assert_eq!(
                    arguments.get("per_page"),
                    Some(&serde_json::json!(5))
                );
                assert!(!json_output);
                assert!(!verbose);
            }
            _ => panic!("Expected CallTool, got {:?}", cmd),
        }
    }

    #[test]
    fn test_tool_call_with_json_flag() {
        let cmd = parse_command(&args("github search_repos --query mcp --json")).unwrap();
        match cmd {
            Command::CallTool { json_output, .. } => assert!(json_output),
            _ => panic!("Expected CallTool"),
        }
    }

    #[test]
    fn test_tool_call_with_verbose() {
        let cmd = parse_command(&args("github search_repos --query mcp --verbose")).unwrap();
        match cmd {
            Command::CallTool { verbose, .. } => assert!(verbose),
            _ => panic!("Expected CallTool"),
        }
    }

    #[test]
    fn test_tool_call_no_params() {
        let cmd = parse_command(&args("github list_repos")).unwrap();
        match cmd {
            Command::CallTool { arguments, .. } => assert!(arguments.is_empty()),
            _ => panic!("Expected CallTool"),
        }
    }

    // ─── coerce_value ────────────────────────────────────────────────────

    #[test]
    fn test_coerce_integer() {
        assert_eq!(coerce_value("42"), serde_json::json!(42));
        assert_eq!(coerce_value("0"), serde_json::json!(0));
        assert_eq!(coerce_value("-1"), serde_json::json!(-1));
    }

    #[test]
    fn test_coerce_bool() {
        assert_eq!(coerce_value("true"), serde_json::json!(true));
        assert_eq!(coerce_value("false"), serde_json::json!(false));
    }

    #[test]
    fn test_coerce_float() {
        assert_eq!(coerce_value("3.14"), serde_json::json!(3.14));
    }

    #[test]
    fn test_coerce_string() {
        assert_eq!(
            coerce_value("hello"),
            serde_json::Value::String("hello".to_string())
        );
        assert_eq!(
            coerce_value("mcp server"),
            serde_json::Value::String("mcp server".to_string())
        );
    }

    // ─── parse_tool_params ───────────────────────────────────────────────

    #[test]
    fn test_boolean_flag() {
        let (params, _, _) = parse_tool_params(&args("--open")).unwrap();
        assert_eq!(params.get("open"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_mixed_params() {
        let (params, json, verbose) = parse_tool_params(&args(
            "--query mcp --per_page 10 --desc --json --verbose",
        ))
        .unwrap();
        assert_eq!(
            params.get("query"),
            Some(&serde_json::Value::String("mcp".to_string()))
        );
        assert_eq!(params.get("per_page"), Some(&serde_json::json!(10)));
        assert_eq!(params.get("desc"), Some(&serde_json::json!(true)));
        assert!(json);
        assert!(verbose);
    }

    #[test]
    fn test_unexpected_positional_arg() {
        let result = parse_tool_params(&args("bare_arg"));
        assert!(result.is_err());
    }

    // ─── format_tool_result ──────────────────────────────────────────────

    #[test]
    fn test_format_single_text_block() {
        let content = vec![serde_json::json!({"type": "text", "text": "Found 42 repos"})];
        assert_eq!(format_tool_result(&content, false), "Found 42 repos");
    }

    #[test]
    fn test_format_multiple_text_blocks() {
        let content = vec![
            serde_json::json!({"type": "text", "text": "Line 1"}),
            serde_json::json!({"type": "text", "text": "Line 2"}),
        ];
        assert_eq!(format_tool_result(&content, false), "Line 1\nLine 2");
    }

    #[test]
    fn test_format_error_result() {
        let content = vec![serde_json::json!({"type": "text", "text": "Not found"})];
        assert_eq!(format_tool_result(&content, true), "Error: Not found");
    }

    #[test]
    fn test_format_empty_content() {
        let content: Vec<serde_json::Value> = vec![];
        assert_eq!(format_tool_result(&content, false), "");
    }

    // ─── format_server_list ──────────────────────────────────────────────

    #[test]
    fn test_format_server_list_empty() {
        let output = format_server_list(&[]);
        assert!(output.contains("No servers connected"));
    }

    #[test]
    fn test_format_server_list_with_servers() {
        let servers = vec![serde_json::json!({
            "server_id": "github",
            "tool_count": 5,
            "auth_type": "oauth",
            "auth_status": "connected",
            "token_estimate": 2200
        })];
        let output = format_server_list(&servers);
        assert!(output.contains("1 server connected"));
        assert!(output.contains("github"));
        assert!(output.contains("5 tools"));
        assert!(output.contains("✓"));
    }

    // ─── format_tool_help ────────────────────────────────────────────────

    #[test]
    fn test_format_tool_help_with_params() {
        let params = vec![
            serde_json::json!({
                "name": "query",
                "description": "Search query string",
                "required": true,
                "default": null
            }),
            serde_json::json!({
                "name": "per_page",
                "description": "Results per page",
                "required": false,
                "default": "10"
            }),
        ];
        let output = format_tool_help("github", "search_repos", "Search GitHub repos", &params);
        assert!(output.contains("Search GitHub repos"));
        assert!(output.contains("bw github search_repos"));
        assert!(output.contains("REQUIRED:"));
        assert!(output.contains("--query"));
        assert!(output.contains("OPTIONS:"));
        assert!(output.contains("--per_page"));
        assert!(output.contains("(default: 10)"));
    }
}
