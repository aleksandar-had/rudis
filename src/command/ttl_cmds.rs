use anyhow::{anyhow, Result};

use super::parse::{extract_bulk_string, extract_integer};
use super::Command;
use crate::resp::RespValue;

pub fn parse_expire(args: &[RespValue]) -> Result<Command> {
    if args.len() != 2 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'expire' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    let seconds = extract_integer(&args[1])?;
    Ok(Command::Expire(key, seconds))
}

pub fn parse_ttl(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'ttl' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::Ttl(key))
}

pub fn parse_persist(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'persist' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::Persist(key))
}

pub fn parse_keys(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'keys' command"));
    }
    let pattern = extract_bulk_string(&args[0])?;
    Ok(Command::Keys(pattern))
}
