# Rudis - A Redis Clone in Rust

A from-scratch implementation of Redis in Rust, built for learning and tinkering.

## Current Status: Phase 4 - Data Structures

### Implemented Features
- TCP server listening on port 6379
- RESP protocol parser (all 5 data types + inline commands)
- Thread-safe data store with key expiration
- Multi-type store: Strings, Lists, Sets, Hashes
- WRONGTYPE errors for cross-type operations
- Passive expiration (lazy deletion on access)
- Active expiration (background task sampling expired keys)
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
| `EXPIRE key seconds` | Set key expiration (negative deletes) |
| `TTL key` | Get time-to-live (-2 no key, -1 no expiry) |
| `PERSIST key` | Remove expiration from key |
| `KEYS pattern` | Find keys matching glob pattern (* ?) |
| **Lists** | |
| `LPUSH key elem [elem ...]` | Push elements to list head |
| `RPUSH key elem [elem ...]` | Push elements to list tail |
| `LPOP key` | Remove and return head element |
| `RPOP key` | Remove and return tail element |
| `LRANGE key start stop` | Get range of elements (negative indices supported) |
| `LLEN key` | Get list length |
| **Sets** | |
| `SADD key member [member ...]` | Add members to set |
| `SREM key member [member ...]` | Remove members from set |
| `SMEMBERS key` | Get all set members |
| `SISMEMBER key member` | Check set membership (0 or 1) |
| `SCARD key` | Get set cardinality |
| **Hashes** | |
| `HSET key field value [field value ...]` | Set hash fields |
| `HGET key field` | Get hash field value |
| `HDEL key field [field ...]` | Delete hash fields |
| `HGETALL key` | Get all hash field-value pairs |
| `HLEN key` | Get number of hash fields |

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

# TTL management
redis-cli SET mykey "value"
redis-cli EXPIRE mykey 300
redis-cli TTL mykey
# 300
redis-cli PERSIST mykey
redis-cli TTL mykey
# -1

# Find keys by pattern
redis-cli KEYS "user:*"
redis-cli KEYS "key?"

# Lists
redis-cli LPUSH mylist "c" "b" "a"
redis-cli LRANGE mylist 0 -1
# 1) "a"  2) "b"  3) "c"
redis-cli LPOP mylist
# "a"

# Sets
redis-cli SADD myset "x" "y" "z"
redis-cli SISMEMBER myset "x"
# 1
redis-cli SMEMBERS myset

# Hashes
redis-cli HSET user:1 name "alice" age "30"
redis-cli HGET user:1 name
# "alice"
redis-cli HGETALL user:1
```

### Run Tests
```bash
# Unit tests
cargo test

# Integration tests (with server)
./run_integration_tests.sh
```

### Benchmarking
```bash
# Phase 2: Compare basic commands (PING, SET, GET)
./compare_benchmark.sh

# Phase 3: Benchmark TTL commands (EXPIRE, TTL, PERSIST)
./compare_benchmark.sh --phase3-ttl

# Phase 3: Benchmark KEYS scaling (1K and 10K keys)
./compare_benchmark.sh --phase3-keys

# Phase 3: Run all Phase 3 benchmarks
./compare_benchmark.sh --phase3-all

# Custom benchmark args
./compare_benchmark.sh -t ping,set,get -n 50000 -c 100 --threads 4 -q
```

Results are appended to `benchmark_results.md`.

## Architecture

### Project Structure
```
src/
├── main.rs          # Entry point
├── server.rs        # TCP server and connection handling
├── resp.rs          # RESP protocol parser/serializer
├── command/         # Command parsing and execution
│   ├── mod.rs       # Command enum, dispatch
│   ├── parse.rs     # Shared parsing helpers
│   ├── string_cmds.rs, ttl_cmds.rs, list_cmds.rs, set_cmds.rs, hash_cmds.rs
└── store/           # Thread-safe data store
    ├── mod.rs       # Store struct, active expiration
    ├── value.rs     # DataType enum, StoredValue
    ├── string_ops.rs, ttl_ops.rs, list_ops.rs, set_ops.rs, hash_ops.rs
```

### RESP Protocol Support
- Simple Strings: `+OK\r\n`
- Errors: `-Error message\r\n`
- Integers: `:1000\r\n`
- Bulk Strings: `$6\r\nfoobar\r\n`
- Arrays: `*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n`

### Data Store
- Thread-safe using `Arc<RwLock<HashMap>>`
- Multi-type: `DataType` enum supports String, List (VecDeque), Set (HashSet), Hash (HashMap)
- WRONGTYPE errors when commands target the wrong data type
- Passive expiration (lazy deletion on key access)
- Active expiration (background task samples 20 keys every 100ms)
- Auto-deletion of empty collections (lists, sets, hashes)
- Supports binary data as values

## Roadmap

- [x] Phase 1: TCP Server & RESP Parser
- [x] Phase 2: Core Commands (GET, SET, DEL, INCR, etc.)
- [x] Phase 3: TTL Commands (EXPIRE, TTL, PERSIST, KEYS) & Active Expiration
  - KEYS supports basic glob (* and ?) - full glob ([abc], [^abc], [a-z]) planned for later
- [x] Phase 4: Data Structures (Lists, Sets, Hashes)
- [ ] Phase 4.5: Sorted Sets (ZADD, ZRANGE, ZRANK, ZSCORE, ZCARD)
- [ ] Phase 5: Persistence (RDB, AOF)
- [ ] Phase 6: Replication & Clustering
