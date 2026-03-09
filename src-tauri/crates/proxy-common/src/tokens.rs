/// Estimate the number of tokens a tool schema will consume in an LLM context.
///
/// Uses a simple heuristic: ~4 characters per token (conservative for English + JSON).
/// This is intentionally rough — the exact tokenizer doesn't matter, we just need
/// a relative ranking to show users which tools are expensive.
pub fn estimate_tool_tokens(name: &str, description: &str, input_schema_json: &str) -> u32 {
    // Tool declaration overhead: name, description, and schema wrapper
    let chars = name.len() + description.len() + input_schema_json.len() + 50; // 50 for JSON wrapper
    (chars as u32 + 3) / 4 // ceil(chars / 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_tool() {
        let tokens = estimate_tool_tokens("ping", "Send a ping", "{}");
        assert!(tokens > 0);
        assert!(tokens < 50); // Should be very small
    }

    #[test]
    fn test_large_schema() {
        let big_schema = r#"{"type":"object","properties":{"query":{"type":"string","description":"The search query to find repositories"},"per_page":{"type":"integer","description":"Number of results per page"},"sort":{"type":"string","enum":["stars","forks","updated"],"description":"How to sort results"},"order":{"type":"string","enum":["asc","desc"],"description":"Sort order"}},"required":["query"]}"#;
        let tokens = estimate_tool_tokens("search_repos", "Search GitHub repositories by query", big_schema);
        assert!(tokens > 50);
        assert!(tokens < 500);
    }

    #[test]
    fn test_relative_ordering() {
        let small = estimate_tool_tokens("ping", "Ping", "{}");
        let medium = estimate_tool_tokens("search", "Search for items", r#"{"type":"object","properties":{"q":{"type":"string"}}}"#);
        let large = estimate_tool_tokens("create_issue", "Create a new issue", r#"{"type":"object","properties":{"title":{"type":"string","description":"Issue title"},"body":{"type":"string","description":"Issue body text"},"labels":{"type":"array","items":{"type":"string"},"description":"Labels to add"},"assignees":{"type":"array","items":{"type":"string"},"description":"Users to assign"}},"required":["title"]}"#);
        assert!(small < medium);
        assert!(medium < large);
    }
}
