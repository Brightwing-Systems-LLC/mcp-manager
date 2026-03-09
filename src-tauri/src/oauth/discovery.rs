use super::types::{OAuthError, OAuthServerMetadata};

/// Discover OAuth metadata from the server's well-known endpoint.
pub async fn discover_oauth_metadata(server_url: &str) -> Result<OAuthServerMetadata, OAuthError> {
    let url = format!(
        "{}/.well-known/oauth-authorization-server",
        server_url.trim_end_matches('/')
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| OAuthError::DiscoveryFailed(format!("Request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(OAuthError::DiscoveryFailed(format!(
            "HTTP {}",
            resp.status()
        )));
    }

    let metadata: OAuthServerMetadata = resp
        .json()
        .await
        .map_err(|e| OAuthError::DiscoveryFailed(format!("Invalid JSON: {}", e)))?;

    // Validate required fields
    if metadata.authorization_endpoint.is_empty() {
        return Err(OAuthError::DiscoveryFailed(
            "Missing authorization_endpoint".to_string(),
        ));
    }
    if metadata.token_endpoint.is_empty() {
        return Err(OAuthError::DiscoveryFailed(
            "Missing token_endpoint".to_string(),
        ));
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discovery_invalid_url() {
        let result = discover_oauth_metadata("http://127.0.0.1:1").await;
        assert!(matches!(result, Err(OAuthError::DiscoveryFailed(_))));
    }

    #[test]
    fn test_metadata_deserialize() {
        let json = r#"{
            "issuer": "https://auth.example.com",
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token",
            "registration_endpoint": "https://auth.example.com/register",
            "scopes_supported": ["read", "write"],
            "response_types_supported": ["code"],
            "code_challenge_methods_supported": ["S256"]
        }"#;
        let meta: OAuthServerMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.authorization_endpoint, "https://auth.example.com/authorize");
        assert_eq!(meta.token_endpoint, "https://auth.example.com/token");
        assert!(meta.registration_endpoint.is_some());
        assert!(meta.code_challenge_methods_supported.unwrap().contains(&"S256".to_string()));
    }

    #[test]
    fn test_metadata_minimal() {
        let json = r#"{
            "authorization_endpoint": "https://a.com/auth",
            "token_endpoint": "https://a.com/token"
        }"#;
        let meta: OAuthServerMetadata = serde_json::from_str(json).unwrap();
        assert!(meta.issuer.is_none());
        assert!(meta.registration_endpoint.is_none());
    }
}
