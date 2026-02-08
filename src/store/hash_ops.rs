use std::collections::HashMap;

use super::Store;
use super::value::{DataType, StoredValue};

impl Store {
    /// Set fields in a hash. Creates the hash if it doesn't exist.
    /// Returns the number of fields that were added (not updated).
    pub async fn hset(&self, key: String, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        if let Some(existing) = write_guard.get(&key)
            && existing.is_expired()
        {
            write_guard.remove(&key);
        }

        let stored = write_guard
            .entry(key)
            .or_insert_with(|| StoredValue::new(DataType::Hash(HashMap::new())));

        let hash = stored.expect_hash_mut()?;

        let mut added = 0;
        for (field, value) in pairs {
            if hash.insert(field, value).is_none() {
                added += 1;
            }
        }

        Ok(added)
    }

    /// Get the value of a field in a hash.
    /// Returns None if the key or field doesn't exist.
    pub async fn hget(&self, key: &str, field: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(None);
            }

            let hash = stored.expect_hash()?;
            Ok(hash.get(field).cloned())
        } else {
            Ok(None)
        }
    }

    /// Delete fields from a hash. Auto-deletes the hash when it becomes empty.
    /// Returns the number of fields that were removed.
    pub async fn hdel(&self, key: &str, fields: Vec<Vec<u8>>) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        if let Some(stored) = write_guard.get_mut(key) {
            if stored.is_expired() {
                write_guard.remove(key);
                return Ok(0);
            }

            let hash = stored.expect_hash_mut()?;

            let mut removed = 0;
            for field in &fields {
                if hash.remove(field).is_some() {
                    removed += 1;
                }
            }

            if hash.is_empty() {
                write_guard.remove(key);
            }

            Ok(removed)
        } else {
            Ok(0)
        }
    }

    /// Get all field-value pairs from a hash.
    /// Returns empty vec for non-existent keys.
    pub async fn hgetall(&self, key: &str) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(Vec::new());
            }

            let hash = stored.expect_hash()?;
            Ok(hash.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get the number of fields in a hash. Returns 0 for non-existent keys.
    pub async fn hlen(&self, key: &str) -> Result<i64, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(0);
            }

            let hash = stored.expect_hash()?;
            Ok(hash.len() as i64)
        } else {
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Store;
    use super::super::value::WRONGTYPE_ERR;

    #[tokio::test]
    async fn test_hset_new_fields() {
        let store = Store::new();
        let result = store
            .hset(
                "hash".to_string(),
                vec![
                    (b"f1".to_vec(), b"v1".to_vec()),
                    (b"f2".to_vec(), b"v2".to_vec()),
                ],
            )
            .await;
        assert_eq!(result, Ok(2));
    }

    #[tokio::test]
    async fn test_hset_update_existing_field() {
        let store = Store::new();
        store
            .hset("hash".to_string(), vec![(b"f1".to_vec(), b"v1".to_vec())])
            .await
            .unwrap();

        // Update f1, add f2
        let result = store
            .hset(
                "hash".to_string(),
                vec![
                    (b"f1".to_vec(), b"updated".to_vec()),
                    (b"f2".to_vec(), b"v2".to_vec()),
                ],
            )
            .await;
        assert_eq!(result, Ok(1)); // Only f2 is new

        let value = store.hget("hash", b"f1").await.unwrap();
        assert_eq!(value, Some(b"updated".to_vec()));
    }

    #[tokio::test]
    async fn test_hget_existing_field() {
        let store = Store::new();
        store
            .hset("hash".to_string(), vec![(b"f1".to_vec(), b"v1".to_vec())])
            .await
            .unwrap();

        assert_eq!(store.hget("hash", b"f1").await, Ok(Some(b"v1".to_vec())));
    }

    #[tokio::test]
    async fn test_hget_nonexistent_field() {
        let store = Store::new();
        store
            .hset("hash".to_string(), vec![(b"f1".to_vec(), b"v1".to_vec())])
            .await
            .unwrap();

        assert_eq!(store.hget("hash", b"nonexistent").await, Ok(None));
    }

    #[tokio::test]
    async fn test_hget_nonexistent_key() {
        let store = Store::new();
        assert_eq!(store.hget("nonexistent", b"f1").await, Ok(None));
    }

    #[tokio::test]
    async fn test_hdel_existing_fields() {
        let store = Store::new();
        store
            .hset(
                "hash".to_string(),
                vec![
                    (b"f1".to_vec(), b"v1".to_vec()),
                    (b"f2".to_vec(), b"v2".to_vec()),
                    (b"f3".to_vec(), b"v3".to_vec()),
                ],
            )
            .await
            .unwrap();

        let result = store
            .hdel("hash", vec![b"f1".to_vec(), b"f3".to_vec()])
            .await;
        assert_eq!(result, Ok(2));

        assert_eq!(store.hget("hash", b"f1").await, Ok(None));
        assert_eq!(store.hget("hash", b"f2").await, Ok(Some(b"v2".to_vec())));
    }

    #[tokio::test]
    async fn test_hdel_nonexistent_fields() {
        let store = Store::new();
        store
            .hset("hash".to_string(), vec![(b"f1".to_vec(), b"v1".to_vec())])
            .await
            .unwrap();

        let result = store.hdel("hash", vec![b"nonexistent".to_vec()]).await;
        assert_eq!(result, Ok(0));
    }

    #[tokio::test]
    async fn test_hdel_nonexistent_key() {
        let store = Store::new();
        let result = store.hdel("nonexistent", vec![b"f1".to_vec()]).await;
        assert_eq!(result, Ok(0));
    }

    #[tokio::test]
    async fn test_hdel_auto_deletes_empty_hash() {
        let store = Store::new();
        store
            .hset("hash".to_string(), vec![(b"f1".to_vec(), b"v1".to_vec())])
            .await
            .unwrap();

        store.hdel("hash", vec![b"f1".to_vec()]).await.unwrap();

        let keys = store.keys("hash").await;
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_hgetall() {
        let store = Store::new();
        store
            .hset(
                "hash".to_string(),
                vec![
                    (b"f1".to_vec(), b"v1".to_vec()),
                    (b"f2".to_vec(), b"v2".to_vec()),
                ],
            )
            .await
            .unwrap();

        let mut pairs = store.hgetall("hash").await.unwrap();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            pairs,
            vec![
                (b"f1".to_vec(), b"v1".to_vec()),
                (b"f2".to_vec(), b"v2".to_vec()),
            ]
        );
    }

    #[tokio::test]
    async fn test_hgetall_nonexistent_key() {
        let store = Store::new();
        let pairs = store.hgetall("nonexistent").await.unwrap();
        assert!(pairs.is_empty());
    }

    #[tokio::test]
    async fn test_hlen() {
        let store = Store::new();
        store
            .hset(
                "hash".to_string(),
                vec![
                    (b"f1".to_vec(), b"v1".to_vec()),
                    (b"f2".to_vec(), b"v2".to_vec()),
                ],
            )
            .await
            .unwrap();

        assert_eq!(store.hlen("hash").await, Ok(2));
    }

    #[tokio::test]
    async fn test_hlen_nonexistent_key() {
        let store = Store::new();
        assert_eq!(store.hlen("nonexistent").await, Ok(0));
    }

    #[tokio::test]
    async fn test_hset_wrongtype_on_string() {
        let store = Store::new();
        store.set("mystring".to_string(), b"value".to_vec()).await;

        let result = store
            .hset(
                "mystring".to_string(),
                vec![(b"f1".to_vec(), b"v1".to_vec())],
            )
            .await;
        assert_eq!(result, Err(WRONGTYPE_ERR.to_string()));
    }

    #[tokio::test]
    async fn test_hget_wrongtype_on_string() {
        let store = Store::new();
        store.set("mystring".to_string(), b"value".to_vec()).await;

        let result = store.hget("mystring", b"f1").await;
        assert_eq!(result, Err(WRONGTYPE_ERR.to_string()));
    }
}
