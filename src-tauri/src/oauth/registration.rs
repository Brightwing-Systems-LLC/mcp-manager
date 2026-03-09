use super::types::{ClientRegistration, OAuthError};
use serde_json::json;

/// Register Brightwing as an OAuth client via RFC 7591 Dynamic Client Registration.
pub async fn register_client(
    registration_endpoint: &str,
    redirect_uri: &str,
) -> Result<ClientRegistration, OAuthError> {
    let body = json!({
        "client_name": "Brightwing MCP Manager",
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    let resp = client
        .post(registration_endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| OAuthError::RegistrationFailed(format!("Request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(OAuthError::RegistrationFailed(format!(
            "HTTP {}: {}",
            status, body
        )));
    }

    let reg: ClientRegistration = resp
        .json()
        .await
        .map_err(|e| OAuthError::RegistrationFailed(format!("Invalid JSON: {}", e)))?;

    if reg.client_id.is_empty() {
        return Err(OAuthError::RegistrationFailed(
            "Response missing client_id".to_string(),
        ));
    }

    Ok(reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_registration_deserialize() {
        let json = r#"{
            "client_id": "abc123",
            "client_secret": "secret456",
            "client_id_issued_at": 1234567890
        }"#;
        let reg: ClientRegistration = serde_json::from_str(json).unwrap();
        assert_eq!(reg.client_id, "abc123");
        assert_eq!(reg.client_secret.as_deref(), Some("secret456"));
        assert!(reg.extra.contains_key("client_id_issued_at"));
    }

    #[test]
    fn test_client_registration_no_secret() {
        let json = r#"{"client_id": "pub-client-xyz"}"#;
        let reg: ClientRegistration = serde_json::from_str(json).unwrap();
        assert_eq!(reg.client_id, "pub-client-xyz");
        assert!(reg.client_secret.is_none());
    }
}
