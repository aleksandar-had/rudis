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

## Integration Tests with redis-cli

### Automated Integration Tests

Run the full integration test suite (starts server automatically):
```bash
./run_integration_tests.sh
```

### Manual Testing

1. **Start the server:**
   ```bash
   cargo run
   ```

2. **In another terminal, test with redis-cli:**
   ```bash
   # Basic PING
   redis-cli -p 6379 PING
   # Expected: PONG

   # PING with message
   redis-cli -p 6379 PING "hello world"
   # Expected: "hello world"

   # Case insensitive
   redis-cli -p 6379 ping
   # Expected: PONG

   # Unknown command (should error)
   redis-cli -p 6379 GET somekey
   # Expected: (error) ERR unknown command 'GET'

   # Interactive mode
   redis-cli -p 6379
   127.0.0.1:6379> PING
   PONG
   127.0.0.1:6379> PING test
   "test"
   127.0.0.1:6379> exit
   ```

## Test Coverage

### Phase 1 Coverage

- ✅ RESP protocol parsing (all 5 types)
- ✅ RESP serialization
- ✅ PING command (with/without message)
- ✅ Error handling (invalid commands, wrong arg counts)
- ✅ Case insensitivity
- ✅ Binary data support
- ✅ Incomplete message buffering
- ✅ redis-cli compatibility

Total: 41 unit tests + 7 integration tests
