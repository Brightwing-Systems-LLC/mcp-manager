//! Integration tests for tool discovery from upstream MCP servers.

mod mock_mcp_server;

use brightwing_mcp_manager_lib::db::Database;
use brightwing_mcp_manager_lib::proxy::discovery::discover_tools;
use mock_mcp_server::MockMcpServer;
use serde_json::json;

#[tokio::test]
async fn test_discover_tools_from_http_server() {
    let server = MockMcpServer::builder()
        .with_tool(
            "search_repos",
            "Search GitHub repositories",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" }
                },
                "required": ["query"]
            }),
        )
        .with_tool(
            "get_repo",
            "Get repository details",
            json!({
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" }
                },
                "required": ["owner", "repo"]
            }),
        )
        .with_tool(
            "list_issues",
            "List issues for a repository",
            json!({
                "type": "object",
                "properties": {
                    "owner": { "type": "string" },
                    "repo": { "type": "string" },
                    "state": { "type": "string" }
                },
                "required": ["owner", "repo"]
            }),
        )
        .build_http()
        .await;

    let tools = discover_tools(&server.url(), None).await.unwrap();

    assert_eq!(tools.len(), 3);
    assert!(tools.iter().any(|t| t.name == "search_repos"));
    assert!(tools.iter().any(|t| t.name == "get_repo"));
    assert!(tools.iter().any(|t| t.name == "list_issues"));

    // Verify descriptions
    let search = tools.iter().find(|t| t.name == "search_repos").unwrap();
    assert_eq!(search.description, "Search GitHub repositories");
    assert!(search.token_estimate > 0);

    // Verify schema is preserved
    assert!(search.input_schema.get("properties").is_some());
}

#[tokio::test]
async fn test_discover_tools_with_auth() {
    let server = MockMcpServer::builder()
        .with_tool("ping", "Ping the server", json!({}))
        .require_auth("Bearer secret-token")
        .build_http()
        .await;

    // Without auth should fail
    let err = discover_tools(&server.url(), None).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("Authentication required"));

    // With correct auth should succeed
    let tools = discover_tools(&server.url(), Some("Bearer secret-token"))
        .await
        .unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "ping");
}

#[tokio::test]
async fn test_discover_tools_empty_server() {
    let server = MockMcpServer::builder().build_http().await;

    let tools = discover_tools(&server.url(), None).await.unwrap();
    assert!(tools.is_empty());
}

#[tokio::test]
async fn test_discover_and_cache_in_db() {
    let server = MockMcpServer::builder()
        .with_tool(
            "search",
            "Search for items",
            json!({
                "type": "object",
                "properties": { "q": { "type": "string" } },
                "required": ["q"]
            }),
        )
        .with_tool("ping", "Ping", json!({}))
        .build_http()
        .await;

    let db = Database::new_in_memory().unwrap();

    // Register a proxy server
    db.register_proxy_server("test-server", "Test", "none", Some(&server.url()), None, None)
        .unwrap();

    // Discover tools
    let tools = discover_tools(&server.url(), None).await.unwrap();
    assert_eq!(tools.len(), 2);

    // Cache them
    for tool in &tools {
        let schema_str = serde_json::to_string(&tool.input_schema).unwrap();
        db.cache_tool_schema("test-server", &tool.name, &tool.description, &schema_str, tool.token_estimate)
            .unwrap();
    }

    // Verify cached tools
    let cached = db.get_cached_tools("test-server").unwrap();
    assert_eq!(cached.len(), 2);

    // Verify tool filter entries were auto-created (default: enabled)
    let filter = db.get_tool_filter("test-server").unwrap();
    assert_eq!(filter.len(), 2);
    assert!(filter.iter().all(|f| f.enabled));
}
