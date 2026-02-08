use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

pub const WRONGTYPE_ERR: &str =
    "WRONGTYPE Operation against a key holding the wrong kind of value";

/// Represents the different data types a Redis key can hold
#[derive(Debug, Clone)]
pub enum DataType {
    String(Vec<u8>),
    List(VecDeque<Vec<u8>>),
    Set(HashSet<Vec<u8>>),
    Hash(HashMap<Vec<u8>, Vec<u8>>),
}

/// A stored value with optional expiration
#[derive(Debug, Clone)]
pub struct StoredValue {
    pub data: DataType,
    pub expires_at: Option<Instant>,
}

impl StoredValue {
    pub fn new(data: DataType) -> Self {
        Self {
            data,
            expires_at: None,
        }
    }

    pub fn with_expiry(data: DataType, ttl: Duration) -> Self {
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

    pub fn expect_string(&self) -> Result<&Vec<u8>, String> {
        match &self.data {
            DataType::String(bytes) => Ok(bytes),
            _ => Err(WRONGTYPE_ERR.to_string()),
        }
    }

    pub fn expect_list(&self) -> Result<&VecDeque<Vec<u8>>, String> {
        match &self.data {
            DataType::List(list) => Ok(list),
            _ => Err(WRONGTYPE_ERR.to_string()),
        }
    }

    pub fn expect_list_mut(&mut self) -> Result<&mut VecDeque<Vec<u8>>, String> {
        match &mut self.data {
            DataType::List(list) => Ok(list),
            _ => Err(WRONGTYPE_ERR.to_string()),
        }
    }

    pub fn expect_set(&self) -> Result<&HashSet<Vec<u8>>, String> {
        match &self.data {
            DataType::Set(set) => Ok(set),
            _ => Err(WRONGTYPE_ERR.to_string()),
        }
    }

    pub fn expect_set_mut(&mut self) -> Result<&mut HashSet<Vec<u8>>, String> {
        match &mut self.data {
            DataType::Set(set) => Ok(set),
            _ => Err(WRONGTYPE_ERR.to_string()),
        }
    }

    pub fn expect_hash(&self) -> Result<&HashMap<Vec<u8>, Vec<u8>>, String> {
        match &self.data {
            DataType::Hash(hash) => Ok(hash),
            _ => Err(WRONGTYPE_ERR.to_string()),
        }
    }

    pub fn expect_hash_mut(&mut self) -> Result<&mut HashMap<Vec<u8>, Vec<u8>>, String> {
        match &mut self.data {
            DataType::Hash(hash) => Ok(hash),
            _ => Err(WRONGTYPE_ERR.to_string()),
        }
    }
}
