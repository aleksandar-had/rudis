use anyhow::{Result, anyhow};

use super::Command;
use super::parse::{extract_bulk_bytes, extract_bulk_string};
use crate::resp::RespValue;

pub fn parse_sadd(args: &[RespValue]) -> Result<Command> {
    if args.len() < 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'sadd' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let members: Result<Vec<Vec<u8>>> = args[1..].iter().map(extract_bulk_bytes).collect();
    Ok(Command::SAdd(key, members?))
}

pub fn parse_srem(args: &[RespValue]) -> Result<Command> {
    if args.len() < 2 {
        return Err(anyhow!("ERR wrong number of arguments for 'srem' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    let members: Result<Vec<Vec<u8>>> = args[1..].iter().map(extract_bulk_bytes).collect();
    Ok(Command::SRem(key, members?))
}

pub fn parse_smembers(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'smembers' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::SMembers(key))
}

pub fn parse_sismember(args: &[RespValue]) -> Result<Command> {
    if args.len() != 2 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'sismember' command"
        ));
    }
    let key = extract_bulk_string(&args[0])?;
    let member = extract_bulk_bytes(&args[1])?;
    Ok(Command::SIsMember(key, member))
}

pub fn parse_scard(args: &[RespValue]) -> Result<Command> {
    if args.len() != 1 {
        return Err(anyhow!("ERR wrong number of arguments for 'scard' command"));
    }
    let key = extract_bulk_string(&args[0])?;
    Ok(Command::SCard(key))
}
