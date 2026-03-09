use super::types::{OAuthError, OAuthTokenSet};

/// Parameters for the authorization code exchange.
pub struct ExchangeParams {
    pub token_endpoint: String,
    pub code: String,
    pub redirect_uri: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub code_verifier: String,
    pub server_url: String,
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(params: &ExchangeParams) -> Result<OAuthTokenSet, OAuthError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", params.code.clone()),
        ("redirect_uri", params.redirect_uri.clone()),
        ("client_id", params.client_id.clone()),
        ("code_verifier", params.code_verifier.clone()),
    ];

    if let Some(secret) = &params.client_secret {
        form.push(("client_secret", secret.clone()));
    }

    let resp = client
        .post(&params.token_endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|e| OAuthError::ExchangeFailed(format!("Request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::ExchangeFailed(format!(
            "HTTP {}: {}",
            status, body
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| OAuthError::ExchangeFailed(format!("Invalid JSON: {}", e)))?;

    parse_token_response(&body, &params.token_endpoint, &params.client_id, params.client_secret.as_deref(), &params.server_url)
}

pub(crate) fn parse_token_response(
    body: &serde_json::Value,
    token_endpoint: &str,
    client_id: &str,
    client_secret: Option<&str>,
    server_url: &str,
) -> Result<OAuthTokenSet, OAuthError> {
    // Check for OAuth error response
    if let Some(error) = body.get("error").and_then(|e| e.as_str()) {
        let desc = body
            .get("error_description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        return Err(OAuthError::ExchangeFailed(format!("{}: {}", error, desc)));
    }

    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OAuthError::ExchangeFailed("Missing access_token".to_string()))?
        .to_string();

    let refresh_token = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(String::from);

    let token_type = body
        .get("token_type")
        .and_then(|v| v.as_str())
        .unwrap_or("Bearer")
        .to_string();

    let expires_in = body
        .get("expires_in")
        .and_then(|v| v.as_u64());

    let expires_at = expires_in.map(|secs| {
        let now = chrono::Utc::now();
        let expiry = now + chrono::Duration::seconds(secs as i64);
        expiry.to_rfc3339()
    });

    let scope = body
        .get("scope")
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(OAuthTokenSet {
        access_token,
        refresh_token,
        token_type,
        expires_in,
        expires_at,
        scope,
        token_endpoint: token_endpoint.to_string(),
        client_id: client_id.to_string(),
        client_secret: client_secret.map(String::from),
        server_url: server_url.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_token_response_full() {
        let body = json!({
            "access_token": "gho_abc123",
            "refresh_token": "ghr_xyz789",
            "token_type": "bearer",
            "expires_in": 3600,
            "scope": "read write"
        });
        let ts = parse_token_response(&body, "https://a.com/token", "cid", None, "https://a.com").unwrap();
        assert_eq!(ts.access_token, "gho_abc123");
        assert_eq!(ts.refresh_token.as_deref(), Some("ghr_xyz789"));
        assert_eq!(ts.token_type, "bearer");
        assert_eq!(ts.expires_in, Some(3600));
        assert!(ts.expires_at.is_some());
        assert_eq!(ts.scope.as_deref(), Some("read write"));
        assert_eq!(ts.client_id, "cid");
    }

    #[test]
    fn test_parse_token_response_minimal() {
        let body = json!({"access_token": "tok"});
        let ts = parse_token_response(&body, "https://a.com/token", "c", None, "https://a.com").unwrap();
        assert_eq!(ts.access_token, "tok");
        assert!(ts.refresh_token.is_none());
        assert_eq!(ts.token_type, "Bearer"); // default
        assert!(ts.expires_at.is_none());
    }

    #[test]
    fn test_parse_token_response_error() {
        let body = json!({
            "error": "invalid_grant",
            "error_description": "Code expired"
        });
        let result = parse_token_response(&body, "https://a.com/token", "c", None, "https://a.com");
        assert!(matches!(result, Err(OAuthError::ExchangeFailed(_))));
    }

    #[test]
    fn test_parse_token_response_missing_access_token() {
        let body = json!({"token_type": "bearer"});
        let result = parse_token_response(&body, "https://a.com/token", "c", None, "https://a.com");
        assert!(result.is_err());
    }
}
