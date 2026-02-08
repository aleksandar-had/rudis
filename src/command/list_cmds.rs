use anyhow::{Result, anyhow};

use super::Command;
use super::parse::{extract_bulk_bytes, extract_bulk_string, extract_integer};
use crate::resp::RespValue;

pub fn parse_lpush(args: &[RespValue]) -> Result<Command> {
    if args.len() < 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'lpush' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let elements: Result<Vec<Vec<u8>>> = args[1..].iter().map(extract_bulk_bytes).collect();
    Ok(Command::LPush(key, elements?))
}

pub fn parse_rpush(args: &[RespValue]) -> Result<Command> {
    if args.len() < 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'rpush' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let elements: Result<Vec<Vec<u8>>> = args[1..].iter().map(extract_bulk_bytes).collect();
    Ok(Command::RPush(key, elements?))
}

pub fn parse_lpop(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'lpop' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::LPop(key))
}

pub fn parse_rpop(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'rpop' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::RPop(key))
}

pub fn parse_lrange(args: &[RespValue]) -> Result<Command> {
    if args.len() != 3 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'lrange' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    let start = extract_integer(&args[1])?;
    let stop = extract_integer(&args[2])?;
    Ok(Command::LRange(key, start, stop))
}

pub fn parse_llen(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'llen' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::LLen(key))
}
