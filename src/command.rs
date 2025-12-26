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
    fn test_ping_no_args() {
        let resp = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"PING".to_vec())),
        ]));
        let cmd = Command::from_resp(resp).unwrap();
        assert_eq!(cmd, Command::Ping(None));
        assert_eq!(cmd.execute(), RespValue::SimpleString("PONG".to_string()));
    }

    #[test]
    fn test_ping_with_message() {
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
}
