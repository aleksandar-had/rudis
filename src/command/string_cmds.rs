use anyhow::{anyhow, Result};

use super::parse::{extract_bulk_bytes, extract_bulk_string, extract_integer};
use super::Command;
use crate::resp::RespValue;

pub fn parse_ping(args: &[RespValue]) -> Result<Command> {
    match args.len() {
        0 => Ok(Command::Ping(None)),
        1 => {
            let message = extract_bulk_string(&args[0])?;
            Ok(Command::Ping(Some(message)))
        }
        _ => Err(anyhow!("ERR wrong number of arguments for 'ping' command")),
    }
}

pub fn parse_get(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'get' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::Get(key))
}

pub fn parse_set(args: &[RespValue]) -> Result<Command> {
    if args.len() != 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'set' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let value = extract_bulk_bytes(&args[1])?;
    Ok(Command::Set(key, value))
}

pub fn parse_del(args: &[RespValue]) -> Result<Command> {
    if args.is_empty() {
        return Err(anyhow!("ERR wrong number of arguments for 'del' command"));
    }
    let keys: Result<Vec<String>> = args.iter().map(extract_bulk_string).collect();
    Ok(Command::Del(keys?))
}

pub fn parse_setnx(args: &[RespValue]) -> Result<Command> {
    if args.len() != 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'setnx' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let value = extract_bulk_bytes(&args[1])?;
    Ok(Command::SetNx(key, value))
}

pub fn parse_setex(args: &[RespValue]) -> Result<Command> {
    if args.len() != 3 {
        return Err(anyhow!("ERR wrong number of arguments for 'setex' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let seconds = extract_integer(&args[1])?;
    if seconds <= 0 {
        return Err(anyhow!("ERR invalid expire time in 'setex' command"));
    }
    let value = extract_bulk_bytes(&args[2])?;
    Ok(Command::SetEx(key, seconds as u64, value))
}

pub fn parse_incr(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'incr' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::Incr(key))
}

pub fn parse_decr(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'decr' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::Decr(key))
}

pub fn parse_incrby(args: &[RespValue]) -> Result<Command> {
    if args.len() != 2 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'incrby' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    let delta = extract_integer(&args[1])?;
    Ok(Command::IncrBy(key, delta))
}

pub fn parse_decrby(args: &[RespValue]) -> Result<Command> {
    if args.len() != 2 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'decrby' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    let delta = extract_integer(&args[1])?;
    Ok(Command::DecrBy(key, delta))
}

pub fn parse_mget(args: &[RespValue]) -> Result<Command> {
    if args.is_empty() {
        return Err(anyhow!("ERR wrong number of arguments for 'mget' command"));
    }
    let keys: Result<Vec<String>> = args.iter().map(extract_bulk_string).collect();
    Ok(Command::MGet(keys?))
}

pub fn parse_mset(args: &[RespValue]) -> Result<Command> {
    if args.is_empty() || !args.len().is_multiple_of(2) {
        return Err(anyhow!("ERR wrong number of arguments for 'mset' command"));
    }
    let mut pairs = Vec::new();
    for chunk in args.chunks(2) {
        let key = extract_bulk_string(&chunk[0])?;
        let value = extract_bulk_bytes(&chunk[1])?;
        pairs.push((key, value));
    }
    Ok(Command::MSet(pairs))
}
