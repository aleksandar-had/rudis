use anyhow::{Result, anyhow};

use super::Command;
use super::parse::{extract_bulk_bytes, extract_bulk_string};
use crate::resp::RespValue;

pub fn parse_hset(args: &[RespValue]) -> Result<Command> {
    // args = [key, field1, value1, ...] — total must be odd (key + even field-value pairs)
    if args.len() < 3 || args.len().is_multiple_of(2) {
        return Err(anyhow!("ERR wrong number of arguments for 'hset' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let mut pairs = Vec::new();
    for chunk in args[1..].chunks(2) {
        let field = extract_bulk_bytes(&chunk[0])?;
        let value = extract_bulk_bytes(&chunk[1])?;
        pairs.push((field, value));
    }
    Ok(Command::HSet(key, pairs))
}

pub fn parse_hget(args: &[RespValue]) -> Result<Command> {
    if args.len() != 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'hget' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let field = extract_bulk_bytes(&args[1])?;
    Ok(Command::HGet(key, field))
}

pub fn parse_hdel(args: &[RespValue]) -> Result<Command> {
    if args.len() < 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'hdel' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let fields: Result<Vec<Vec<u8>>> = args[1..].iter().map(extract_bulk_bytes).collect();
    Ok(Command::HDel(key, fields?))
}

pub fn parse_hgetall(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'hgetall' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::HGetAll(key))
}

pub fn parse_hlen(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'hlen' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::HLen(key))
}
