use std::time::Duration;

use super::value::{DataType, StoredValue};
use super::Store;

impl Store {
    /// Get a value by key, returns None if key doesn't exist or is expired.
    /// Returns WRONGTYPE error if key holds a non-string value.
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let read_guard = self.data.read().await;
        if let Some(value) = read_guard.get(key) {
            if value.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                Ok(None)
            } else {
                let bytes = value.expect_string()?;
                Ok(Some(bytes.clone()))
            }
        } else {
            Ok(None)
        }
    }

    /// Set a key to a value. Overwrites any existing value regardless of type.
    pub async fn set(&self, key: String, value: Vec<u8>) {
        let stored = StoredValue::new(DataType::String(value));
        self.data.write().await.insert(key, stored);
    }

    /// Set a key with expiration (in seconds). Overwrites any existing value.
    pub async fn set_ex(&self, key: String, value: Vec<u8>, seconds: u64) {
        let stored =
            StoredValue::with_expiry(DataType::String(value), Duration::from_secs(seconds));
        self.data.write().await.insert(key, stored);
    }

    /// Set a key only if it doesn't exist. Returns true if set, false if key already exists.
    /// Works regardless of existing value type (checks existence only).
    pub async fn set_nx(&self, key: String, value: Vec<u8>) -> bool {
        let mut write_guard = self.data.write().await;

        if let Some(existing) = write_guard.get(&key)
            && !existing.is_expired()
        {
            return false;
        }

        write_guard.insert(key, StoredValue::new(DataType::String(value)));
        true
    }

    /// Delete one or more keys. Returns the number of keys deleted.
    /// Works on any value type.
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

    /// Increment value by 1. Returns the new value or error if not an integer.
    pub async fn incr(&self, key: &str) -> Result<i64, String> {
        self.incr_by(key, 1).await
    }

    /// Decrement value by 1. Returns the new value or error if not an integer.
    pub async fn decr(&self, key: &str) -> Result<i64, String> {
        self.incr_by(key, -1).await
    }

    /// Increment value by a specific amount. Returns the new value or error.
    /// Returns WRONGTYPE error if key holds a non-string value.
    pub async fn incr_by(&self, key: &str, delta: i64) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        let current = if let Some(value) = write_guard.get(key) {
            if value.is_expired() {
                0
            } else {
                let bytes = value.expect_string()?;
                let s = String::from_utf8(bytes.clone())
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
            StoredValue::new(DataType::String(new_value.to_string().into_bytes())),
        );

        Ok(new_value)
    }

    /// Get multiple keys at once.
    /// Returns WRONGTYPE error if any key holds a non-string value.
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>, String> {
        let read_guard = self.data.read().await;
        let mut results = Vec::with_capacity(keys.len());
        let mut expired_keys = Vec::new();

        for key in keys {
            if let Some(value) = read_guard.get(key) {
                if value.is_expired() {
                    expired_keys.push(key.clone());
                    results.push(None);
                } else {
                    let bytes = value.expect_string()?;
                    results.push(Some(bytes.clone()));
                }
            } else {
                results.push(None);
            }
        }

        drop(read_guard);

        if !expired_keys.is_empty() {
            let mut write_guard = self.data.write().await;
            for key in expired_keys {
                write_guard.remove(&key);
            }
        }

        Ok(results)
    }

    /// Set multiple keys at once. Overwrites any existing values.
    pub async fn mset(&self, pairs: Vec<(String, Vec<u8>)>) {
        let mut write_guard = self.data.write().await;
        for (key, value) in pairs {
            write_guard.insert(key, StoredValue::new(DataType::String(value)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Store;

    #[tokio::test]
    async fn test_get_set() {
        let store = Store::new();
        store.set("key1".to_string(), b"value1".to_vec()).await;
        assert_eq!(store.get("key1").await, Ok(Some(b"value1".to_vec())));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let store = Store::new();
        assert_eq!(store.get("nonexistent").await, Ok(None));
    }

    #[tokio::test]
    async fn test_del() {
        let store = Store::new();
        store.set("key1".to_string(), b"value1".to_vec()).await;
        store.set("key2".to_string(), b"value2".to_vec()).await;

        let deleted = store.del(&["key1".to_string(), "key3".to_string()]).await;
        assert_eq!(deleted, 1);
        assert_eq!(store.get("key1").await, Ok(None));
        assert_eq!(store.get("key2").await, Ok(Some(b"value2".to_vec())));
    }

    #[tokio::test]
    async fn test_set_nx() {
        let store = Store::new();

        assert!(store.set_nx("key1".to_string(), b"value1".to_vec()).await);
        assert!(!store.set_nx("key1".to_string(), b"value2".to_vec()).await);
        assert_eq!(store.get("key1").await, Ok(Some(b"value1".to_vec())));
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
            Ok(vec![
                Some(b"value1".to_vec()),
                Some(b"value2".to_vec()),
                None,
            ])
        );
    }

    #[tokio::test]
    async fn test_set_ex_expiry() {
        let store = Store::new();

        store.set_ex("key".to_string(), b"value".to_vec(), 1).await;
        assert_eq!(store.get("key").await, Ok(Some(b"value".to_vec())));

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        assert_eq!(store.get("key").await, Ok(None));
    }

    #[tokio::test]
    async fn test_get_wrongtype_on_list() {
        use super::super::value::DataType;
        use super::super::value::StoredValue;
        use std::collections::VecDeque;

        let store = Store::new();
        store.data.write().await.insert(
            "mylist".to_string(),
            StoredValue::new(DataType::List(VecDeque::from(vec![b"a".to_vec()]))),
        );

        let result = store.get("mylist").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("WRONGTYPE"));
    }
}
