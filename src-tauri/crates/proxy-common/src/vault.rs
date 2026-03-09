use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

/// Abstraction over credential storage (Decision D2).
/// Production uses Stronghold, tests use InMemoryVaultBackend.
#[async_trait]
pub trait VaultBackend: Send + Sync {
    /// Store a value under the given key. Overwrites if key exists.
    async fn store(&self, key: &str, value: &[u8]) -> Result<(), VaultError>;

    /// Retrieve a value by key. Returns None if not found.
    async fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>, VaultError>;

    /// Delete a key. No-op if key doesn't exist.
    async fn delete(&self, key: &str) -> Result<(), VaultError>;

    /// List all keys matching a prefix.
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, VaultError>;
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault is locked or inaccessible: {0}")]
    Locked(String),

    #[error("vault I/O error: {0}")]
    Io(String),

    #[error("vault internal error: {0}")]
    Internal(String),
}

/// In-memory vault for testing. Thread-safe via Mutex.
pub struct InMemoryVaultBackend {
    store: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemoryVaultBackend {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryVaultBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VaultBackend for InMemoryVaultBackend {
    async fn store(&self, key: &str, value: &[u8]) -> Result<(), VaultError> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| VaultError::Internal(e.to_string()))?;
        store.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>, VaultError> {
        let store = self
            .store
            .lock()
            .map_err(|e| VaultError::Internal(e.to_string()))?;
        Ok(store.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> Result<(), VaultError> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| VaultError::Internal(e.to_string()))?;
        store.remove(key);
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, VaultError> {
        let store = self
            .store
            .lock()
            .map_err(|e| VaultError::Internal(e.to_string()))?;
        let keys: Vec<String> = store
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let vault = InMemoryVaultBackend::new();
        vault.store("key1", b"value1").await.unwrap();

        let result = vault.retrieve("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_retrieve_missing_key() {
        let vault = InMemoryVaultBackend::new();

        let result = vault.retrieve("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_overwrite() {
        let vault = InMemoryVaultBackend::new();
        vault.store("key1", b"original").await.unwrap();
        vault.store("key1", b"updated").await.unwrap();

        let result = vault.retrieve("key1").await.unwrap();
        assert_eq!(result, Some(b"updated".to_vec()));
    }

    #[tokio::test]
    async fn test_delete() {
        let vault = InMemoryVaultBackend::new();
        vault.store("key1", b"value1").await.unwrap();
        vault.delete("key1").await.unwrap();

        let result = vault.retrieve("key1").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let vault = InMemoryVaultBackend::new();
        // Should not error
        vault.delete("nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn test_list_keys_with_prefix() {
        let vault = InMemoryVaultBackend::new();
        vault.store("secret:github:TOKEN", b"a").await.unwrap();
        vault.store("secret:github:KEY", b"b").await.unwrap();
        vault.store("secret:jira:TOKEN", b"c").await.unwrap();
        vault.store("oauth:github", b"d").await.unwrap();

        let mut keys = vault.list_keys("secret:github:").await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["secret:github:KEY", "secret:github:TOKEN"]);
    }

    #[tokio::test]
    async fn test_list_keys_empty_prefix() {
        let vault = InMemoryVaultBackend::new();
        vault.store("a", b"1").await.unwrap();
        vault.store("b", b"2").await.unwrap();

        let keys = vault.list_keys("").await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_list_keys_no_match() {
        let vault = InMemoryVaultBackend::new();
        vault.store("secret:github:TOKEN", b"a").await.unwrap();

        let keys = vault.list_keys("oauth:").await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_store_binary_data() {
        let vault = InMemoryVaultBackend::new();
        let binary_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        vault.store("binary", &binary_data).await.unwrap();

        let result = vault.retrieve("binary").await.unwrap();
        assert_eq!(result, Some(binary_data));
    }
}
