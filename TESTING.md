# Testing Guide

## Unit Tests

Run all unit tests:
```bash
cargo test
```

Run tests with output:
```bash
cargo test -- --nocapture
```

Run a specific test:
```bash
cargo test test_name
```

## Integration Tests with redis-cli

### Automated Integration Tests

Run the full integration test suite (starts server automatically):
```bash
./run_integration_tests.sh
```

This script:
1. Builds the project
2. Starts the rudis server in the background
3. Runs all integration tests against it
4. Shuts down the server on completion

### Manual Testing

1. **Start the server:**
   ```bash
   cargo run
   ```

2. **In another terminal, test with redis-cli:**

   ```bash
   # Basic PING
   redis-cli PING
   # PONG

   # PING with message
   redis-cli PING "hello world"
   # "hello world"

   # SET and GET
   redis-cli SET foo bar
   # OK
   redis-cli GET foo
   # "bar"

   # GET nonexistent key
   redis-cli GET nonexistent
   # (nil)

   # DELETE
   redis-cli DEL foo
   # (integer) 1

   # SETNX (set if not exists)
   redis-cli SETNX mykey "first"
   # (integer) 1
   redis-cli SETNX mykey "second"
   # (integer) 0
   redis-cli GET mykey
   # "first"

   # SETEX (set with expiry)
   redis-cli SETEX tempkey 10 "temporary"
   # OK

   # INCR/DECR
   redis-cli SET counter 10
   redis-cli INCR counter
   # (integer) 11
   redis-cli INCRBY counter 5
   # (integer) 16
   redis-cli DECR counter
   # (integer) 15
   redis-cli DECRBY counter 3
   # (integer) 12

   # MSET/MGET (batch operations)
   redis-cli MSET key1 val1 key2 val2 key3 val3
   # OK
   redis-cli MGET key1 key2 key3
   # 1) "val1"
   # 2) "val2"
   # 3) "val3"

   # Unknown command (should error)
   redis-cli UNKNOWN
   # (error) ERR unknown command 'UNKNOWN'

   # Interactive mode
   redis-cli
   127.0.0.1:6379> PING
   PONG
   127.0.0.1:6379> SET greeting "Hello, Rudis!"
   OK
   127.0.0.1:6379> GET greeting
   "Hello, Rudis!"
   127.0.0.1:6379> exit
   ```

## Test Coverage

### Phase 1 Coverage
- RESP protocol parsing (all 5 types)
- RESP serialization
- PING command (with/without message)
- Error handling (invalid commands, wrong arg counts)
- Case insensitivity
- Binary data support
- Incomplete message buffering

### Phase 2 Coverage
- Thread-safe store operations
- GET/SET commands
- DEL command (single and multiple keys)
- SETNX (set if not exists)
- SETEX (set with expiration)
- Key expiration (lazy deletion)
- INCR/DECR/INCRBY/DECRBY (atomic counters)
- INCR on nonexistent key (starts at 0)
- INCR on non-integer value (error handling)
- MGET/MSET (batch operations)
- redis-cli compatibility for all commands
