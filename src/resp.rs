use anyhow::{anyhow, Result};
use bytes::{Buf, BytesMut};

/// RESP (REdis Serialization Protocol) data types
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>), // None represents null bulk string
    Array(Option<Vec<RespValue>>), // None represents null array
}

impl RespValue {
    /// Serialize RESP value to bytes
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
            RespValue::Error(e) => format!("-{}\r\n", e).into_bytes(),
            RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
            RespValue::BulkString(None) => b"$-1\r\n".to_vec(),
            RespValue::BulkString(Some(bytes)) => {
                let mut result = format!("${}\r\n", bytes.len()).into_bytes();
                result.extend_from_slice(bytes);
                result.extend_from_slice(b"\r\n");
                result
            }
            RespValue::Array(None) => b"*-1\r\n".to_vec(),
            RespValue::Array(Some(values)) => {
                let mut result = format!("*{}\r\n", values.len()).into_bytes();
                for value in values {
                    result.extend_from_slice(&value.serialize());
                }
                result
            }
        }
    }

    /// Attempt to parse a RESP value from a buffer
    /// Returns Ok(Some(value, bytes_consumed)) if successful
    /// Returns Ok(None) if more data is needed
    /// Returns Err if the data is invalid
    pub fn parse(buffer: &mut BytesMut) -> Result<Option<(RespValue, usize)>> {
        if buffer.is_empty() {
            return Ok(None);
        }

        match buffer[0] {
            b'+' => parse_simple_string(buffer),
            b'-' => parse_error(buffer),
            b':' => parse_integer(buffer),
            b'$' => parse_bulk_string(buffer),
            b'*' => parse_array(buffer),
            _ => Err(anyhow!("Invalid RESP type byte: {}", buffer[0])),
        }
    }
}

fn find_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|w| w == b"\r\n")
}

fn parse_simple_string(buffer: &mut BytesMut) -> Result<Option<(RespValue, usize)>> {
    if let Some(pos) = find_crlf(&buffer[1..]) {
        let line = &buffer[1..pos + 1];
        let s = String::from_utf8(line.to_vec())?;
        let consumed = pos + 3; // +1 for type byte, +2 for \r\n
        Ok(Some((RespValue::SimpleString(s), consumed)))
    } else {
        Ok(None) // Need more data
    }
}

fn parse_error(buffer: &mut BytesMut) -> Result<Option<(RespValue, usize)>> {
    if let Some(pos) = find_crlf(&buffer[1..]) {
        let line = &buffer[1..pos + 1];
        let s = String::from_utf8(line.to_vec())?;
        let consumed = pos + 3;
        Ok(Some((RespValue::Error(s), consumed)))
    } else {
        Ok(None)
    }
}

fn parse_integer(buffer: &mut BytesMut) -> Result<Option<(RespValue, usize)>> {
    if let Some(pos) = find_crlf(&buffer[1..]) {
        let line = &buffer[1..pos + 1];
        let s = String::from_utf8(line.to_vec())?;
        let num = s.parse::<i64>()?;
        let consumed = pos + 3;
        Ok(Some((RespValue::Integer(num), consumed)))
    } else {
        Ok(None)
    }
}

fn parse_bulk_string(buffer: &mut BytesMut) -> Result<Option<(RespValue, usize)>> {
    // First, parse the length
    if let Some(pos) = find_crlf(&buffer[1..]) {
        let line = &buffer[1..pos + 1];
        let len_str = String::from_utf8(line.to_vec())?;
        let len = len_str.parse::<i64>()?;

        if len == -1 {
            // Null bulk string
            return Ok(Some((RespValue::BulkString(None), pos + 3)));
        }

        let len = len as usize;
        let total_needed = pos + 3 + len + 2; // type + length + \r\n + data + \r\n

        if buffer.len() < total_needed {
            return Ok(None); // Need more data
        }

        let data_start = pos + 3;
        let data = buffer[data_start..data_start + len].to_vec();
        Ok(Some((RespValue::BulkString(Some(data)), total_needed)))
    } else {
        Ok(None) // Need more data
    }
}

fn parse_array(buffer: &mut BytesMut) -> Result<Option<(RespValue, usize)>> {
    // First, parse the array length
    if let Some(pos) = find_crlf(&buffer[1..]) {
        let line = &buffer[1..pos + 1];
        let len_str = String::from_utf8(line.to_vec())?;
        let len = len_str.parse::<i64>()?;

        if len == -1 {
            // Null array
            return Ok(Some((RespValue::Array(None), pos + 3)));
        }

        let mut consumed = pos + 3;
        let mut elements = Vec::new();
        let mut temp_buffer = buffer.clone();
        temp_buffer.advance(consumed);

        for _ in 0..len {
            match RespValue::parse(&mut temp_buffer)? {
                Some((value, bytes)) => {
                    elements.push(value);
                    consumed += bytes;
                    temp_buffer.advance(bytes);
                }
                None => return Ok(None), // Need more data
            }
        }

        Ok(Some((RespValue::Array(Some(elements)), consumed)))
    } else {
        Ok(None) // Need more data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_string() {
        let mut buffer = BytesMut::from("+OK\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::SimpleString("OK".to_string()));
        assert_eq!(result.1, 5);
    }

    #[test]
    fn test_error() {
        let mut buffer = BytesMut::from("-Error message\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Error("Error message".to_string()));
    }

    #[test]
    fn test_integer() {
        let mut buffer = BytesMut::from(":1000\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Integer(1000));
    }

    #[test]
    fn test_bulk_string() {
        let mut buffer = BytesMut::from("$6\r\nfoobar\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::BulkString(Some(b"foobar".to_vec())));
    }

    #[test]
    fn test_null_bulk_string() {
        let mut buffer = BytesMut::from("$-1\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::BulkString(None));
    }

    #[test]
    fn test_array() {
        let mut buffer = BytesMut::from("*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(
            result.0,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"foo".to_vec())),
                RespValue::BulkString(Some(b"bar".to_vec())),
            ]))
        );
    }

    #[test]
    fn test_serialize_simple_string() {
        let value = RespValue::SimpleString("OK".to_string());
        assert_eq!(value.serialize(), b"+OK\r\n");
    }

    #[test]
    fn test_serialize_bulk_string() {
        let value = RespValue::BulkString(Some(b"foobar".to_vec()));
        assert_eq!(value.serialize(), b"$6\r\nfoobar\r\n");
    }
}
