use std::collections::VecDeque;

use super::Store;
use super::value::{DataType, StoredValue};

/// Normalize a Redis-style index range to bounded usize values.
/// Returns None if the normalized range is empty or the list is empty.
/// Negative indices count from the end: -1 = last, -2 = second-to-last.
/// Out-of-bounds start is clamped to 0, out-of-bounds stop is clamped to len-1.
fn normalize_range(start: i64, stop: i64, len: usize) -> Option<(usize, usize)> {
    if len == 0 {
        return None;
    }

    let len = len as i64;

    let start = if start < 0 {
        (len + start).max(0)
    } else {
        start
    };

    let stop = if stop < 0 {
        (len + stop).max(0)
    } else {
        stop.min(len - 1)
    };

    if start > stop || start >= len {
        return None;
    }

    Some((start as usize, stop as usize))
}

impl Store {
    /// Push elements to the head of a list. Creates the list if it doesn't exist.
    /// Returns the new length of the list or WRONGTYPE error.
    pub async fn lpush(&self, key: String, elements: Vec<Vec<u8>>) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        // If key exists but is expired, remove it first
        if let Some(existing) = write_guard.get(&key)
            && existing.is_expired()
        {
            write_guard.remove(&key);
        }

        let stored = write_guard
            .entry(key)
            .or_insert_with(|| StoredValue::new(DataType::List(VecDeque::new())));

        let list = stored.expect_list_mut()?;

        for elem in elements {
            list.push_front(elem);
        }

        Ok(list.len() as i64)
    }

    /// Push elements to the tail of a list. Creates the list if it doesn't exist.
    /// Returns the new length of the list or WRONGTYPE error.
    pub async fn rpush(&self, key: String, elements: Vec<Vec<u8>>) -> Result<i64, String> {
        let mut write_guard = self.data.write().await;

        if let Some(existing) = write_guard.get(&key)
            && existing.is_expired()
        {
            write_guard.remove(&key);
        }

        let stored = write_guard
            .entry(key)
            .or_insert_with(|| StoredValue::new(DataType::List(VecDeque::new())));

        let list = stored.expect_list_mut()?;

        for elem in elements {
            list.push_back(elem);
        }

        Ok(list.len() as i64)
    }

    /// Remove and return the first element from a list.
    /// Returns None if the key doesn't exist. Auto-deletes empty lists.
    pub async fn lpop(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let mut write_guard = self.data.write().await;

        if let Some(stored) = write_guard.get_mut(key) {
            if stored.is_expired() {
                write_guard.remove(key);
                return Ok(None);
            }

            let list = stored.expect_list_mut()?;
            let result = list.pop_front();

            if list.is_empty() {
                write_guard.remove(key);
            }

            Ok(result)
        } else {
            Ok(None)
        }
    }

    /// Remove and return the last element from a list.
    /// Returns None if the key doesn't exist. Auto-deletes empty lists.
    pub async fn rpop(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let mut write_guard = self.data.write().await;

        if let Some(stored) = write_guard.get_mut(key) {
            if stored.is_expired() {
                write_guard.remove(key);
                return Ok(None);
            }

            let list = stored.expect_list_mut()?;
            let result = list.pop_back();

            if list.is_empty() {
                write_guard.remove(key);
            }

            Ok(result)
        } else {
            Ok(None)
        }
    }

    /// Get a range of elements from a list. Supports negative indices.
    /// Returns empty vec for non-existent keys or empty ranges.
    pub async fn lrange(&self, key: &str, start: i64, stop: i64) -> Result<Vec<Vec<u8>>, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(Vec::new());
            }

            let list = stored.expect_list()?;

            match normalize_range(start, stop, list.len()) {
                Some((start, stop)) => {
                    let result = list
                        .iter()
                        .skip(start)
                        .take(stop - start + 1)
                        .cloned()
                        .collect();
                    Ok(result)
                }
                None => Ok(Vec::new()),
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Get the length of a list. Returns 0 for non-existent keys.
    pub async fn llen(&self, key: &str) -> Result<i64, String> {
        let read_guard = self.data.read().await;

        if let Some(stored) = read_guard.get(key) {
            if stored.is_expired() {
                drop(read_guard);
                self.data.write().await.remove(key);
                return Ok(0);
            }

            let list = stored.expect_list()?;
            Ok(list.len() as i64)
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
    async fn test_lpush_single() {
        let store = Store::new();
        let result = store.lpush("list".to_string(), vec![b"a".to_vec()]).await;
        assert_eq!(result, Ok(1));
    }

    #[tokio::test]
    async fn test_lpush_multiple() {
        let store = Store::new();
        // LPUSH mylist a b c → list is [c, b, a]
        let result = store
            .lpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await;
        assert_eq!(result, Ok(3));

        let items = store.lrange("list", 0, -1).await.unwrap();
        assert_eq!(items, vec![b"c".to_vec(), b"b".to_vec(), b"a".to_vec()]);
    }

    #[tokio::test]
    async fn test_rpush_single() {
        let store = Store::new();
        let result = store.rpush("list".to_string(), vec![b"a".to_vec()]).await;
        assert_eq!(result, Ok(1));
    }

    #[tokio::test]
    async fn test_rpush_multiple() {
        let store = Store::new();
        // RPUSH mylist a b c → list is [a, b, c]
        let result = store
            .rpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await;
        assert_eq!(result, Ok(3));

        let items = store.lrange("list", 0, -1).await.unwrap();
        assert_eq!(items, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[tokio::test]
    async fn test_lpop_from_list() {
        let store = Store::new();
        store
            .rpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await
            .unwrap();

        assert_eq!(store.lpop("list").await, Ok(Some(b"a".to_vec())));
        assert_eq!(store.lpop("list").await, Ok(Some(b"b".to_vec())));
        assert_eq!(store.lpop("list").await, Ok(Some(b"c".to_vec())));
        assert_eq!(store.lpop("list").await, Ok(None)); // empty, key removed
    }

    #[tokio::test]
    async fn test_rpop_from_list() {
        let store = Store::new();
        store
            .rpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await
            .unwrap();

        assert_eq!(store.rpop("list").await, Ok(Some(b"c".to_vec())));
        assert_eq!(store.rpop("list").await, Ok(Some(b"b".to_vec())));
        assert_eq!(store.rpop("list").await, Ok(Some(b"a".to_vec())));
        assert_eq!(store.rpop("list").await, Ok(None));
    }

    #[tokio::test]
    async fn test_lpop_nonexistent_key() {
        let store = Store::new();
        assert_eq!(store.lpop("nonexistent").await, Ok(None));
    }

    #[tokio::test]
    async fn test_rpop_nonexistent_key() {
        let store = Store::new();
        assert_eq!(store.rpop("nonexistent").await, Ok(None));
    }

    #[tokio::test]
    async fn test_lrange_full() {
        let store = Store::new();
        store
            .rpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await
            .unwrap();

        let result = store.lrange("list", 0, -1).await;
        assert_eq!(
            result,
            Ok(vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
        );
    }

    #[tokio::test]
    async fn test_lrange_partial() {
        let store = Store::new();
        store
            .rpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await
            .unwrap();

        let result = store.lrange("list", 0, 1).await;
        assert_eq!(result, Ok(vec![b"a".to_vec(), b"b".to_vec()]));
    }

    #[tokio::test]
    async fn test_lrange_negative_indices() {
        let store = Store::new();
        store
            .rpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await
            .unwrap();

        // Last two elements
        let result = store.lrange("list", -2, -1).await;
        assert_eq!(result, Ok(vec![b"b".to_vec(), b"c".to_vec()]));
    }

    #[tokio::test]
    async fn test_lrange_out_of_bounds() {
        let store = Store::new();
        store
            .rpush("list".to_string(), vec![b"a".to_vec(), b"b".to_vec()])
            .await
            .unwrap();

        // Range extends beyond list - should clamp
        let result = store.lrange("list", 0, 100).await;
        assert_eq!(result, Ok(vec![b"a".to_vec(), b"b".to_vec()]));
    }

    #[tokio::test]
    async fn test_lrange_empty_range() {
        let store = Store::new();
        store
            .rpush("list".to_string(), vec![b"a".to_vec()])
            .await
            .unwrap();

        let result = store.lrange("list", 2, 1).await;
        assert_eq!(result, Ok(vec![]));
    }

    #[tokio::test]
    async fn test_lrange_nonexistent_key() {
        let store = Store::new();
        let result = store.lrange("nonexistent", 0, -1).await;
        assert_eq!(result, Ok(vec![]));
    }

    #[tokio::test]
    async fn test_llen() {
        let store = Store::new();
        store
            .rpush(
                "list".to_string(),
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            )
            .await
            .unwrap();

        assert_eq!(store.llen("list").await, Ok(3));
    }

    #[tokio::test]
    async fn test_llen_nonexistent_key() {
        let store = Store::new();
        assert_eq!(store.llen("nonexistent").await, Ok(0));
    }

    #[tokio::test]
    async fn test_lpush_wrongtype_on_string() {
        let store = Store::new();
        store.set("mystring".to_string(), b"value".to_vec()).await;

        let result = store
            .lpush("mystring".to_string(), vec![b"a".to_vec()])
            .await;
        assert_eq!(result, Err(WRONGTYPE_ERR.to_string()));
    }

    #[tokio::test]
    async fn test_llen_wrongtype_on_string() {
        let store = Store::new();
        store.set("mystring".to_string(), b"value".to_vec()).await;

        let result = store.llen("mystring").await;
        assert_eq!(result, Err(WRONGTYPE_ERR.to_string()));
    }

    #[tokio::test]
    async fn test_auto_delete_empty_list_after_pop() {
        let store = Store::new();
        store
            .rpush("list".to_string(), vec![b"a".to_vec()])
            .await
            .unwrap();

        store.lpop("list").await.unwrap();

        // Key should no longer exist
        assert_eq!(store.llen("list").await, Ok(0));
        // KEYS should not return it
        let keys = store.keys("list").await;
        assert!(keys.is_empty());
    }
}
