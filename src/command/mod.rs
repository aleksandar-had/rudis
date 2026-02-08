mod parse;
mod string_cmds;
mod ttl_cmds;

use crate::resp::RespValue;
use crate::store::Store;
use anyhow::{anyhow, Result};
use parse::extract_bulk_string;

/// Represents a Redis command
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Ping(Option<String>),
    Get(String),
    Set(String, Vec<u8>),
    Del(Vec<String>),
    SetNx(String, Vec<u8>),
    SetEx(String, u64, Vec<u8>),
    Incr(String),
    Decr(String),
    IncrBy(String, i64),
    DecrBy(String, i64),
    MGet(Vec<String>),
    MSet(Vec<(String, Vec<u8>)>),
    Expire(String, i64),
    Ttl(String),
    Persist(String),
    Keys(String),
}

impl Command {
    /// Parse a RESP array into a command
    pub fn from_resp(value: RespValue) -> Result<Self> {
        match value {
            RespValue::Array(Some(elements)) if !elements.is_empty() => {
                let cmd_name = extract_bulk_string(&elements[0])?;
                let args = &elements[1..];

                match cmd_name.to_uppercase().as_str() {
                    "PING" => string_cmds::parse_ping(args),
                    "GET" => string_cmds::parse_get(args),
                    "SET" => string_cmds::parse_set(args),
                    "DEL" => string_cmds::parse_del(args),
                    "SETNX" => string_cmds::parse_setnx(args),
                    "SETEX" => string_cmds::parse_setex(args),
                    "INCR" => string_cmds::parse_incr(args),
                    "DECR" => string_cmds::parse_decr(args),
                    "INCRBY" => string_cmds::parse_incrby(args),
                    "DECRBY" => string_cmds::parse_decrby(args),
                    "MGET" => string_cmds::parse_mget(args),
                    "MSET" => string_cmds::parse_mset(args),
                    "EXPIRE" => ttl_cmds::parse_expire(args),
                    "TTL" => ttl_cmds::parse_ttl(args),
                    "PERSIST" => ttl_cmds::parse_persist(args),
                    "KEYS" => ttl_cmds::parse_keys(args),
                    _ => Err(anyhow!("ERR unknown command '{}'", cmd_name)),
                }
            }
            _ => Err(anyhow!("ERR expected array")),
        }
    }

    /// Execute the command and return a RESP response
    pub async fn execute(&self, store: &Store) -> RespValue {
        match self {
            Command::Ping(None) => RespValue::SimpleString("PONG".to_string()),
            Command::Ping(Some(msg)) => RespValue::BulkString(Some(msg.as_bytes().to_vec())),

            Command::Get(key) => match store.get(key).await {
                Ok(Some(value)) => RespValue::BulkString(Some(value)),
                Ok(None) => RespValue::BulkString(None),
                Err(e) => RespValue::Error(e),
            },

            Command::Set(key, value) => {
                store.set(key.clone(), value.clone()).await;
                RespValue::SimpleString("OK".to_string())
            }

            Command::Del(keys) => {
                let deleted = store.del(keys).await;
                RespValue::Integer(deleted)
            }

            Command::SetNx(key, value) => {
                let was_set = store.set_nx(key.clone(), value.clone()).await;
                RespValue::Integer(if was_set { 1 } else { 0 })
            }

            Command::SetEx(key, seconds, value) => {
                store.set_ex(key.clone(), value.clone(), *seconds).await;
                RespValue::SimpleString("OK".to_string())
            }

            Command::Incr(key) => match store.incr(key).await {
                Ok(value) => RespValue::Integer(value),
                Err(e) => RespValue::Error(e),
            },

            Command::Decr(key) => match store.decr(key).await {
                Ok(value) => RespValue::Integer(value),
                Err(e) => RespValue::Error(e),
            },

            Command::IncrBy(key, delta) => match store.incr_by(key, *delta).await {
                Ok(value) => RespValue::Integer(value),
                Err(e) => RespValue::Error(e),
            },

            Command::DecrBy(key, delta) => match store.incr_by(key, -*delta).await {
                Ok(value) => RespValue::Integer(value),
                Err(e) => RespValue::Error(e),
            },

            Command::MGet(keys) => match store.mget(keys).await {
                Ok(values) => {
                    let resp_values: Vec<RespValue> =
                        values.into_iter().map(RespValue::BulkString).collect();
                    RespValue::Array(Some(resp_values))
                }
                Err(e) => RespValue::Error(e),
            },

            Command::MSet(pairs) => {
                store.mset(pairs.clone()).await;
                RespValue::SimpleString("OK".to_string())
            }

            Command::Expire(key, seconds) => {
                let result = store.expire(key, *seconds).await;
                RespValue::Integer(result)
            }

            Command::Ttl(key) => {
                let ttl = store.ttl(key).await;
                RespValue::Integer(ttl)
            }

            Command::Persist(key) => {
                let result = store.persist(key).await;
                RespValue::Integer(result)
            }

            Command::Keys(pattern) => {
                let keys = store.keys(pattern).await;
                let resp_values: Vec<RespValue> = keys
                    .into_iter()
                    .map(|k| RespValue::BulkString(Some(k.into_bytes())))
                    .collect();
                RespValue::Array(Some(resp_values))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cmd(args: &[&[u8]]) -> RespValue {
        RespValue::Array(Some(
            args.iter()
                .map(|a| RespValue::BulkString(Some(a.to_vec())))
                .collect(),
        ))
    }

    #[test]
    fn ping_without_args_returns_pong() {
        let resp = make_cmd(&[b"PING"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Ping(None));
    }

    #[test]
    fn ping_with_message_echoes_message() {
        let resp = make_cmd(&[b"PING", b"hello"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Ping(Some("hello".to_string())));
    }

    #[test]
    fn ping_is_case_insensitive() {
        for variant in &[b"ping".as_slice(), b"PING", b"Ping", b"PiNg"] {
            let resp = RespValue::Array(Some(vec![RespValue::BulkString(Some(variant.to_vec()))]));
            let cmd = Command::from_resp(resp).unwrap();
            assert_eq!(cmd, Command::Ping(None));
        }
    }

    #[test]
    fn ping_with_too_many_args_returns_error() {
        let resp = make_cmd(&[b"PING", b"arg1", b"arg2"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("wrong number of arguments"));
    }

    #[test]
    fn unknown_command_returns_error() {
        let resp = make_cmd(&[b"UNKNOWN"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown command"));
    }

    #[test]
    fn empty_array_returns_error() {
        let resp = RespValue::Array(Some(Vec::new()));
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn null_array_returns_error() {
        let resp = RespValue::Array(None);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn non_array_input_returns_error() {
        let resp = RespValue::SimpleString("PING".to_string());
        let result = Command::from_resp(resp);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected array"));
    }

    #[test]
    fn parse_get_command() {
        let resp = make_cmd(&[b"GET", b"mykey"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Get("mykey".to_string()));
    }

    #[test]
    fn parse_get_missing_key_returns_error() {
        let resp = make_cmd(&[b"GET"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn parse_set_command() {
        let resp = make_cmd(&[b"SET", b"mykey", b"myvalue"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Set("mykey".to_string(), b"myvalue".to_vec()));
    }

    #[test]
    fn parse_set_missing_value_returns_error() {
        let resp = make_cmd(&[b"SET", b"mykey"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn parse_del_single_key() {
        let resp = make_cmd(&[b"DEL", b"key1"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Del(vec!["key1".to_string()]));
    }

    #[test]
    fn parse_del_multiple_keys() {
        let resp = make_cmd(&[b"DEL", b"key1", b"key2", b"key3"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(
            cmd,
            Command::Del(vec![
                "key1".to_string(),
                "key2".to_string(),
                "key3".to_string()
            ])
        );
    }

    #[test]
    fn parse_del_no_keys_returns_error() {
        let resp = make_cmd(&[b"DEL"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn parse_setnx_command() {
        let resp = make_cmd(&[b"SETNX", b"mykey", b"myvalue"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(
            cmd,
            Command::SetNx("mykey".to_string(), b"myvalue".to_vec())
        );
    }

    #[test]
    fn parse_setex_command() {
        let resp = make_cmd(&[b"SETEX", b"mykey", b"60", b"myvalue"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(
            cmd,
            Command::SetEx("mykey".to_string(), 60, b"myvalue".to_vec())
        );
    }

    #[test]
    fn parse_setex_invalid_seconds_returns_error() {
        let resp = make_cmd(&[b"SETEX", b"mykey", b"-1", b"myvalue"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn parse_incr_command() {
        let resp = make_cmd(&[b"INCR", b"counter"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Incr("counter".to_string()));
    }

    #[test]
    fn parse_decr_command() {
        let resp = make_cmd(&[b"DECR", b"counter"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Decr("counter".to_string()));
    }

    #[test]
    fn parse_incrby_command() {
        let resp = make_cmd(&[b"INCRBY", b"counter", b"5"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::IncrBy("counter".to_string(), 5));
    }

    #[test]
    fn parse_decrby_command() {
        let resp = make_cmd(&[b"DECRBY", b"counter", b"5"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::DecrBy("counter".to_string(), 5));
    }

    #[test]
    fn parse_mget_command() {
        let resp = make_cmd(&[b"MGET", b"key1", b"key2", b"key3"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(
            cmd,
            Command::MGet(vec![
                "key1".to_string(),
                "key2".to_string(),
                "key3".to_string()
            ])
        );
    }

    #[test]
    fn parse_mget_no_keys_returns_error() {
        let resp = make_cmd(&[b"MGET"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mset_command() {
        let resp = make_cmd(&[b"MSET", b"key1", b"value1", b"key2", b"value2"]);
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(
            cmd,
            Command::MSet(vec![
                ("key1".to_string(), b"value1".to_vec()),
                ("key2".to_string(), b"value2".to_vec()),
            ])
        );
    }

    #[test]
    fn parse_mset_odd_args_returns_error() {
        let resp = make_cmd(&[b"MSET", b"key1", b"value1", b"key2"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mset_no_args_returns_error() {
        let resp = make_cmd(&[b"MSET"]);
        let result = Command::from_resp(resp);
        assert!(result.is_err());
    }

    // Async execution tests
    #[tokio::test]
    async fn execute_ping() {
        let store = Store::new();
        let cmd = Command::Ping(None);
        assert_eq!(
            cmd.execute(&store).await,
            RespValue::SimpleString("PONG".to_string())
        );
    }

    #[tokio::test]
    async fn execute_ping_with_message() {
        let store = Store::new();
        let cmd = Command::Ping(Some("hello".to_string()));
        assert_eq!(
            cmd.execute(&store).await,
            RespValue::BulkString(Some(b"hello".to_vec()))
        );
    }

    #[tokio::test]
    async fn execute_set_get() {
        let store = Store::new();

        let set_cmd = Command::Set("key".to_string(), b"value".to_vec());
        assert_eq!(
            set_cmd.execute(&store).await,
            RespValue::SimpleString("OK".to_string())
        );

        let get_cmd = Command::Get("key".to_string());
        assert_eq!(
            get_cmd.execute(&store).await,
            RespValue::BulkString(Some(b"value".to_vec()))
        );
    }

    #[tokio::test]
    async fn execute_get_nonexistent() {
        let store = Store::new();
        let cmd = Command::Get("nonexistent".to_string());
        assert_eq!(cmd.execute(&store).await, RespValue::BulkString(None));
    }

    #[tokio::test]
    async fn execute_del() {
        let store = Store::new();
        store.set("key1".to_string(), b"value1".to_vec()).await;
        store.set("key2".to_string(), b"value2".to_vec()).await;

        let cmd = Command::Del(vec!["key1".to_string(), "key3".to_string()]);
        assert_eq!(cmd.execute(&store).await, RespValue::Integer(1));
    }

    #[tokio::test]
    async fn execute_setnx() {
        let store = Store::new();

        let cmd = Command::SetNx("key".to_string(), b"value1".to_vec());
        assert_eq!(cmd.execute(&store).await, RespValue::Integer(1));

        let cmd = Command::SetNx("key".to_string(), b"value2".to_vec());
        assert_eq!(cmd.execute(&store).await, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn execute_incr_decr() {
        let store = Store::new();

        let cmd = Command::Incr("counter".to_string());
        assert_eq!(cmd.execute(&store).await, RespValue::Integer(1));

        let cmd = Command::IncrBy("counter".to_string(), 5);
        assert_eq!(cmd.execute(&store).await, RespValue::Integer(6));

        let cmd = Command::Decr("counter".to_string());
        assert_eq!(cmd.execute(&store).await, RespValue::Integer(5));

        let cmd = Command::DecrBy("counter".to_string(), 3);
        assert_eq!(cmd.execute(&store).await, RespValue::Integer(2));
    }

    #[tokio::test]
    async fn execute_mget_mset() {
        let store = Store::new();

        let cmd = Command::MSet(vec![
            ("key1".to_string(), b"value1".to_vec()),
            ("key2".to_string(), b"value2".to_vec()),
        ]);
        assert_eq!(
            cmd.execute(&store).await,
            RespValue::SimpleString("OK".to_string())
        );

        let cmd = Command::MGet(vec![
            "key1".to_string(),
            "key2".to_string(),
            "key3".to_string(),
        ]);
        assert_eq!(
            cmd.execute(&store).await,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"value1".to_vec())),
                RespValue::BulkString(Some(b"value2".to_vec())),
                RespValue::BulkString(None),
            ]))
        );
    }
}
