use std::collections::HashSet;

use super::value::{DataType, StoredValue};
use super::Store;

impl Store {
    /// Add members to a set. Creates the set if it doesn't exist.
    /// Returns the number of members that were added (not already present).
    pub async fn sadd(&self, key: String, members: Vec<Vec<u8>>) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        if let Some(existing) = write_guard.get(&key) {
            if existing.is_expired() {
                write_guard.remove(&key);
            }
        }

        let stored = write_guard
            .entry(key)
            .or_insert_with(|| StoredValue::new(DataType::Set(HashSet::new())));

        let set = stored.expect_set_mut()?;

        let mut added = 0;
        for member in members {
            if set.insert(member) {
                added += 1;
            }
        }

        Ok(added)
    }

    /// Remove members from a set. Auto-deletes the set when it becomes empty.
    /// Returns the number of members that were removed.
    pub async fn srem(&self, key: &str, members: Vec<Vec<u8>>) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        if let Some(stored) = write_guard.get_mut(key) {
            if stored.is_expired() {
                write_guard.remove(key);
                return Ok(0);
            }

            let set = stored.expect_set_mut()?;

            let mut removed = 0;
            for member in &members {
                if set.remove(member) {
                    removed += 1;
                }
            }

            if set.is_empty() {
                write_guard.remove(key);
            }

            Ok(removed)
        } else {
            Ok(0)
        }
    }

    /// Get all members of a set. Returns empty vec for non-existent keys.
    pub async fn smembers(&self, key: &str) -> Result<Vec<Vec<u8>>, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(Vec::new());
            }

            let set = stored.expect_set()?;
            Ok(set.iter().cloned().collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Check if a member exists in a set. Returns 1 if present, 0 otherwise.
    pub async fn sismember(&self, key: &str, member: &[u8]) -> Result<i64, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(0);
            }

            let set = stored.expect_set()?;
            Ok(if set.contains(member) { 1 } else { 0 })
        } else {
            Ok(0)
        }
    }

    /// Get the number of members in a set. Returns 0 for non-existent keys.
    pub async fn scard(&self, key: &str) -> Result<i64, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(0);
            }

            let set = stored.expect_set()?;
            Ok(set.len() as i64)
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
    async fn test_sadd_new_members() {
        let store = Store::new();
        let result = store
            .sadd("set".to_string(), vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .await;
        assert_eq!(result, Ok(3));
    }

    #[tokio::test]
    async fn test_sadd_duplicate_members() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec(), b"b".to_vec()])
            .await
            .unwrap();

        // Adding a mix of new and existing
        let result = store
            .sadd("set".to_string(), vec![b"b".to_vec(), b"c".to_vec()])
            .await;
        assert_eq!(result, Ok(1)); // Only "c" is new
    }

    #[tokio::test]
    async fn test_srem_existing_members() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .await
            .unwrap();

        let result = store
            .srem("set", vec![b"a".to_vec(), b"c".to_vec()])
            .await;
        assert_eq!(result, Ok(2));
    }

    #[tokio::test]
    async fn test_srem_nonexistent_members() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec()])
            .await
            .unwrap();

        let result = store.srem("set", vec![b"z".to_vec()]).await;
        assert_eq!(result, Ok(0));
    }

    #[tokio::test]
    async fn test_srem_nonexistent_key() {
        let store = Store::new();
        let result = store.srem("nonexistent", vec![b"a".to_vec()]).await;
        assert_eq!(result, Ok(0));
    }

    #[tokio::test]
    async fn test_srem_auto_deletes_empty_set() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec()])
            .await
            .unwrap();

        store.srem("set", vec![b"a".to_vec()]).await.unwrap();

        // Key should be gone
        let keys = store.keys("set").await;
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_smembers() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .await
            .unwrap();

        let mut members = store.smembers("set").await.unwrap();
        members.sort();
        assert_eq!(
            members,
            vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
        );
    }

    #[tokio::test]
    async fn test_smembers_nonexistent_key() {
        let store = Store::new();
        let members = store.smembers("nonexistent").await.unwrap();
        assert!(members.is_empty());
    }

    #[tokio::test]
    async fn test_sismember_present() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec(), b"b".to_vec()])
            .await
            .unwrap();

        assert_eq!(store.sismember("set", b"a").await, Ok(1));
    }

    #[tokio::test]
    async fn test_sismember_absent() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec()])
            .await
            .unwrap();

        assert_eq!(store.sismember("set", b"z").await, Ok(0));
    }

    #[tokio::test]
    async fn test_sismember_nonexistent_key() {
        let store = Store::new();
        assert_eq!(store.sismember("nonexistent", b"a").await, Ok(0));
    }

    #[tokio::test]
    async fn test_scard() {
        let store = Store::new();
        store
            .sadd("set".to_string(), vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .await
            .unwrap();

        assert_eq!(store.scard("set").await, Ok(3));
    }

    #[tokio::test]
    async fn test_scard_nonexistent_key() {
        let store = Store::new();
        assert_eq!(store.scard("nonexistent").await, Ok(0));
    }

    #[tokio::test]
    async fn test_sadd_wrongtype_on_string() {
        let store = Store::new();
        store.set("mystring".to_string(), b"value".to_vec()).await;

        let result = store
            .sadd("mystring".to_string(), vec![b"a".to_vec()])
            .await;
        assert_eq!(result, Err(WRONGTYPE_ERR.to_string()));
    }

    #[tokio::test]
    async fn test_sismember_wrongtype_on_string() {
        let store = Store::new();
        store.set("mystring".to_string(), b"value".to_vec()).await;

        let result = store.sismember("mystring", b"a").await;
        assert_eq!(result, Err(WRONGTYPE_ERR.to_string()));
    }
}
