# Rudis - A Redis Clone in Rust

A from-scratch implementation of Redis in Rust, built for learning and tinkering.

## Current Status: Phase 1 - TCP Server & RESP Parser

### Implemented Features
- ✅ TCP server listening on port 6379
- ✅ RESP protocol parser (all 5 data types)
- ✅ PING/PONG command

## Quick Start

### Build and Run
```bash
cargo run
```

The server will start on `127.0.0.1:6379`.

### Testing with redis-cli

In another terminal:
```bash
redis-cli -p 6379 PING
# Should return: PONG

redis-cli -p 6379 PING "Hello, Rudis!"
# Should return: "Hello, Rudis!"
```

### Run Tests
```bash
cargo test
```

## Architecture

### Project Structure
```
src/
├── main.rs      # Entry point
├── server.rs    # TCP server and connection handling
├── resp.rs      # RESP protocol parser/serializer
└── command.rs   # Command parsing and execution
```

### RESP Protocol Support
- ✅ Simple Strings: `+OK\r\n`
- ✅ Errors: `-Error message\r\n`
- ✅ Integers: `:1000\r\n`
- ✅ Bulk Strings: `$6\r\nfoobar\r\n`
- ✅ Arrays: `*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n`

## Roadmap

- [x] Phase 1: TCP Server & RESP Parser
- [ ] Phase 2: Core Data Structures (GET/SET)
- [ ] Phase 3: Advanced Commands (EXPIRE, TTL, DEL)
- [ ] Phase 4: Persistence (RDB, AOF)
- [ ] Phase 5: Replication & Clustering
