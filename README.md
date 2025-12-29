# Rudis - A Redis Clone in Rust

A from-scratch implementation of Redis in Rust, built for learning and tinkering.

## Current Status: Phase 2 - Core Commands

### Implemented Features
- TCP server listening on port 6379
- RESP protocol parser (all 5 data types)
- Thread-safe data store with key expiration
- Full redis-cli compatibility

### Supported Commands

| Command | Description |
|---------|-------------|
| `PING [message]` | Test connectivity, optionally echo message |
| `GET key` | Get the value of a key |
| `SET key value` | Set a key to a value |
| `DEL key [key ...]` | Delete one or more keys |
| `SETNX key value` | Set key only if it doesn't exist |
| `SETEX key seconds value` | Set key with expiration time |
| `INCR key` | Increment value by 1 |
| `DECR key` | Decrement value by 1 |
| `INCRBY key delta` | Increment value by delta |
| `DECRBY key delta` | Decrement value by delta |
| `MGET key [key ...]` | Get multiple keys at once |
| `MSET key value [key value ...]` | Set multiple keys at once |

## Quick Start

### Build and Run
```bash
cargo run
```

The server will start on `127.0.0.1:6379`.

### Testing with redis-cli

In another terminal:
```bash
# Basic connectivity
redis-cli PING
# PONG

# Key-value operations
redis-cli SET mykey "Hello, Rudis!"
# OK
redis-cli GET mykey
# "Hello, Rudis!"

# Atomic counters
redis-cli SET counter 10
redis-cli INCR counter
# 11
redis-cli INCRBY counter 5
# 16

# Batch operations
redis-cli MSET a 1 b 2 c 3
redis-cli MGET a b c
# 1) "1"
# 2) "2"
# 3) "3"

# Key with expiration
redis-cli SETEX tempkey 60 "expires in 60 seconds"
```

### Run Tests
```bash
# Unit tests
cargo test

# Integration tests (with server)
./run_integration_tests.sh
```

## Architecture

### Project Structure
```
src/
├── main.rs      # Entry point
├── server.rs    # TCP server and connection handling
├── resp.rs      # RESP protocol parser/serializer
├── command.rs   # Command parsing and execution
└── store.rs     # Thread-safe key-value store with expiration
```

### RESP Protocol Support
- Simple Strings: `+OK\r\n`
- Errors: `-Error message\r\n`
- Integers: `:1000\r\n`
- Bulk Strings: `$6\r\nfoobar\r\n`
- Arrays: `*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n`

### Data Store
- Thread-safe using `Arc<RwLock<HashMap>>`
- Lazy expiration on key access
- Supports binary data as values

## Roadmap

- [x] Phase 1: TCP Server & RESP Parser
- [x] Phase 2: Core Commands (GET, SET, DEL, INCR, etc.)
- [ ] Phase 3: Advanced Commands (EXPIRE, TTL, KEYS, EXISTS)
- [ ] Phase 4: Persistence (RDB, AOF)
- [ ] Phase 5: Replication & Clustering
