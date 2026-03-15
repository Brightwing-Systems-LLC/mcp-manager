//! Vault initialization — creates or opens the Stronghold-backed encrypted vault
//! for API key and OAuth token storage.
//!
//! The vault encryption key is a random 32-byte secret stored in the OS keychain
//! (macOS Keychain, Windows Credential Manager, or Linux Secret Service). If the
//! keychain is unavailable, a file-based fallback (`vault.key`) is used with
//! restrictive permissions.
//!
//! On first launch after upgrading from the legacy key derivation (SHA-256 of
//! hostname + username), the vault is automatically re-encrypted with a new
//! random key and the old predictable key is discarded.

use proxy_common::stronghold_vault::StrongholdBackend;
use proxy_common::vault::VaultBackend;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const KEYCHAIN_SERVICE: &str = "com.brightwing.mcp-manager";
const KEYCHAIN_ACCOUNT: &str = "vault-key";
const FALLBACK_KEY_FILENAME: &str = "vault.key";

/// Get the path for the vault file.
fn vault_path() -> Result<PathBuf, String> {
    let data_dir = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?;
    Ok(data_dir.join("com.brightwing.mcp-manager").join("vault.stronghold"))
}

/// Path for the file-based key fallback (next to the vault file).
fn fallback_key_path(vault_path: &Path) -> PathBuf {
    vault_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(FALLBACK_KEY_FILENAME)
}

/// Generate a cryptographically random 32-byte key.
fn generate_random_key() -> Vec<u8> {
    use rand::RngCore;
    let mut key = vec![0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    key
}

/// Legacy key derivation (v1) — SHA256(hostname + username + static salt).
/// Only used during migration from old vault format.
fn legacy_machine_passphrase() -> Vec<u8> {
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

// ─── Keychain helpers ────────────────────────────────────────────────────────

/// Try to retrieve the vault key from the OS keychain.
fn try_get_keychain_key() -> Result<Option<Vec<u8>>, String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Keychain init error: {}", e))?;
    match entry.get_secret() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Keychain read error: {}", e)),
    }
}

/// Store the vault key in the OS keychain.
fn try_store_keychain_key(key: &[u8]) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Keychain init error: {}", e))?;
    entry
        .set_secret(key)
        .map_err(|e| format!("Keychain write error: {}", e))
}

// ─── File-based fallback ─────────────────────────────────────────────────────

/// Try to read the vault key from a file-based fallback.
fn try_get_fallback_key(vault_path: &Path) -> Option<Vec<u8>> {
    let key_path = fallback_key_path(vault_path);
    let data = std::fs::read(&key_path).ok()?;
    if data.len() == 32 { Some(data) } else { None }
}

/// Store the vault key to a file with restrictive permissions.
fn store_fallback_key(key: &[u8], vault_path: &Path) -> Result<(), String> {
    let key_path = fallback_key_path(vault_path);
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create key directory: {}", e))?;
    }
    std::fs::write(&key_path, key)
        .map_err(|e| format!("Failed to write fallback key: {}", e))?;

    // Restrict to owner-only (Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to set key file permissions: {}", e))?;
    }

    Ok(())
}

// ─── Key resolution ──────────────────────────────────────────────────────────

/// Store the vault key in the best available backend (keychain first, file fallback).
fn store_vault_key(key: &[u8], vault_path: &Path) -> Result<(), String> {
    match try_store_keychain_key(key) {
        Ok(()) => {
            eprintln!("vault: encryption key stored in OS keychain");
            Ok(())
        }
        Err(e) => {
            eprintln!("vault: keychain unavailable ({}), using file-based fallback", e);
            store_fallback_key(key, vault_path)
        }
    }
}

/// Resolve the vault encryption key. Handles three scenarios:
///
/// 1. Key already in fallback file or keychain → use it
/// 2. Vault file exists but no key stored → legacy upgrade, re-encrypt with new key
/// 3. No vault file, no key → fresh install, generate new key
///
/// The file-based fallback is checked FIRST because keychain access on macOS
/// triggers a system authentication prompt for unsigned/local builds. Once a key
/// is stored in the fallback file, the keychain is never consulted again.
fn get_or_create_vault_key(vault_path: &Path) -> Result<Vec<u8>, String> {
    // 1. Try file-based fallback first (no system prompt)
    if let Some(key) = try_get_fallback_key(vault_path) {
        return Ok(key);
    }

    // 2. Try keychain (may prompt on macOS for unsigned builds)
    match try_get_keychain_key() {
        Ok(Some(key)) => {
            // Persist to file so we never need keychain again
            if let Err(e) = store_fallback_key(&key, vault_path) {
                eprintln!("vault: failed to cache keychain key to file: {}", e);
            }
            return Ok(key);
        }
        Ok(None) => {}
        Err(e) => eprintln!("vault: keychain unavailable ({})", e),
    }

    // 3. If vault file exists, this is a legacy upgrade — re-encrypt
    if vault_path.exists() {
        eprintln!("vault: migrating from legacy key derivation to random key");
        let old_passphrase = legacy_machine_passphrase();
        let new_key = generate_random_key();

        proxy_common::stronghold_vault::re_encrypt_snapshot(
            vault_path,
            &old_passphrase,
            &new_key,
        )
        .map_err(|e| format!("Vault re-encryption failed: {}", e))?;

        store_vault_key(&new_key, vault_path)?;
        eprintln!("vault: successfully migrated to random encryption key");
        return Ok(new_key);
    }

    // 4. Fresh install — generate random key, store to file first
    let new_key = generate_random_key();
    // Store to file-based fallback first (no prompt), then try keychain
    store_fallback_key(&new_key, vault_path)?;
    // Best-effort store to keychain (don't fail if it prompts/errors)
    match try_store_keychain_key(&new_key) {
        Ok(()) => eprintln!("vault: encryption key stored in OS keychain and file"),
        Err(e) => eprintln!("vault: key stored in file only (keychain: {})", e),
    }
    Ok(new_key)
}

/// Open (or create) the encrypted vault. Returns an Arc'd trait object.
pub fn open_vault() -> Result<Arc<dyn VaultBackend>, String> {
    let path = vault_path()?;
    let passphrase = get_or_create_vault_key(&path)?;
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

/// Migrate OAuth tokens from plaintext SQLite to the encrypted vault (one-time, idempotent).
/// After migration, the SQLite rows are replaced with metadata-only JSON (no secrets).
pub async fn migrate_oauth_tokens_to_vault(
    db: &crate::db::Database,
    vault: &dyn VaultBackend,
) {
    use crate::oauth::types::{OAuthTokenMeta, OAuthTokenSet};

    let token_sets = match db.get_all_oauth_token_sets() {
        Ok(ts) => ts,
        Err(e) => {
            eprintln!("vault migration: failed to read OAuth tokens from DB: {}", e);
            return;
        }
    };

    for (server_id, token_json) in token_sets {
        let vault_key = format!("oauth:{}", server_id);

        // Skip if already migrated
        match vault.retrieve(&vault_key).await {
            Ok(Some(_)) => continue,
            Ok(None) => {}
            Err(e) => {
                eprintln!("vault migration: failed to check key {}: {}", vault_key, e);
                continue;
            }
        }

        // Parse the full token set from SQLite
        let token_set: OAuthTokenSet = match serde_json::from_str(&token_json) {
            Ok(ts) => ts,
            Err(e) => {
                eprintln!("vault migration: failed to parse OAuth tokens for {}: {}", server_id, e);
                continue;
            }
        };

        // Store full token set in vault
        let vault_json = match serde_json::to_vec(&token_set) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("vault migration: failed to serialize OAuth tokens for {}: {}", server_id, e);
                continue;
            }
        };
        if let Err(e) = vault.store(&vault_key, &vault_json).await {
            eprintln!("vault migration: failed to store OAuth tokens for {}: {}", server_id, e);
            continue;
        }

        // Replace SQLite row with metadata-only JSON (strip secrets)
        let meta = OAuthTokenMeta::from(&token_set);
        let meta_json = match serde_json::to_string(&meta) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("vault migration: failed to serialize OAuth metadata for {}: {}", server_id, e);
                continue;
            }
        };
        if let Err(e) = db.store_oauth_token_set(&server_id, &meta_json) {
            eprintln!("vault migration: failed to update OAuth metadata for {}: {}", server_id, e);
        } else {
            eprintln!("vault migration: migrated OAuth tokens for '{}'", server_id);
        }
    }
}
