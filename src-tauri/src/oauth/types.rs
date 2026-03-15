use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OAuth 2.1 server metadata from `.well-known/oauth-authorization-server`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthServerMetadata {
    pub issuer: Option<String>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Option<Vec<String>>,
    #[serde(default)]
    pub response_types_supported: Vec<String>,
    #[serde(default)]
    pub code_challenge_methods_supported: Option<Vec<String>>,
}

/// Full token set stored in the vault (richer than the IPC Credential::OAuth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<u64>,
    /// ISO 8601 timestamp of when the token expires.
    pub expires_at: Option<String>,
    pub scope: Option<String>,
    // Metadata for refresh:
    pub token_endpoint: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    /// The upstream MCP server URL.
    pub server_url: String,
}

/// Non-sensitive metadata stored in SQLite (secrets go in the vault).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenMeta {
    pub token_type: String,
    pub expires_at: Option<String>,
    pub scope: Option<String>,
    pub token_endpoint: String,
    pub client_id: String,
    pub server_url: String,
}

impl From<&OAuthTokenSet> for OAuthTokenMeta {
    fn from(ts: &OAuthTokenSet) -> Self {
        Self {
            token_type: ts.token_type.clone(),
            expires_at: ts.expires_at.clone(),
            scope: ts.scope.clone(),
            token_endpoint: ts.token_endpoint.clone(),
            client_id: ts.client_id.clone(),
            server_url: ts.server_url.clone(),
        }
    }
}

/// Dynamic client registration result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRegistration {
    pub client_id: String,
    pub client_secret: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// In-progress OAuth flow state (held in memory until callback completes).
pub struct OAuthFlowState {
    pub server_id: String,
    pub code_verifier: String,
    pub state: String,
    pub redirect_uri: String,
    pub token_endpoint: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub server_url: String,
}

/// Returned to the frontend when starting an OAuth flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthFlowInfo {
    pub auth_url: String,
    pub state: String,
}

/// OAuth status for a proxy server, returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthStatus {
    pub status: String, // "disconnected" | "connected" | "expired" | "error"
    pub expires_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug)]
pub enum OAuthError {
    DiscoveryFailed(String),
    RegistrationFailed(String),
    ExchangeFailed(String),
    RefreshFailed(String),
    CallbackTimeout,
    InvalidState,
    Network(String),
    Internal(String),
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DiscoveryFailed(msg) => write!(f, "OAuth discovery failed: {}", msg),
            Self::RegistrationFailed(msg) => write!(f, "Client registration failed: {}", msg),
            Self::ExchangeFailed(msg) => write!(f, "Token exchange failed: {}", msg),
            Self::RefreshFailed(msg) => write!(f, "Token refresh failed: {}", msg),
            Self::CallbackTimeout => write!(f, "OAuth callback timed out"),
            Self::InvalidState => write!(f, "Invalid OAuth state parameter (CSRF check failed)"),
            Self::Network(msg) => write!(f, "Network error: {}", msg),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}
