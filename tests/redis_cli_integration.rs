use std::process::{Command, Stdio};

/// Helper to check if redis-cli is available
fn redis_cli_available() -> bool {
    Command::new("redis-cli")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Helper to run redis-cli command against our server
fn run_redis_cli(args: &[&str]) -> Result<String, String> {
    let output = Command::new("redis-cli")
        .args(["-p", "6379"])
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute redis-cli: {}", e))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Invalid UTF-8 output: {}", e))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Check if server is running by trying to ping it
fn server_is_running() -> bool {
    run_redis_cli(&["PING"]).is_ok()
}

fn skip_if_unavailable() -> bool {
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return true;
    }
    if !server_is_running() {
        eprintln!("Server not running. Run with: ./run_integration_tests.sh");
        return true;
    }
    false
}

#[test]
fn test_redis_cli_ping() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["PING"]);
    assert!(result.is_ok(), "PING command failed: {:?}", result);
    assert_eq!(result.unwrap(), "PONG");
}

#[test]
fn test_redis_cli_ping_with_message() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["PING", "hello"]);
    assert!(result.is_ok(), "PING with message failed: {:?}", result);
    assert_eq!(result.unwrap(), "hello");
}

#[test]
fn test_redis_cli_ping_case_insensitive() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["ping"]);
    assert!(result.is_ok(), "ping (lowercase) failed: {:?}", result);
    assert_eq!(result.unwrap(), "PONG");
}

#[test]
fn test_redis_cli_ping_with_spaces() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["PING", "hello world"]);
    assert!(result.is_ok(), "PING with spaces failed: {:?}", result);
    assert_eq!(result.unwrap(), "hello world");
}

#[test]
fn test_redis_cli_unknown_command() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["NOTACOMMAND"]);
    if let Ok(output) = result {
        assert!(
            output.contains("ERR") || output.contains("unknown"),
            "Expected error for unknown command, got: {}",
            output
        );
    }
}

#[test]
fn test_redis_cli_multiple_pings() {
    if skip_if_unavailable() {
        return;
    }

    for i in 0..5 {
        let msg = format!("message{}", i);
        let result = run_redis_cli(&["PING", &msg]);
        assert!(result.is_ok(), "PING #{} failed: {:?}", i, result);
        assert_eq!(result.unwrap(), msg);
    }
}

#[test]
fn test_redis_cli_ping_empty_string() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["PING", ""]);
    assert!(
        result.is_ok(),
        "PING with empty string failed: {:?}",
        result
    );
    assert_eq!(result.unwrap(), "");
}

// Phase 2 integration tests

#[test]
fn test_redis_cli_set_get() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["SET", "testkey", "testvalue"]);
    assert!(result.is_ok(), "SET failed: {:?}", result);
    assert_eq!(result.unwrap(), "OK");

    let result = run_redis_cli(&["GET", "testkey"]);
    assert!(result.is_ok(), "GET failed: {:?}", result);
    assert_eq!(result.unwrap(), "testvalue");
}

#[test]
fn test_redis_cli_get_nonexistent() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["GET", "nonexistent_key_12345"]);
    assert!(result.is_ok(), "GET nonexistent failed: {:?}", result);
    let output = result.unwrap();
    assert!(output.is_empty() || output == "(nil)");
}

#[test]
fn test_redis_cli_del() {
    if skip_if_unavailable() {
        return;
    }

    // Set a key first
    run_redis_cli(&["SET", "delkey", "value"]).unwrap();

    let result = run_redis_cli(&["DEL", "delkey"]);
    assert!(result.is_ok(), "DEL failed: {:?}", result);
    assert_eq!(result.unwrap(), "1");

    // Verify it's gone
    let result = run_redis_cli(&["GET", "delkey"]);
    assert!(result.is_ok());
}

#[test]
fn test_redis_cli_setnx() {
    if skip_if_unavailable() {
        return;
    }

    // Clean up first
    let _ = run_redis_cli(&["DEL", "setnxkey"]);

    // First SETNX should succeed
    let result = run_redis_cli(&["SETNX", "setnxkey", "first"]);
    assert!(result.is_ok(), "SETNX failed: {:?}", result);
    assert_eq!(result.unwrap(), "1");

    // Second SETNX should fail
    let result = run_redis_cli(&["SETNX", "setnxkey", "second"]);
    assert!(result.is_ok(), "SETNX failed: {:?}", result);
    assert_eq!(result.unwrap(), "0");

    // Value should be "first"
    let result = run_redis_cli(&["GET", "setnxkey"]);
    assert_eq!(result.unwrap(), "first");
}

#[test]
fn test_redis_cli_incr_decr() {
    if skip_if_unavailable() {
        return;
    }

    // Clean up and set initial value
    let _ = run_redis_cli(&["DEL", "counter"]);
    run_redis_cli(&["SET", "counter", "10"]).unwrap();

    let result = run_redis_cli(&["INCR", "counter"]);
    assert!(result.is_ok(), "INCR failed: {:?}", result);
    assert_eq!(result.unwrap(), "11");

    let result = run_redis_cli(&["INCRBY", "counter", "5"]);
    assert!(result.is_ok(), "INCRBY failed: {:?}", result);
    assert_eq!(result.unwrap(), "16");

    let result = run_redis_cli(&["DECR", "counter"]);
    assert!(result.is_ok(), "DECR failed: {:?}", result);
    assert_eq!(result.unwrap(), "15");

    let result = run_redis_cli(&["DECRBY", "counter", "3"]);
    assert!(result.is_ok(), "DECRBY failed: {:?}", result);
    assert_eq!(result.unwrap(), "12");
}

#[test]
fn test_redis_cli_incr_new_key() {
    if skip_if_unavailable() {
        return;
    }

    // Clean up
    let _ = run_redis_cli(&["DEL", "newcounter"]);

    let result = run_redis_cli(&["INCR", "newcounter"]);
    assert!(result.is_ok(), "INCR new key failed: {:?}", result);
    assert_eq!(result.unwrap(), "1");
}

#[test]
fn test_redis_cli_mset_mget() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["MSET", "mkey1", "mval1", "mkey2", "mval2", "mkey3", "mval3"]);
    assert!(result.is_ok(), "MSET failed: {:?}", result);
    assert_eq!(result.unwrap(), "OK");

    let result = run_redis_cli(&["MGET", "mkey1", "mkey2", "mkey3"]);
    assert!(result.is_ok(), "MGET failed: {:?}", result);
    let output = result.unwrap();
    assert!(output.contains("mval1"));
    assert!(output.contains("mval2"));
    assert!(output.contains("mval3"));
}

#[test]
fn test_redis_cli_setex() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["SETEX", "exkey", "10", "temporary"]);
    assert!(result.is_ok(), "SETEX failed: {:?}", result);
    assert_eq!(result.unwrap(), "OK");

    let result = run_redis_cli(&["GET", "exkey"]);
    assert!(result.is_ok(), "GET after SETEX failed: {:?}", result);
    assert_eq!(result.unwrap(), "temporary");
}

// Phase 3 integration tests

#[test]
fn test_redis_cli_expire_ttl() {
    if skip_if_unavailable() {
        return;
    }

    // Set a key
    run_redis_cli(&["SET", "ttlkey", "value"]).unwrap();

    // Set expiration
    let result = run_redis_cli(&["EXPIRE", "ttlkey", "100"]);
    assert!(result.is_ok(), "EXPIRE failed: {:?}", result);
    assert_eq!(result.unwrap(), "1");

    // Check TTL
    let result = run_redis_cli(&["TTL", "ttlkey"]);
    assert!(result.is_ok(), "TTL failed: {:?}", result);
    let ttl: i64 = result.unwrap().parse().unwrap();
    assert!(ttl >= 99 && ttl <= 100, "TTL was {}", ttl);
}

#[test]
fn test_redis_cli_expire_negative_deletes() {
    if skip_if_unavailable() {
        return;
    }

    // Set a key
    run_redis_cli(&["SET", "negexpkey", "value"]).unwrap();

    // Negative expire should delete the key
    let result = run_redis_cli(&["EXPIRE", "negexpkey", "-1"]);
    assert!(result.is_ok(), "EXPIRE negative failed: {:?}", result);
    assert_eq!(result.unwrap(), "1");

    // Key should be gone
    let result = run_redis_cli(&["GET", "negexpkey"]);
    let output = result.unwrap();
    assert!(output.is_empty() || output == "(nil)");
}

#[test]
fn test_redis_cli_ttl_no_expiry() {
    if skip_if_unavailable() {
        return;
    }

    // Set a key without expiration
    run_redis_cli(&["SET", "noexpkey", "value"]).unwrap();

    let result = run_redis_cli(&["TTL", "noexpkey"]);
    assert!(result.is_ok(), "TTL failed: {:?}", result);
    assert_eq!(result.unwrap(), "-1");
}

#[test]
fn test_redis_cli_ttl_nonexistent() {
    if skip_if_unavailable() {
        return;
    }

    let result = run_redis_cli(&["TTL", "nonexistent_key_99999"]);
    assert!(result.is_ok(), "TTL failed: {:?}", result);
    assert_eq!(result.unwrap(), "-2");
}

#[test]
fn test_redis_cli_persist() {
    if skip_if_unavailable() {
        return;
    }

    // Set key with expiration
    run_redis_cli(&["SETEX", "persistkey", "100", "value"]).unwrap();

    // Verify TTL exists
    let result = run_redis_cli(&["TTL", "persistkey"]);
    let ttl: i64 = result.unwrap().parse().unwrap();
    assert!(ttl > 0);

    // Remove expiration
    let result = run_redis_cli(&["PERSIST", "persistkey"]);
    assert!(result.is_ok(), "PERSIST failed: {:?}", result);
    assert_eq!(result.unwrap(), "1");

    // Verify no TTL
    let result = run_redis_cli(&["TTL", "persistkey"]);
    assert_eq!(result.unwrap(), "-1");
}

#[test]
fn test_redis_cli_keys_pattern() {
    if skip_if_unavailable() {
        return;
    }

    // Clean up and set test keys
    let _ = run_redis_cli(&["DEL", "keystest:a", "keystest:b", "keystest:c", "other"]);
    run_redis_cli(&[
        "MSET",
        "keystest:a",
        "1",
        "keystest:b",
        "2",
        "keystest:c",
        "3",
        "other",
        "4",
    ])
    .unwrap();

    let result = run_redis_cli(&["KEYS", "keystest:*"]);
    assert!(result.is_ok(), "KEYS failed: {:?}", result);
    let output = result.unwrap();
    assert!(output.contains("keystest:a"));
    assert!(output.contains("keystest:b"));
    assert!(output.contains("keystest:c"));
    assert!(!output.contains("other"));
}

#[test]
fn test_redis_cli_keys_single_char() {
    if skip_if_unavailable() {
        return;
    }

    // Clean up and set test keys
    let _ = run_redis_cli(&["DEL", "k1", "k2", "k10"]);
    run_redis_cli(&["MSET", "k1", "a", "k2", "b", "k10", "c"]).unwrap();

    let result = run_redis_cli(&["KEYS", "k?"]);
    assert!(result.is_ok(), "KEYS failed: {:?}", result);
    let output = result.unwrap();
    assert!(output.contains("k1"));
    assert!(output.contains("k2"));
    assert!(!output.contains("k10")); // k10 has 2 chars after k
}
