use crate::resp::RespValue;
use anyhow::{anyhow, Result};

/// Extract a UTF-8 string from a RESP value
pub fn extract_bulk_string(value: &RespValue) -> Result<String> {
    match value {
        RespValue::BulkString(Some(bytes)) => {
            String::from_utf8(bytes.clone()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
        }
        RespValue::SimpleString(s) => Ok(s.clone()),
        _ => Err(anyhow!("Expected bulk string or simple string")),
    }
}

/// Extract raw bytes from a RESP value
pub fn extract_bulk_bytes(value: &RespValue) -> Result<Vec<u8>> {
    match value {
        RespValue::BulkString(Some(bytes)) => Ok(bytes.clone()),
        RespValue::SimpleString(s) => Ok(s.as_bytes().to_vec()),
        _ => Err(anyhow!("Expected bulk string or simple string")),
    }
}

/// Extract an integer from a RESP value (supports integer type and string parsing)
pub fn extract_integer(value: &RespValue) -> Result<i64> {
    match value {
        RespValue::Integer(i) => Ok(*i),
        RespValue::BulkString(Some(bytes)) => {
            let s = String::from_utf8(bytes.clone())?;
            s.parse::<i64>()
                .map_err(|_| anyhow!("ERR value is not an integer or out of range"))
        }
        RespValue::SimpleString(s) => s
            .parse::<i64>()
            .map_err(|_| anyhow!("ERR value is not an integer or out of range")),
        _ => Err(anyhow!("ERR value is not an integer or out of range")),
    }
}
