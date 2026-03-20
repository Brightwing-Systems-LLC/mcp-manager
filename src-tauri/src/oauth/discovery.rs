use super::types::{OAuthError, OAuthServerMetadata};

/// Discover OAuth metadata from the server's well-known endpoint.
///
/// Per RFC 8414 / MCP spec, tries the path-level well-known URL first
/// (e.g. `https://host/mcp/.well-known/oauth-authorization-server`),
/// then falls back to the origin-level URL
/// (e.g. `https://host/.well-known/oauth-authorization-server`).
pub async fn discover_oauth_metadata(server_url: &str) -> Result<OAuthServerMetadata, OAuthError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    // Build candidate URLs: path-level first, then origin-level fallback
    let trimmed = server_url.trim_end_matches('/');
    let path_level = format!("{}/.well-known/oauth-authorization-server", trimmed);

    let origin_level = match reqwest::Url::parse(trimmed) {
        Ok(parsed) => {
            let origin = parsed.origin().ascii_serialization();
            Some(format!("{}/.well-known/oauth-authorization-server", origin))
        }
        Err(_) => None,
    };

    let mut urls = vec![path_level.clone()];
    if let Some(ref origin) = origin_level {
        // Only add origin-level if it differs from path-level
        if origin != &path_level {
            urls.push(origin.clone());
        }
    }

    let mut last_err = OAuthError::DiscoveryFailed("No URLs to try".to_string());

    for candidate_url in &urls {
        let resp = match client.get(candidate_url).send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = OAuthError::DiscoveryFailed(format!("Request failed: {}", e));
                continue;
            }
        };

        if !resp.status().is_success() {
            last_err = OAuthError::DiscoveryFailed(format!("HTTP {}", resp.status()));
            continue;
        }

        let metadata: OAuthServerMetadata = match resp.json().await {
            Ok(m) => m,
            Err(e) => {
                last_err = OAuthError::DiscoveryFailed(format!("Invalid JSON: {}", e));
                continue;
            }
        };

        // Validate required fields
        if metadata.authorization_endpoint.is_empty() {
            last_err = OAuthError::DiscoveryFailed(
                "Missing authorization_endpoint".to_string(),
            );
            continue;
        }
        if metadata.token_endpoint.is_empty() {
            last_err = OAuthError::DiscoveryFailed(
                "Missing token_endpoint".to_string(),
            );
            continue;
        }

        return Ok(metadata);
    }

    Err(last_err)
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
