//! Tool discovery — sends MCP `initialize` + `tools/list` to an upstream HTTP server
//! and returns discovered tool metadata.

use serde::{Deserialize, Serialize};

/// A tool discovered from an upstream MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub token_estimate: u32,
}

/// Parse an MCP HTTP response that may be JSON or SSE (text/event-stream).
/// For SSE, extracts the JSON from the first `data:` line that contains a JSON-RPC message.
async fn parse_mcp_response(resp: reqwest::Response) -> Result<serde_json::Value, String> {
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body_text = resp.text().await.map_err(|e| e.to_string())?;

    if content_type.contains("text/event-stream") {
        // Parse SSE: look for lines starting with "data:" that contain JSON-RPC
        for line in body_text.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data.starts_with('{') {
                    return serde_json::from_str(data).map_err(|e| e.to_string());
                }
            }
        }
        Err("No JSON-RPC message found in SSE response".to_string())
    } else {
        serde_json::from_str(&body_text).map_err(|e| e.to_string())
    }
}

/// Discover tools from an upstream HTTP MCP server.
///
/// Sends `initialize` (required by MCP spec) then `tools/list`, returning
/// the full list of tools with their schemas and token estimates.
pub async fn discover_tools(
    upstream_url: &str,
    auth_header: Option<&str>,
) -> Result<Vec<DiscoveredTool>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // 1. Send initialize request
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "brightwing-discovery",
                "version": "1.0.0"
            }
        }
    });

    let mut req = client
        .post(upstream_url)
        .header("Accept", "application/json, text/event-stream")
        .json(&init_request);
    if let Some(auth) = auth_header {
        req = req.header("Authorization", auth);
    }

    let init_resp = req.send().await.map_err(|e| format!("Initialize request failed: {}", e))?;

    if init_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Authentication required. Please connect your account first.".to_string());
    }
    if !init_resp.status().is_success() {
        return Err(format!("Initialize failed with status {}", init_resp.status()));
    }

    // Capture session ID if the server provides one
    let session_id = init_resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Parse initialize response — handle both JSON and SSE formats
    let _init_body = parse_mcp_response(init_resp).await
        .map_err(|e| format!("Failed to parse initialize response: {}", e))?;

    // 2. Send initialized notification (required by MCP spec before calling methods)
    let initialized_notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    let mut req = client
        .post(upstream_url)
        .header("Accept", "application/json, text/event-stream")
        .json(&initialized_notification);
    if let Some(auth) = auth_header {
        req = req.header("Authorization", auth);
    }
    if let Some(ref sid) = session_id {
        req = req.header("Mcp-Session-Id", sid);
    }
    // Notification — we don't need the response, just fire it
    let _ = req.send().await;

    // 3. Send tools/list request
    let tools_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });

    let mut req = client
        .post(upstream_url)
        .header("Accept", "application/json, text/event-stream")
        .json(&tools_request);
    if let Some(auth) = auth_header {
        req = req.header("Authorization", auth);
    }
    if let Some(ref sid) = session_id {
        req = req.header("Mcp-Session-Id", sid);
    }

    let tools_resp = req.send().await.map_err(|e| format!("tools/list request failed: {}", e))?;

    if !tools_resp.status().is_success() {
        return Err(format!("tools/list failed with status {}", tools_resp.status()));
    }

    let body: serde_json::Value = parse_mcp_response(tools_resp).await
        .map_err(|e| format!("Failed to parse tools/list response: {}", e))?;

    // Check for JSON-RPC error
    if let Some(error) = body.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(format!("tools/list returned error: {}", msg));
    }

    // Extract tools array from result
    let tools_array = body
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .ok_or("tools/list response missing result.tools array")?;

    let mut discovered = Vec::new();
    for tool in tools_array {
        let name = tool
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let description = tool
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let input_schema = tool
            .get("inputSchema")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let schema_str = serde_json::to_string(&input_schema).unwrap_or_default();
        let token_estimate =
            proxy_common::tokens::estimate_tool_tokens(&name, &description, &schema_str);

        discovered.push(DiscoveredTool {
            name,
            description,
            input_schema,
            token_estimate,
        });
    }

    Ok(discovered)
}

/// Check whether a set of discovered tools includes a PatchworkMCP `feedback` tool.
pub fn has_patchwork_feedback(tools: &[DiscoveredTool]) -> bool {
    tools.iter().any(|t| t.name == "feedback")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tools_list_response() {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {
                        "name": "search_repos",
                        "description": "Search GitHub repositories",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Search query" }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "get_repo",
                        "description": "Get repository details",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "owner": { "type": "string" },
                                "repo": { "type": "string" }
                            },
                            "required": ["owner", "repo"]
                        }
                    }
                ]
            }
        });

        let tools_array = body["result"]["tools"].as_array().unwrap();
        assert_eq!(tools_array.len(), 2);

        let tool = &tools_array[0];
        assert_eq!(tool["name"], "search_repos");
        assert_eq!(tool["description"], "Search GitHub repositories");
    }

    #[test]
    fn test_parse_error_response() {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        });

        let error = body.get("error");
        assert!(error.is_some());
        let msg = error.unwrap().get("message").unwrap().as_str().unwrap();
        assert_eq!(msg, "Method not found");
    }

    #[test]
    fn test_has_patchwork_feedback_present() {
        let tools = vec![
            DiscoveredTool {
                name: "search".to_string(),
                description: "Search".to_string(),
                input_schema: serde_json::json!({}),
                token_estimate: 100,
            },
            DiscoveredTool {
                name: "feedback".to_string(),
                description: "Report limitations".to_string(),
                input_schema: serde_json::json!({}),
                token_estimate: 50,
            },
        ];
        assert!(has_patchwork_feedback(&tools));
    }

    #[test]
    fn test_has_patchwork_feedback_absent() {
        let tools = vec![
            DiscoveredTool {
                name: "search".to_string(),
                description: "Search".to_string(),
                input_schema: serde_json::json!({}),
                token_estimate: 100,
            },
        ];
        assert!(!has_patchwork_feedback(&tools));
    }

    #[test]
    fn test_parse_empty_tools() {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": []
            }
        });

        let tools = body["result"]["tools"].as_array().unwrap();
        assert!(tools.is_empty());
    }
}
