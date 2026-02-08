use std::time::{Duration, Instant};

use super::Store;
use super::glob::glob_match;

impl Store {
    /// Set expiration on an existing key.
    /// If seconds <= 0, deletes the key.
    /// Returns 1 if timeout was set/key was deleted, 0 if key doesn't exist.
    pub async fn expire(&self, key: &str, seconds: i64) -> i64 {
        let mut write_guard = self.data.write().await;

        if seconds <= 0 {
            if let Some(value) = write_guard.get(key)
                && !value.is_expired()
            {
                write_guard.remove(key);
                return 1;
            }
            write_guard.remove(key);
            return 0;
        }

        if let Some(value) = write_guard.get_mut(key) {
            if value.is_expired() {
                write_guard.remove(key);
                return 0;
            }
            value.expires_at = Some(Instant::now() + Duration::from_secs(seconds as u64));
            1
        } else {
            0
        }
    }

    /// Get TTL of a key in seconds.
    /// Returns -2 if key doesn't exist, -1 if key has no expiry, or remaining seconds.
    pub async fn ttl(&self, key: &str) -> i64 {
        let read_guard = self.data.read().await;

        if let Some(value) = read_guard.get(key) {
            if value.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return -2;
            }
            match value.expires_at {
                Some(expires_at) => {
                    let now = Instant::now();
                    if expires_at > now {
                        (expires_at - now).as_secs() as i64
                    } else {
                        -2
                    }
                }
                None => -1,
            }
        } else {
            -2
        }
    }

    /// Remove expiration from a key.
    /// Returns 1 if expiration was removed, 0 if key doesn't exist or had no expiry.
    pub async fn persist(&self, key: &str) -> i64 {
        let mut write_guard = self.data.write().await;

        if let Some(value) = write_guard.get_mut(key) {
            if value.is_expired() {
                write_guard.remove(key);
                return 0;
            }
            if value.expires_at.is_some() {
                value.expires_at = None;
                1
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Get all keys matching a glob pattern. Supports * and ? wildcards.
    pub async fn keys(&self, pattern: &str) -> Vec<String> {
        let read_guard = self.data.read().await;
        let mut matching_keys = Vec::new();
        let mut expired_keys = Vec::new();

        for (key, value) in read_guard.iter() {
            if value.is_expired() {
                expired_keys.push(key.clone());
            } else if glob_match(pattern, key) {
                matching_keys.push(key.clone());
            }
        }

        drop(read_guard);

        if !expired_keys.is_empty() {
            let mut write_guard = self.data.write().await;
            for key in expired_keys {
                write_guard.remove(&key);
            }
        }

        matching_keys
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::Store;

    #[tokio::test]
    async fn test_expire_existing_key() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        let result = store.expire("key", 10).await;
        assert_eq!(result, 1);
        assert_eq!(store.get("key").await, Ok(Some(b"value".to_vec())));
    }

    #[tokio::test]
    async fn test_expire_nonexistent_key() {
        let store = Store::new();
        let result = store.expire("nonexistent", 10).await;
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_expire_negative_deletes_key() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        let result = store.expire("key", -1).await;
        assert_eq!(result, 1);
        assert_eq!(store.get("key").await, Ok(None));
    }

    #[tokio::test]
    async fn test_expire_zero_deletes_key() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        let result = store.expire("key", 0).await;
        assert_eq!(result, 1);
        assert_eq!(store.get("key").await, Ok(None));
    }

    #[tokio::test]
    async fn test_expire_causes_expiration() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;
        store.expire("key", 1).await;

        assert_eq!(store.get("key").await, Ok(Some(b"value".to_vec())));

        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(store.get("key").await, Ok(None));
    }

    #[tokio::test]
    async fn test_ttl_with_expiration() {
        let store = Store::new();
        store.set_ex("key".to_string(), b"value".to_vec(), 10).await;

        let ttl = store.ttl("key").await;
        assert!(ttl >= 9 && ttl <= 10);
    }

    #[tokio::test]
    async fn test_ttl_no_expiration() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        let ttl = store.ttl("key").await;
        assert_eq!(ttl, -1);
    }

    #[tokio::test]
    async fn test_ttl_nonexistent_key() {
        let store = Store::new();
        let ttl = store.ttl("nonexistent").await;
        assert_eq!(ttl, -2);
    }

    #[tokio::test]
    async fn test_persist_removes_expiration() {
        let store = Store::new();
        store.set_ex("key".to_string(), b"value".to_vec(), 10).await;

        let result = store.persist("key").await;
        assert_eq!(result, 1);

        let ttl = store.ttl("key").await;
        assert_eq!(ttl, -1);
    }

    #[tokio::test]
    async fn test_persist_key_without_expiration() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        let result = store.persist("key").await;
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_persist_nonexistent_key() {
        let store = Store::new();
        let result = store.persist("nonexistent").await;
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_keys_all() {
        let store = Store::new();
        store.set("foo".to_string(), b"1".to_vec()).await;
        store.set("bar".to_string(), b"2".to_vec()).await;
        store.set("baz".to_string(), b"3".to_vec()).await;

        let mut keys = store.keys("*").await;
        keys.sort();
        assert_eq!(keys, vec!["bar", "baz", "foo"]);
    }

    #[tokio::test]
    async fn test_keys_prefix_pattern() {
        let store = Store::new();
        store.set("user:1".to_string(), b"a".to_vec()).await;
        store.set("user:2".to_string(), b"b".to_vec()).await;
        store.set("session:1".to_string(), b"c".to_vec()).await;

        let mut keys = store.keys("user:*").await;
        keys.sort();
        assert_eq!(keys, vec!["user:1", "user:2"]);
    }

    #[tokio::test]
    async fn test_keys_single_char_wildcard() {
        let store = Store::new();
        store.set("key1".to_string(), b"a".to_vec()).await;
        store.set("key2".to_string(), b"b".to_vec()).await;
        store.set("key10".to_string(), b"c".to_vec()).await;

        let mut keys = store.keys("key?").await;
        keys.sort();
        assert_eq!(keys, vec!["key1", "key2"]);
    }

    #[tokio::test]
    async fn test_keys_excludes_expired() {
        let store = Store::new();
        store.set("good".to_string(), b"value".to_vec()).await;
        store
            .set_ex("expired".to_string(), b"value".to_vec(), 1)
            .await;

        tokio::time::sleep(Duration::from_secs(2)).await;

        let keys = store.keys("*").await;
        assert_eq!(keys, vec!["good"]);
    }
}
