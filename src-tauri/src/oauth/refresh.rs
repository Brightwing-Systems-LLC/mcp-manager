use super::exchange::parse_token_response;
use super::types::{OAuthError, OAuthTokenSet};

/// Refresh an OAuth token using the refresh_token grant.
pub async fn refresh_token(token_set: &OAuthTokenSet) -> Result<OAuthTokenSet, OAuthError> {
    let refresh_token = token_set
        .refresh_token
        .as_ref()
        .ok_or_else(|| OAuthError::RefreshFailed("No refresh token available".to_string()))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.clone()),
        ("client_id", token_set.client_id.clone()),
    ];

    if let Some(secret) = &token_set.client_secret {
        form.push(("client_secret", secret.clone()));
    }

    let resp = client
        .post(&token_set.token_endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|e| OAuthError::RefreshFailed(format!("Request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::RefreshFailed(format!(
            "HTTP {}: {}",
            status, body
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| OAuthError::RefreshFailed(format!("Invalid JSON: {}", e)))?;

    let mut new_set = parse_token_response(
        &body,
        &token_set.token_endpoint,
        &token_set.client_id,
        token_set.client_secret.as_deref(),
        &token_set.server_url,
    )
    .map_err(|e| OAuthError::RefreshFailed(format!("{}", e)))?;

    // Preserve old refresh token if the server didn't issue a new one
    if new_set.refresh_token.is_none() {
        new_set.refresh_token = token_set.refresh_token.clone();
    }

    Ok(new_set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_no_refresh_token() {
        let ts = OAuthTokenSet {
            access_token: "old".to_string(),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_in: None,
            expires_at: None,
            scope: None,
            token_endpoint: "https://a.com/token".to_string(),
            client_id: "c".to_string(),
            client_secret: None,
            server_url: "https://a.com".to_string(),
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(refresh_token(&ts));
        assert!(matches!(result, Err(OAuthError::RefreshFailed(_))));
    }
}
