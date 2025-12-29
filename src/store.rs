use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// A stored value with optional expiration
#[derive(Debug, Clone)]
pub struct StoredValue {
    pub data: Vec<u8>,
    pub expires_at: Option<Instant>,
}

impl StoredValue {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            expires_at: None,
        }
    }

    pub fn with_expiry(data: Vec<u8>, ttl: Duration) -> Self {
        Self {
            data,
            expires_at: Some(Instant::now() + ttl),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Instant::now() > exp)
            .unwrap_or(false)
    }
}

/// Thread-safe key-value store
#[derive(Debug, Clone)]
pub struct Store {
    data: Arc<RwLock<HashMap<String, StoredValue>>>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a value by key, returns None if key doesn't exist or is expired
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let read_guard = self.data.read().await;
        if let Some(value) = read_guard.get(key) {
            if value.is_expired() {
                drop(read_guard);
                // Lazily delete expired key
                self.data.write().await.remove(key);
                None
            } else {
                Some(value.data.clone())
            }
        } else {
            None
        }
    }

    /// Set a key to a value
    pub async fn set(&self, key: String, value: Vec<u8>) {
        let stored = StoredValue::new(value);
        self.data.write().await.insert(key, stored);
    }

    /// Set a key with expiration (in seconds)
    pub async fn set_ex(&self, key: String, value: Vec<u8>, seconds: u64) {
        let stored = StoredValue::with_expiry(value, Duration::from_secs(seconds));
        self.data.write().await.insert(key, stored);
    }

    /// Set a key only if it doesn't exist. Returns true if set, false if key already exists
    pub async fn set_nx(&self, key: String, value: Vec<u8>) -> bool {
        let mut write_guard = self.data.write().await;

        // Check if key exists and is not expired
        if let Some(existing) = write_guard.get(&key) {
            if !existing.is_expired() {
                return false;
            }
        }

        write_guard.insert(key, StoredValue::new(value));
        true
    }

    /// Delete one or more keys. Returns the number of keys deleted
    pub async fn del(&self, keys: &[String]) -> i64 {
        let mut write_guard = self.data.write().await;
        let mut deleted = 0;
        for key in keys {
            if write_guard.remove(key).is_some() {
                deleted += 1;
            }
        }
        deleted
    }

    /// Increment value by 1. Returns the new value or error if not an integer
    pub async fn incr(&self, key: &str) -> Result<i64, String> {
        self.incr_by(key, 1).await
    }

    /// Decrement value by 1. Returns the new value or error if not an integer
    pub async fn decr(&self, key: &str) -> Result<i64, String> {
        self.incr_by(key, -1).await
    }

    /// Increment value by a specific amount. Returns the new value or error if not an integer
    pub async fn incr_by(&self, key: &str, delta: i64) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        let current = if let Some(value) = write_guard.get(key) {
            if value.is_expired() {
                0
            } else {
                let s = String::from_utf8(value.data.clone())
                    .map_err(|_| "ERR value is not an integer or out of range".to_string())?;
                s.parse::<i64>()
                    .map_err(|_| "ERR value is not an integer or out of range".to_string())?
            }
        } else {
            0
        };

        let new_value = current
            .checked_add(delta)
            .ok_or_else(|| "ERR increment or decrement would overflow".to_string())?;

        write_guard.insert(
            key.to_string(),
            StoredValue::new(new_value.to_string().into_bytes()),
        );

        Ok(new_value)
    }

    /// Get multiple keys at once
    pub async fn mget(&self, keys: &[String]) -> Vec<Option<Vec<u8>>> {
        let read_guard = self.data.read().await;
        let mut results = Vec::with_capacity(keys.len());
        let mut expired_keys = Vec::new();

        for key in keys {
            if let Some(value) = read_guard.get(key) {
                if value.is_expired() {
                    expired_keys.push(key.clone());
                    results.push(None);
                } else {
                    results.push(Some(value.data.clone()));
                }
            } else {
                results.push(None);
            }
        }

        drop(read_guard);

        // Clean up expired keys
        if !expired_keys.is_empty() {
            let mut write_guard = self.data.write().await;
            for key in expired_keys {
                write_guard.remove(&key);
            }
        }

        results
    }

    /// Set multiple keys at once
    pub async fn mset(&self, pairs: Vec<(String, Vec<u8>)>) {
        let mut write_guard = self.data.write().await;
        for (key, value) in pairs {
            write_guard.insert(key, StoredValue::new(value));
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_set() {
        let store = Store::new();
        store.set("key1".to_string(), b"value1".to_vec()).await;
        assert_eq!(store.get("key1").await, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let store = Store::new();
        assert_eq!(store.get("nonexistent").await, None);
    }

    #[tokio::test]
    async fn test_del() {
        let store = Store::new();
        store.set("key1".to_string(), b"value1".to_vec()).await;
        store.set("key2".to_string(), b"value2".to_vec()).await;

        let deleted = store.del(&["key1".to_string(), "key3".to_string()]).await;
        assert_eq!(deleted, 1);
        assert_eq!(store.get("key1").await, None);
        assert_eq!(store.get("key2").await, Some(b"value2".to_vec()));
    }

    #[tokio::test]
    async fn test_set_nx() {
        let store = Store::new();

        // First set should succeed
        assert!(store.set_nx("key1".to_string(), b"value1".to_vec()).await);

        // Second set should fail
        assert!(!store.set_nx("key1".to_string(), b"value2".to_vec()).await);

        // Value should be unchanged
        assert_eq!(store.get("key1").await, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_incr_new_key() {
        let store = Store::new();
        assert_eq!(store.incr("counter").await, Ok(1));
        assert_eq!(store.incr("counter").await, Ok(2));
    }

    #[tokio::test]
    async fn test_incr_existing_key() {
        let store = Store::new();
        store.set("counter".to_string(), b"10".to_vec()).await;
        assert_eq!(store.incr("counter").await, Ok(11));
    }

    #[tokio::test]
    async fn test_incr_invalid_value() {
        let store = Store::new();
        store.set("key".to_string(), b"not a number".to_vec()).await;
        assert!(store.incr("key").await.is_err());
    }

    #[tokio::test]
    async fn test_decr() {
        let store = Store::new();
        store.set("counter".to_string(), b"10".to_vec()).await;
        assert_eq!(store.decr("counter").await, Ok(9));
    }

    #[tokio::test]
    async fn test_incr_by() {
        let store = Store::new();
        store.set("counter".to_string(), b"10".to_vec()).await;
        assert_eq!(store.incr_by("counter", 5).await, Ok(15));
        assert_eq!(store.incr_by("counter", -3).await, Ok(12));
    }

    #[tokio::test]
    async fn test_mget_mset() {
        let store = Store::new();

store
            .mset(vec![
                ("key1".to_string(), b"value1".to_vec()),
                ("key2".to_string(), b"value2".to_vec()),
            ])
    .await;

        let results = store
            .mget(&["key1".to_string(), "key2".to_string(), "key3".to_string()])
            .await;
        assert_eq!(
            results,
            vec![Some(b"value1".to_vec()), Some(b"value2".to_vec()), None,]
        );
    }

    #[tokio::test]
    async fn test_set_ex_expiry() {
        let store = Store::new();

        // Set with 1 second expiry
        store.set_ex("key".to_string(), b"value".to_vec(), 1).await;

        // Should exist immediately
        assert_eq!(store.get("key").await, Some(b"value".to_vec()));

        // Wait for expiry
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should be expired now
        assert_eq!(store.get("key").await, None);
    }
}
