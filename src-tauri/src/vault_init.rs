//! Vault initialization — creates or opens the Stronghold-backed encrypted vault
//! for API key storage.
//!
//! The passphrase is derived from the machine's hostname + username + a fixed salt,
//! hashed with SHA-256. This provides at-rest encryption without requiring a user
//! password — the vault is tied to the local machine.

use proxy_common::stronghold_vault::StrongholdBackend;
use proxy_common::vault::VaultBackend;
use std::path::PathBuf;
use std::sync::Arc;

/// Derive a machine-local passphrase from hostname + username + salt.
fn machine_passphrase() -> Vec<u8> {
    use sha2::{Sha256, Digest};
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown-host".to_string());
    let username = whoami::username();
    let mut hasher = Sha256::new();
    hasher.update(b"brightwing-vault-v1:");
    hasher.update(hostname.as_bytes());
    hasher.update(b":");
    hasher.update(username.as_bytes());
    hasher.finalize().to_vec()
}

/// Get the path for the vault file.
fn vault_path() -> Result<PathBuf, String> {
    let data_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?;
    Ok(data_dir.join("com.brightwing.mcp-manager").join("vault.stronghold"))
}

/// Open (or create) the encrypted vault. Returns an Arc'd trait object.
pub fn open_vault() -> Result<Arc<dyn VaultBackend>, String> {
    let path = vault_path()?;
    let passphrase = machine_passphrase();
    let backend = StrongholdBackend::new(path, &passphrase)
        .map_err(|e| format!("Failed to open vault: {}", e))?;
    Ok(Arc::new(backend))
}

/// Migrate API keys from SQLite to the vault (one-time, idempotent).
/// Keys already in the vault are skipped. After migration, the SQLite
/// rows are left in place (harmless) so we don't break downgrades.
pub async fn migrate_api_keys_to_vault(
    db: &crate::db::Database,
    vault: &dyn VaultBackend,
) {
    let keys = match db.get_all_api_keys() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("vault migration: failed to read API keys from DB: {}", e);
            return;
        }
    };

    for key in keys {
        let vault_key = format!("apikey:{}", key.server_id);
        // Skip if already migrated
        match vault.retrieve(&vault_key).await {
            Ok(Some(_)) => continue,
            Ok(None) => {}
            Err(e) => {
                eprintln!("vault migration: failed to check key {}: {}", vault_key, e);
                continue;
            }
        }
        let json = match serde_json::to_vec(&key.env) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("vault migration: failed to serialize {}: {}", key.server_id, e);
                continue;
            }
        };
        if let Err(e) = vault.store(&vault_key, &json).await {
            eprintln!("vault migration: failed to store {}: {}", key.server_id, e);
        } else {
            eprintln!("vault migration: migrated API key for '{}'", key.server_id);
        }
    }
}
