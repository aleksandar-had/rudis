use anyhow::Result;
use bytes::{Buf, BytesMut};

/// RESP (REdis Serialization Protocol) data types
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),   // None represents null bulk string
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
    ///
    /// This also handles inline commands (plain text commands like "PING\r\n")
    /// which are converted to RESP arrays for uniform command processing.
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
            // Any other byte indicates an inline command
            _ => parse_inline_command(buffer),
        }
    }
}

fn find_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|w| w == b"\r\n")
}

/// Parse an inline command (plain text like "PING\r\n" or "SET foo bar\r\n")
/// Converts it to a RESP array for uniform command processing
fn parse_inline_command(buffer: &mut BytesMut) -> Result<Option<(RespValue, usize)>> {
    if let Some(pos) = find_crlf(buffer) {
        let line = &buffer[..pos];

        // Split the line by whitespace to get command and arguments
        let parts: Vec<&[u8]> = line
            .split(|&b| b == b' ' || b == b'\t')
            .filter(|part| !part.is_empty())
            .collect();

        if parts.is_empty() {
            // Empty command, consume the line and return empty array
            return Ok(Some((RespValue::Array(Some(Vec::new())), pos + 2)));
        }

        // Convert each part to a bulk string
        let elements: Vec<RespValue> = parts
            .into_iter()
            .map(|part| RespValue::BulkString(Some(part.to_vec())))
            .collect();

        let consumed = pos + 2; // line + \r\n
        Ok(Some((RespValue::Array(Some(elements)), consumed)))
    } else {
        Ok(None) // Need more data
    }
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

    // Parsing tests
    #[test]
    fn parse_simple_string() {
        let mut buffer = BytesMut::from("+OK\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::SimpleString("OK".to_string()));
        assert_eq!(result.1, 5);
    }

    #[test]
    fn parse_empty_simple_string() {
        let mut buffer = BytesMut::from("+\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::SimpleString("".to_string()));
    }

    #[test]
    fn parse_error() {
        let mut buffer = BytesMut::from("-Error message\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Error("Error message".to_string()));
    }

    #[test]
    fn parse_integer_positive() {
        let mut buffer = BytesMut::from(":1000\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Integer(1000));
    }

    #[test]
    fn parse_integer_negative() {
        let mut buffer = BytesMut::from(":-42\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Integer(-42));
    }

    #[test]
    fn parse_integer_zero() {
        let mut buffer = BytesMut::from(":0\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Integer(0));
    }

    #[test]
    fn parse_bulk_string() {
        let mut buffer = BytesMut::from("$6\r\nfoobar\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::BulkString(Some(b"foobar".to_vec())));
    }

    #[test]
    fn parse_empty_bulk_string() {
        let mut buffer = BytesMut::from("$0\r\n\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::BulkString(Some(Vec::new())));
    }

    #[test]
    fn parse_null_bulk_string() {
        let mut buffer = BytesMut::from("$-1\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::BulkString(None));
    }

    #[test]
    fn parse_bulk_string_with_binary_data() {
        let mut buffer = BytesMut::from("$5\r\n\0\r\n\x01\x02\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(
            result.0,
            RespValue::BulkString(Some(vec![0, b'\r', b'\n', 1, 2]))
        );
    }

    #[test]
    fn parse_array() {
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
    fn parse_empty_array() {
        let mut buffer = BytesMut::from("*0\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Array(Some(Vec::new())));
    }

    #[test]
    fn parse_null_array() {
        let mut buffer = BytesMut::from("*-1\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Array(None));
    }

    #[test]
    fn parse_nested_array() {
        let mut buffer = BytesMut::from("*2\r\n*2\r\n:1\r\n:2\r\n*1\r\n+OK\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(
            result.0,
            RespValue::Array(Some(vec![
                RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(2),])),
                RespValue::Array(Some(vec![RespValue::SimpleString("OK".to_string()),])),
            ]))
        );
    }

    #[test]
    fn parse_mixed_type_array() {
        let mut buffer = BytesMut::from("*3\r\n:1\r\n+OK\r\n$3\r\nfoo\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(
            result.0,
            RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::SimpleString("OK".to_string()),
                RespValue::BulkString(Some(b"foo".to_vec())),
            ]))
        );
    }

    #[test]
    fn parse_incomplete_simple_string_returns_none() {
        let mut buffer = BytesMut::from("+OK");
        let result = RespValue::parse(&mut buffer).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_incomplete_bulk_string_returns_none() {
        let mut buffer = BytesMut::from("$6\r\nfoo");
        let result = RespValue::parse(&mut buffer).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_incomplete_array_returns_none() {
        let mut buffer = BytesMut::from("*2\r\n$3\r\nfoo\r\n");
        let result = RespValue::parse(&mut buffer).unwrap();
        assert!(result.is_none());
    }

    // Inline command parsing tests
    #[test]
    fn parse_inline_ping() {
        let mut buffer = BytesMut::from("PING\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(
            result.0,
            RespValue::Array(Some(vec![RespValue::BulkString(Some(b"PING".to_vec())),]))
        );
        assert_eq!(result.1, 6);
    }

    #[test]
    fn parse_inline_set_command() {
        let mut buffer = BytesMut::from("SET foo bar\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(
            result.0,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"SET".to_vec())),
                RespValue::BulkString(Some(b"foo".to_vec())),
                RespValue::BulkString(Some(b"bar".to_vec())),
            ]))
        );
    }

    #[test]
    fn parse_inline_with_extra_spaces() {
        let mut buffer = BytesMut::from("SET  foo   bar\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(
            result.0,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"SET".to_vec())),
                RespValue::BulkString(Some(b"foo".to_vec())),
                RespValue::BulkString(Some(b"bar".to_vec())),
            ]))
        );
    }

    #[test]
    fn parse_inline_empty_line() {
        let mut buffer = BytesMut::from("\r\n");
        let result = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(result.0, RespValue::Array(Some(Vec::new())));
    }

    #[test]
    fn parse_inline_incomplete_returns_none() {
        let mut buffer = BytesMut::from("PING");
        let result = RespValue::parse(&mut buffer).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_invalid_integer_returns_error() {
        let mut buffer = BytesMut::from(":notanumber\r\n");
        let result = RespValue::parse(&mut buffer);
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_buffer_returns_none() {
        let mut buffer = BytesMut::new();
        let result = RespValue::parse(&mut buffer).unwrap();
        assert!(result.is_none());
    }

    // Serialization tests
    #[test]
    fn serialize_simple_string() {
        let value = RespValue::SimpleString("OK".to_string());
        assert_eq!(value.serialize(), b"+OK\r\n");
    }

    #[test]
    fn serialize_error() {
        let value = RespValue::Error("ERR unknown command".to_string());
        assert_eq!(value.serialize(), b"-ERR unknown command\r\n");
    }

    #[test]
    fn serialize_integer() {
        let value = RespValue::Integer(1000);
        assert_eq!(value.serialize(), b":1000\r\n");
    }

    #[test]
    fn serialize_bulk_string() {
        let value = RespValue::BulkString(Some(b"foobar".to_vec()));
        assert_eq!(value.serialize(), b"$6\r\nfoobar\r\n");
    }

    #[test]
    fn serialize_null_bulk_string() {
        let value = RespValue::BulkString(None);
        assert_eq!(value.serialize(), b"$-1\r\n");
    }

    #[test]
    fn serialize_array() {
        let value = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"foo".to_vec())),
            RespValue::BulkString(Some(b"bar".to_vec())),
        ]));
        assert_eq!(value.serialize(), b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");
    }

    #[test]
    fn serialize_null_array() {
        let value = RespValue::Array(None);
        assert_eq!(value.serialize(), b"*-1\r\n");
    }

    // Round-trip tests
    #[test]
    fn roundtrip_simple_string() {
        let original = RespValue::SimpleString("PONG".to_string());
        let serialized = original.serialize();
        let mut buffer = BytesMut::from(&serialized[..]);
        let (parsed, _) = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn roundtrip_bulk_string() {
        let original = RespValue::BulkString(Some(b"hello world".to_vec()));
        let serialized = original.serialize();
        let mut buffer = BytesMut::from(&serialized[..]);
        let (parsed, _) = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn roundtrip_array() {
        let original = RespValue::Array(Some(vec![
            RespValue::Integer(42),
            RespValue::SimpleString("OK".to_string()),
            RespValue::BulkString(Some(b"test".to_vec())),
        ]));
        let serialized = original.serialize();
        let mut buffer = BytesMut::from(&serialized[..]);
        let (parsed, _) = RespValue::parse(&mut buffer).unwrap().unwrap();
        assert_eq!(original, parsed);
    }
}
