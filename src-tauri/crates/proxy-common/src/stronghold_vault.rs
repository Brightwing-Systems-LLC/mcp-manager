//! StrongholdBackend — VaultBackend implementation using IOTA Stronghold.
//!
//! Stores credentials and metadata in an encrypted Stronghold snapshot file.
//! Requires the `stronghold` feature flag.

use crate::vault::{VaultBackend, VaultError};
use async_trait::async_trait;
use iota_stronghold::{KeyProvider, SnapshotPath, Stronghold};
use std::path::PathBuf;
use tokio::sync::Mutex;

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Vault backend that persists data in an IOTA Stronghold encrypted file.
///
/// Thread-safe via tokio::sync::Mutex (Stronghold is not Send+Sync across awaits).
pub struct StrongholdBackend {
    inner: Mutex<StrongholdInner>,
}

/// Re-encrypt an existing Stronghold snapshot with a new passphrase.
///
/// Loads the snapshot using `old_passphrase`, then re-commits it encrypted with
/// `new_passphrase`. Both passphrases are SHA-256 hashed to produce 32-byte keys
/// (matching `StrongholdBackend::new` behavior).
pub fn re_encrypt_snapshot(
    path: &std::path::Path,
    old_passphrase: &[u8],
    new_passphrase: &[u8],
) -> Result<(), VaultError> {
    let stronghold = Stronghold::default();
    let snapshot_path = SnapshotPath::from_path(path);
    let client_path = b"brightwing".to_vec();

    // Load with old key
    let old_hash = Sha256::digest(old_passphrase);
    let old_kp = KeyProvider::try_from(Zeroizing::new(old_hash.to_vec()))
        .map_err(|e| VaultError::Internal(format!("Invalid old passphrase: {}", e)))?;

    stronghold
        .load_client_from_snapshot(client_path, &old_kp, &snapshot_path)
        .map_err(|e| VaultError::Io(format!("Failed to load with old key: {}", e)))?;

    // Re-commit with new key
    let new_hash = Sha256::digest(new_passphrase);
    let new_kp = KeyProvider::try_from(Zeroizing::new(new_hash.to_vec()))
        .map_err(|e| VaultError::Internal(format!("Invalid new passphrase: {}", e)))?;

    stronghold
        .commit_with_keyprovider(&snapshot_path, &new_kp)
        .map_err(|e| VaultError::Io(format!("Failed to re-encrypt: {}", e)))?;

    Ok(())
}

struct StrongholdInner {
    stronghold: Stronghold,
    snapshot_path: SnapshotPath,
    key_provider: KeyProvider,
    client_path: Vec<u8>,
}

impl StrongholdBackend {
    /// Create or open a Stronghold vault at the given path.
    ///
    /// `passphrase` is used to derive the encryption key. In production this should
    /// come from the OS keychain or a user-provided password.
    pub fn new(path: PathBuf, passphrase: &[u8]) -> Result<Self, VaultError> {
        let stronghold = Stronghold::default();
        let snapshot_path = SnapshotPath::from_path(&path);
        // Hash the passphrase to get a fixed 32-byte key (Stronghold requires it)
        let hash = Sha256::digest(passphrase);
        let key_provider = KeyProvider::try_from(Zeroizing::new(hash.to_vec()))
            .map_err(|e| VaultError::Internal(format!("Invalid passphrase: {}", e)))?;
        let client_path = b"brightwing".to_vec();

        // Try to load existing snapshot, or create a new client
        if path.exists() {
            stronghold
                .load_client_from_snapshot(client_path.clone(), &key_provider, &snapshot_path)
                .map_err(|e| VaultError::Io(format!("Failed to load Stronghold snapshot: {}", e)))?;
        } else {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| VaultError::Io(format!("Failed to create vault directory: {}", e)))?;
            }
            stronghold
                .create_client(client_path.clone())
                .map_err(|e| VaultError::Internal(format!("Failed to create Stronghold client: {}", e)))?;
        }

        Ok(Self {
            inner: Mutex::new(StrongholdInner {
                stronghold,
                snapshot_path,
                key_provider,
                client_path,
            }),
        })
    }

    /// Save the current state to disk.
    async fn commit(&self) -> Result<(), VaultError> {
        let inner = self.inner.lock().await;
        inner
            .stronghold
            .commit_with_keyprovider(&inner.snapshot_path, &inner.key_provider)
            .map_err(|e| VaultError::Io(format!("Failed to commit Stronghold: {}", e)))?;
        Ok(())
    }
}

#[async_trait]
impl VaultBackend for StrongholdBackend {
    async fn store(&self, key: &str, value: &[u8]) -> Result<(), VaultError> {
        {
            let inner = self.inner.lock().await;
            let client = inner
                .stronghold
                .get_client(inner.client_path.clone())
                .map_err(|e| VaultError::Internal(format!("Failed to get client: {}", e)))?;

            let store = client.store();
            // Store value as-is in the Stronghold store
            store
                .insert(key.as_bytes().to_vec(), value.to_vec(), None)
                .map_err(|e| VaultError::Internal(format!("Failed to store: {}", e)))?;
        }
        self.commit().await
    }

    async fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>, VaultError> {
        let inner = self.inner.lock().await;
        let client = inner
            .stronghold
            .get_client(inner.client_path.clone())
            .map_err(|e| VaultError::Internal(format!("Failed to get client: {}", e)))?;

        let store = client.store();
        match store.get(key.as_bytes()) {
            Ok(Some(data)) => Ok(Some(data)),
            Ok(None) => Ok(None),
            Err(_) => Ok(None), // Key not found
        }
    }

    async fn delete(&self, key: &str) -> Result<(), VaultError> {
        {
            let inner = self.inner.lock().await;
            let client = inner
                .stronghold
                .get_client(inner.client_path.clone())
                .map_err(|e| VaultError::Internal(format!("Failed to get client: {}", e)))?;

            let store = client.store();
            // delete returns an error if key doesn't exist — that's fine
            let _ = store.delete(key.as_bytes());
        }
        self.commit().await
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, VaultError> {
        let inner = self.inner.lock().await;
        let client = inner
            .stronghold
            .get_client(inner.client_path.clone())
            .map_err(|e| VaultError::Internal(format!("Failed to get client: {}", e)))?;

        let store = client.store();
        let keys = store
            .keys()
            .map_err(|e| VaultError::Internal(format!("Failed to list keys: {}", e)))?;

        let prefix_bytes = prefix.as_bytes();
        let matching: Vec<String> = keys
            .into_iter()
            .filter(|k| k.starts_with(prefix_bytes))
            .filter_map(|k| String::from_utf8(k).ok())
            .collect();
        Ok(matching)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_vault_path() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "bw-test-vault-{}-{}.stronghold",
            std::process::id(),
            id
        ))
    }

    fn cleanup(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let path = temp_vault_path();
        let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();

        vault.store("key1", b"value1").await.unwrap();
        let result = vault.retrieve("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_retrieve_missing_key() {
        let path = temp_vault_path();
        let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();

        let result = vault.retrieve("nonexistent").await.unwrap();
        assert_eq!(result, None);

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_overwrite() {
        let path = temp_vault_path();
        let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();

        vault.store("key1", b"original").await.unwrap();
        vault.store("key1", b"updated").await.unwrap();

        let result = vault.retrieve("key1").await.unwrap();
        assert_eq!(result, Some(b"updated".to_vec()));

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_delete() {
        let path = temp_vault_path();
        let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();

        vault.store("key1", b"value1").await.unwrap();
        vault.delete("key1").await.unwrap();

        let result = vault.retrieve("key1").await.unwrap();
        assert_eq!(result, None);

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let path = temp_vault_path();
        let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();

        // Should not error
        vault.delete("nonexistent").await.unwrap();

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_list_keys_with_prefix() {
        let path = temp_vault_path();
        let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();

        vault.store("credential:github", b"a").await.unwrap();
        vault.store("credential:jira", b"b").await.unwrap();
        vault.store("filter:github", b"c").await.unwrap();

        let mut keys = vault.list_keys("credential:").await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["credential:github", "credential:jira"]);

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_persistence_across_open() {
        let path = temp_vault_path();

        // Store data
        {
            let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();
            vault.store("persist_key", b"persist_value").await.unwrap();
        }

        // Reopen and verify
        {
            let vault = StrongholdBackend::new(path.clone(), b"test-passphrase").unwrap();
            let result = vault.retrieve("persist_key").await.unwrap();
            assert_eq!(result, Some(b"persist_value".to_vec()));
        }

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_wrong_passphrase_fails() {
        let path = temp_vault_path();

        // Create with one passphrase
        {
            let vault = StrongholdBackend::new(path.clone(), b"correct-passphrase").unwrap();
            vault.store("key", b"value").await.unwrap();
        }

        // Attempt to open with wrong passphrase
        let result = StrongholdBackend::new(path.clone(), b"wrong-passphrase");
        assert!(result.is_err());

        cleanup(&path);
    }
}
