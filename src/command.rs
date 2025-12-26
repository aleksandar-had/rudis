use crate::resp::RespValue;
use anyhow::{anyhow, Result};

/// Represents a Redis command
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Ping(Option<String>),
    // More commands will be added in future phases
}

impl Command {
    /// Parse a RESP array into a command
    pub fn from_resp(value: RespValue) -> Result<Self> {
        match value {
            RespValue::Array(Some(elements)) if !elements.is_empty() => {
                let cmd_name = extract_bulk_string(&elements[0])?;

                match cmd_name.to_uppercase().as_str() {
                    "PING" => {
                        if elements.len() == 1 {
                            Ok(Command::Ping(None))
                        } else if elements.len() == 2 {
                            let message = extract_bulk_string(&elements[1])?;
                            Ok(Command::Ping(Some(message)))
                        } else {
                            Err(anyhow!("ERR wrong number of arguments for 'ping' command"))
                        }
                    }
                    _ => Err(anyhow!("ERR unknown command '{}'", cmd_name)),
                }
            }
            _ => Err(anyhow!("ERR expected array")),
        }
    }

    /// Execute the command and return a RESP response
    pub fn execute(&self) -> RespValue {
        match self {
            Command::Ping(None) => RespValue::SimpleString("PONG".to_string()),
            Command::Ping(Some(msg)) => RespValue::BulkString(Some(msg.as_bytes().to_vec())),
        }
    }
}

// Helper function to extract a string from a bulk string RESP value
fn extract_bulk_string(value: &RespValue) -> Result<String> {
    match value {
        RespValue::BulkString(Some(bytes)) => {
            String::from_utf8(bytes.clone()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
        }
        RespValue::SimpleString(s) => Ok(s.clone()),
        _ => Err(anyhow!("Expected bulk string or simple string")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_without_args_returns_pong() {
        let resp = RespValue::Array(Some(vec![RespValue::BulkString(Some(b"PING".to_vec()))]));
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Ping(None));
        assert_eq!(cmd.execute(), RespValue::SimpleString("PONG".to_string()));
    }

    #[test]
    fn ping_with_message_echoes_message() {
        let resp = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"PING".to_vec())),
            RespValue::BulkString(Some(b"hello".to_vec())),
        ]));
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Ping(Some("hello".to_string())));
        assert_eq!(
            cmd.execute(),
            RespValue::BulkString(Some(b"hello".to_vec()))
        );
    }

    #[test]
    fn ping_is_case_insensitive() {
        let variations = vec![b"ping", b"PING", b"Ping", b"PiNg"];
        for variant in variations {
            let resp = RespValue::Array(Some(vec![RespValue::BulkString(Some(variant.to_vec()))]));
            let cmd = Command::from_resp(resp).unwrap();
            assert_eq!(cmd, Command::Ping(None));
        }
    }

    #[test]
    fn ping_with_too_many_args_returns_error() {
        let resp = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"PING".to_vec())),
            RespValue::BulkString(Some(b"arg1".to_vec())),
            RespValue::BulkString(Some(b"arg2".to_vec())),
        ]));
        let result = Command::from_resp(resp);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("wrong number of arguments"));
    }

    #[test]
    fn unknown_command_returns_error() {
        let resp = RespValue::Array(Some(vec![RespValue::BulkString(Some(b"UNKNOWN".to_vec()))]));
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
    fn ping_with_empty_message() {
        let resp = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"PING".to_vec())),
            RespValue::BulkString(Some(b"".to_vec())),
        ]));
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Ping(Some("".to_string())));
        assert_eq!(cmd.execute(), RespValue::BulkString(Some(Vec::new())));
    }

    #[test]
    fn command_from_simple_string_works() {
        let resp = RespValue::Array(Some(vec![RespValue::SimpleString("PING".to_string())]));
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Ping(None));
    }
}
