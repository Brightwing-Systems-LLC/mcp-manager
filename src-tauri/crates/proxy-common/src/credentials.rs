use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Credential types that can be stored in the vault and served to proxies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Credential {
    /// OAuth token (access + optional refresh)
    #[serde(rename = "oauth")]
    OAuth(OAuthCredential),

    /// API key(s) injected as environment variables
    #[serde(rename = "api_key")]
    ApiKey(ApiKeyCredential),

    /// No authentication needed
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthCredential {
    pub access_token: String,
    /// URL of the upstream MCP server to connect to
    pub url: String,
    /// When the access token expires (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKeyCredential {
    /// Environment variables to inject (e.g., {"GITHUB_TOKEN": "ghp_xxx"})
    pub env: HashMap<String, String>,
    /// Command to spawn for stdio upstream servers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Args for the command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// URL for HTTP upstream servers (mutually exclusive with command)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Errors returned when credentials can't be retrieved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CredentialError {
    pub code: CredentialErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialErrorCode {
    /// Server not registered in Brightwing
    NotFound,
    /// OAuth token expired, user needs to re-authenticate
    AuthExpired,
    /// Vault is locked or inaccessible
    VaultError,
    /// Internal error
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_credential_roundtrip() {
        let cred = Credential::OAuth(OAuthCredential {
            access_token: "gho_abc123".to_string(),
            url: "https://github-mcp.example.com/mcp".to_string(),
            expires_at: Some("2026-03-10T12:00:00Z".to_string()),
        });

        let json = serde_json::to_string(&cred).unwrap();
        let parsed: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(cred, parsed);
    }

    #[test]
    fn test_api_key_credential_roundtrip() {
        let mut env = HashMap::new();
        env.insert("GITHUB_TOKEN".to_string(), "ghp_xxx".to_string());

        let cred = Credential::ApiKey(ApiKeyCredential {
            env,
            command: Some("npx".to_string()),
            args: Some(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-github".to_string(),
            ]),
            url: None,
        });

        let json = serde_json::to_string(&cred).unwrap();
        let parsed: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(cred, parsed);
    }

    #[test]
    fn test_none_credential_roundtrip() {
        let cred = Credential::None;
        let json = serde_json::to_string(&cred).unwrap();
        let parsed: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(cred, parsed);
    }

    #[test]
    fn test_credential_error_roundtrip() {
        let err = CredentialError {
            code: CredentialErrorCode::AuthExpired,
            message: "OAuth token expired. Please re-authenticate in Brightwing.".to_string(),
        };

        let json = serde_json::to_string(&err).unwrap();
        let parsed: CredentialError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, parsed);
    }

    #[test]
    fn test_api_key_http_upstream() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "sk-xxx".to_string());

        let cred = Credential::ApiKey(ApiKeyCredential {
            env,
            command: None,
            args: None,
            url: Some("https://brave-mcp.example.com".to_string()),
        });

        let json = serde_json::to_string(&cred).unwrap();
        // Verify optional fields are omitted
        assert!(!json.contains("command"));
        assert!(!json.contains("args"));
        assert!(json.contains("url"));
    }
}
