use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Simple glob pattern matching supporting * (any sequence) and ? (single char)
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    glob_match_recursive(&pattern, &text, 0, 0)
}

fn glob_match_recursive(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
    // Base case: pattern exhausted
    if pi == pattern.len() {
        return ti == text.len();
    }

    match pattern[pi] {
        '*' => {
            // Try matching * with 0 or more characters
            for i in ti..=text.len() {
                if glob_match_recursive(pattern, text, pi + 1, i) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // Match exactly one character
            if ti < text.len() {
                glob_match_recursive(pattern, text, pi + 1, ti + 1)
            } else {
                false
            }
        }
        c => {
            // Match literal character
            if ti < text.len() && text[ti] == c {
                glob_match_recursive(pattern, text, pi + 1, ti + 1)
            } else {
                false
            }
        }
    }
}

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
        if let Some(existing) = write_guard.get(&key)
            && !existing.is_expired()
        {
            return false;
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

    /// Set expiration on an existing key.
    /// If seconds <= 0, deletes the key.
    /// Returns 1 if timeout was set/key was deleted, 0 if key doesn't exist.
    pub async fn expire(&self, key: &str, seconds: i64) -> i64 {
        let mut write_guard = self.data.write().await;

        // Handle negative/zero seconds - delete the key
        if seconds <= 0 {
            if let Some(value) = write_guard.get(key)
                && !value.is_expired()
            {
                write_guard.remove(key);
                return 1;
            }
            write_guard.remove(key); // Clean up if expired
            return 0;
        }

        // Set expiration on existing non-expired key
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
                        -2 // Should not happen due to is_expired check
                    }
                }
                None => -1, // Key exists but has no expiration
            }
        } else {
            -2 // Key doesn't exist
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
                0 // No expiration to remove
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

        // Clean up expired keys
        if !expired_keys.is_empty() {
            let mut write_guard = self.data.write().await;
            for key in expired_keys {
                write_guard.remove(&key);
            }
        }

        matching_keys
    }

    /// Start background task for active expiration.
    /// This should be called once when the server starts.
    pub fn start_active_expiration(store: Store) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                store.expire_random_keys().await;
            }
        })
    }

    /// Sample keys and delete expired ones.
    /// Redis samples 20 keys per cycle and continues if >25% are expired.
    async fn expire_random_keys(&self) {
        const SAMPLE_SIZE: usize = 20;
        const EXPIRY_THRESHOLD: f64 = 0.25;

        loop {
            let keys_to_check: Vec<String> = {
                let read_guard = self.data.read().await;
                if read_guard.is_empty() {
                    return;
                }
                // Sample up to SAMPLE_SIZE keys
                read_guard.keys().take(SAMPLE_SIZE).cloned().collect()
            };

            if keys_to_check.is_empty() {
                return;
            }

            let mut expired_count = 0;
            let mut expired_keys = Vec::new();

            {
                let read_guard = self.data.read().await;
                for key in &keys_to_check {
                    if let Some(value) = read_guard.get(key)
                        && value.is_expired()
                    {
                        expired_keys.push(key.clone());
                        expired_count += 1;
                    }
                }
            }

            // Delete expired keys
            if !expired_keys.is_empty() {
                let mut write_guard = self.data.write().await;
                for key in expired_keys {
                    write_guard.remove(&key);
                }
            }

            // If less than 25% were expired, stop
            let ratio = expired_count as f64 / keys_to_check.len() as f64;
            if ratio < EXPIRY_THRESHOLD {
                return;
            }
            // Otherwise, continue sampling (Redis behavior)
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

    // Glob matching tests
    #[test]
    fn test_glob_match_star() {
        // * matches any sequence including empty
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
        assert!(glob_match("foo*", "foobar"));
        assert!(glob_match("foo*", "foo"));
        assert!(glob_match("*bar", "foobar"));
        assert!(glob_match("*bar", "bar"));
        assert!(glob_match("*oba*", "foobar"));
        assert!(!glob_match("foo*", "bar"));
        assert!(!glob_match("*foo", "foobar"));
    }

    #[test]
    fn test_glob_match_question() {
        // ? matches exactly one character
        assert!(glob_match("?", "a"));
        assert!(!glob_match("?", ""));
        assert!(!glob_match("?", "ab"));
        assert!(glob_match("fo?", "foo"));
        assert!(glob_match("f??", "foo"));
        assert!(!glob_match("f?", "foo"));
        assert!(glob_match("???", "abc"));
    }

    #[test]
    fn test_glob_match_literal() {
        // Literal characters must match exactly
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exactx"));
        assert!(!glob_match("exactx", "exact"));
        assert!(!glob_match("foo", "bar"));
    }

    #[test]
    fn test_glob_match_combined() {
        // Combined patterns
        assert!(glob_match("user:*:name", "user:123:name"));
        assert!(glob_match("user:*:name", "user::name"));
        assert!(!glob_match("user:*:name", "user:123:age"));
        assert!(glob_match("key?_*", "key1_value"));
        assert!(glob_match("key?_*", "key1_"));
        assert!(!glob_match("key?_*", "key12_value"));
        assert!(glob_match("*?*", "a"));
        assert!(!glob_match("*?*", ""));
    }

    // EXPIRE tests
    #[tokio::test]
    async fn test_expire_existing_key() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        let result = store.expire("key", 10).await;
        assert_eq!(result, 1);

        // Key should still exist
        assert_eq!(store.get("key").await, Some(b"value".to_vec()));
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

        // Negative seconds should delete the key
        let result = store.expire("key", -1).await;
        assert_eq!(result, 1);

        // Key should be gone
        assert_eq!(store.get("key").await, None);
    }

    #[tokio::test]
    async fn test_expire_zero_deletes_key() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        // Zero seconds should delete the key
        let result = store.expire("key", 0).await;
        assert_eq!(result, 1);

        // Key should be gone
        assert_eq!(store.get("key").await, None);
    }

    #[tokio::test]
    async fn test_expire_causes_expiration() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;
        store.expire("key", 1).await;

        // Should exist immediately
        assert_eq!(store.get("key").await, Some(b"value".to_vec()));

        // Wait for expiry
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should be gone
        assert_eq!(store.get("key").await, None);
    }

    // TTL tests
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

    // PERSIST tests
    #[tokio::test]
    async fn test_persist_removes_expiration() {
        let store = Store::new();
        store.set_ex("key".to_string(), b"value".to_vec(), 10).await;

        let result = store.persist("key").await;
        assert_eq!(result, 1);

        // TTL should now be -1 (no expiration)
        let ttl = store.ttl("key").await;
        assert_eq!(ttl, -1);
    }

    #[tokio::test]
    async fn test_persist_key_without_expiration() {
        let store = Store::new();
        store.set("key".to_string(), b"value".to_vec()).await;

        let result = store.persist("key").await;
        assert_eq!(result, 0); // No expiration to remove
    }

    #[tokio::test]
    async fn test_persist_nonexistent_key() {
        let store = Store::new();
        let result = store.persist("nonexistent").await;
        assert_eq!(result, 0);
    }

    // KEYS tests
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

        // Wait for expiry
        tokio::time::sleep(Duration::from_secs(2)).await;

        let keys = store.keys("*").await;
        assert_eq!(keys, vec!["good"]);
    }
}
