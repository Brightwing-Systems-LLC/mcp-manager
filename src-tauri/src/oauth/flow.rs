use super::callback::{start_callback_server, CallbackParams};
use super::discovery::discover_oauth_metadata;
use super::exchange::{ExchangeParams, exchange_code};
use super::pkce;
use super::types::{OAuthError, OAuthFlowInfo, OAuthFlowState, OAuthStatus, OAuthTokenMeta, OAuthTokenSet};
use crate::db::Database;
use proxy_common::vault::VaultBackend;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Managed Tauri state holding in-progress OAuth flows and completed callbacks.
pub struct OAuthFlowStates {
    pub flows: Mutex<HashMap<String, OAuthFlowState>>,
    pub callbacks: Arc<Mutex<HashMap<String, CallbackParams>>>,
}

impl OAuthFlowStates {
    pub fn new() -> Self {
        Self {
            flows: Mutex::new(HashMap::new()),
            callbacks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Vault key for storing the full OAuth token set.
pub fn oauth_vault_key(server_id: &str) -> String {
    format!("oauth:{}", server_id)
}

/// Vault key for stored client registrations.
pub fn oauth_client_key(server_id: &str) -> String {
    format!("oauth_client:{}", server_id)
}

/// Store an OAuth token set: secrets go to vault, metadata to SQLite.
pub async fn store_token_set(
    server_id: &str,
    token_set: &OAuthTokenSet,
    vault: &dyn VaultBackend,
    db: &Database,
) -> Result<(), OAuthError> {
    // Store full token set in vault
    let vault_json = serde_json::to_vec(token_set)
        .map_err(|e| OAuthError::Internal(format!("Failed to serialize token set: {}", e)))?;
    vault
        .store(&oauth_vault_key(server_id), &vault_json)
        .await
        .map_err(|e| OAuthError::Internal(format!("Failed to store tokens in vault: {}", e)))?;

    // Store non-sensitive metadata in SQLite (for status checks)
    let meta = OAuthTokenMeta::from(token_set);
    let meta_json = serde_json::to_string(&meta)
        .map_err(|e| OAuthError::Internal(format!("Failed to serialize metadata: {}", e)))?;
    db.store_oauth_token_set(server_id, &meta_json)
        .map_err(|e| OAuthError::Internal(e))?;

    Ok(())
}

/// Retrieve the full OAuth token set from the vault.
pub async fn get_token_set_from_vault(
    server_id: &str,
    vault: &dyn VaultBackend,
) -> Result<Option<OAuthTokenSet>, OAuthError> {
    let vault_key = oauth_vault_key(server_id);
    match vault.retrieve(&vault_key).await {
        Ok(Some(data)) => {
            let ts: OAuthTokenSet = serde_json::from_slice(&data)
                .map_err(|e| OAuthError::Internal(format!("Failed to parse vault token data: {}", e)))?;
            Ok(Some(ts))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(OAuthError::Internal(format!("Vault read error: {}", e))),
    }
}

/// Start an OAuth flow: discover metadata, register client if needed, generate PKCE,
/// start callback server, and return the authorization URL for the browser.
pub async fn start_flow(
    server_id: &str,
    server_url: &str,
    client_id: Option<&str>,
    db: &Database,
    flow_states: &OAuthFlowStates,
) -> Result<OAuthFlowInfo, OAuthError> {
    // 1. Discover OAuth metadata (check cache first)
    let metadata = match db.get_oauth_metadata(server_id) {
        Ok(Some(cached)) => {
            serde_json::from_str(&cached)
                .map_err(|e| OAuthError::Internal(format!("Bad cached metadata: {}", e)))?
        }
        _ => {
            let meta = discover_oauth_metadata(server_url).await?;
            // Cache it
            let meta_json = serde_json::to_string(&meta)
                .map_err(|e| OAuthError::Internal(e.to_string()))?;
            let _ = db.store_oauth_metadata(server_id, server_url, &meta_json);
            meta
        }
    };

    // 2. Start callback server to get the port for redirect_uri
    let (port, callback_rx) = start_callback_server().await?;
    let redirect_uri = format!("http://127.0.0.1:{}/callback", port);

    // 3. Determine client_id (provided, or via dynamic registration)
    let (cid, csecret) = if let Some(id) = client_id {
        (id.to_string(), None)
    } else if let Some(reg_endpoint) = &metadata.registration_endpoint {
        let reg = register_client(reg_endpoint, &redirect_uri).await?;
        (reg.client_id, reg.client_secret)
    } else {
        return Err(OAuthError::Internal(
            "No client_id provided and server doesn't support dynamic registration".to_string(),
        ));
    };

    // 4. Generate PKCE
    let verifier = pkce::generate_verifier();
    let challenge = pkce::compute_challenge(&verifier);
    let state = pkce::generate_state();

    // 5. Build authorization URL
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}",
        metadata.authorization_endpoint,
        urlencoding::encode(&cid),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&challenge),
        urlencoding::encode(&state),
    );

    // 6. Store flow state for completion
    let flow_state = OAuthFlowState {
        server_id: server_id.to_string(),
        code_verifier: verifier,
        state: state.clone(),
        redirect_uri,
        token_endpoint: metadata.token_endpoint,
        client_id: cid,
        client_secret: csecret,
        server_url: server_url.to_string(),
    };

    {
        let mut flows = flow_states.flows.lock().map_err(|e| OAuthError::Internal(e.to_string()))?;
        flows.insert(state.clone(), flow_state);
    }

    // 7. Spawn a task to wait for the callback and store the result
    let state_clone = state.clone();
    let callbacks = Arc::clone(&flow_states.callbacks);
    tokio::spawn(async move {
        if let Ok(params) = callback_rx.await {
            log::info!("OAuth callback received for state {}", state_clone);
            if let Ok(mut cb) = callbacks.lock() {
                cb.insert(state_clone, params);
            }
        }
    });

    Ok(OAuthFlowInfo {
        auth_url,
        state,
    })
}

use super::registration::register_client;

/// Complete an OAuth flow after the callback is received.
/// If `code` is provided, use it directly. Otherwise, check for a stored callback.
pub async fn complete_flow(
    state_param: &str,
    code: Option<&str>,
    flow_states: &OAuthFlowStates,
    db: &Database,
    vault: &dyn VaultBackend,
) -> Result<OAuthTokenSet, OAuthError> {
    // Get the code from either the parameter or the stored callback
    let auth_code = if let Some(c) = code {
        c.to_string()
    } else {
        let cb = {
            let mut callbacks = flow_states.callbacks.lock()
                .map_err(|e| OAuthError::Internal(e.to_string()))?;
            callbacks.remove(state_param)
        };
        let params = cb.ok_or(OAuthError::Internal(
            "No callback received yet. The user may still be in the browser.".to_string(),
        ))?;
        if params.state != state_param {
            return Err(OAuthError::InvalidState);
        }
        params.code
    };

    let flow_state = {
        let mut flows = flow_states
            .flows
            .lock()
            .map_err(|e| OAuthError::Internal(e.to_string()))?;
        flows.remove(state_param)
    };

    let fs = flow_state.ok_or(OAuthError::InvalidState)?;

    let exchange_params = ExchangeParams {
        token_endpoint: fs.token_endpoint,
        code: auth_code,
        redirect_uri: fs.redirect_uri,
        client_id: fs.client_id,
        client_secret: fs.client_secret,
        code_verifier: fs.code_verifier,
        server_url: fs.server_url,
    };

    let token_set = exchange_code(&exchange_params).await?;

    // Store secrets in vault, metadata in SQLite
    store_token_set(&fs.server_id, &token_set, vault, db).await?;

    Ok(token_set)
}

/// Get the OAuth status for a server (reads metadata from SQLite — no vault needed).
pub fn get_status(server_id: &str, db: &Database) -> OAuthStatus {
    match db.get_oauth_token_set(server_id) {
        Ok(Some(json)) => {
            // Try parsing as metadata (new format)
            if let Ok(meta) = serde_json::from_str::<OAuthTokenMeta>(&json) {
                let is_expired = meta.expires_at.as_ref().map_or(false, |exp| {
                    chrono::DateTime::parse_from_rfc3339(exp)
                        .map_or(false, |dt| dt < chrono::Utc::now())
                });
                return OAuthStatus {
                    status: if is_expired { "expired" } else { "connected" }.to_string(),
                    expires_at: meta.expires_at,
                    error_message: None,
                };
            }
            // Fallback: try parsing as full token set (legacy format)
            match serde_json::from_str::<OAuthTokenSet>(&json) {
                Ok(ts) => {
                    let is_expired = ts.expires_at.as_ref().map_or(false, |exp| {
                        chrono::DateTime::parse_from_rfc3339(exp)
                            .map_or(false, |dt| dt < chrono::Utc::now())
                    });
                    OAuthStatus {
                        status: if is_expired { "expired" } else { "connected" }.to_string(),
                        expires_at: ts.expires_at,
                        error_message: None,
                    }
                }
                Err(e) => OAuthStatus {
                    status: "error".to_string(),
                    expires_at: None,
                    error_message: Some(format!("Corrupt token data: {}", e)),
                },
            }
        }
        Ok(None) => OAuthStatus {
            status: "disconnected".to_string(),
            expires_at: None,
            error_message: None,
        },
        Err(e) => OAuthStatus {
            status: "error".to_string(),
            expires_at: None,
            error_message: Some(e),
        },
    }
}

/// Disconnect OAuth for a server (delete stored tokens from both vault and SQLite).
pub async fn disconnect(
    server_id: &str,
    db: &Database,
    vault: &dyn VaultBackend,
) -> Result<(), OAuthError> {
    // Delete from vault
    let _ = vault.delete(&oauth_vault_key(server_id)).await;

    // Delete from SQLite
    db.delete_oauth_token_set(server_id)
        .map_err(|e| OAuthError::Internal(e))?;
    db.delete_oauth_metadata(server_id)
        .map_err(|e| OAuthError::Internal(e))?;
    Ok(())
}
