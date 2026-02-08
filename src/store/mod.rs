mod glob;
mod hash_ops;
mod list_ops;
mod set_ops;
mod string_ops;
mod ttl_ops;
pub mod value;

pub use value::StoredValue;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Thread-safe key-value store
#[derive(Debug, Clone)]
pub struct Store {
    pub(crate) data: Arc<RwLock<HashMap<String, StoredValue>>>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
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

            if !expired_keys.is_empty() {
                let mut write_guard = self.data.write().await;
                for key in expired_keys {
                    write_guard.remove(&key);
                }
            }

            let ratio = expired_count as f64 / keys_to_check.len() as f64;
            if ratio < EXPIRY_THRESHOLD {
                return;
            }
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}
