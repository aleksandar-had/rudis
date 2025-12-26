use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

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

#[test]
fn test_redis_cli_ping() {
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return;
    }

    // Give the server time to start (if running separately)
    thread::sleep(Duration::from_millis(100));

    let result = run_redis_cli(&["PING"]);
    assert!(result.is_ok(), "PING command failed: {:?}", result);
    assert_eq!(result.unwrap(), "PONG");
}

#[test]
fn test_redis_cli_ping_with_message() {
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return;
    }

    thread::sleep(Duration::from_millis(100));

    let result = run_redis_cli(&["PING", "hello"]);
    assert!(result.is_ok(), "PING with message failed: {:?}", result);
    assert_eq!(result.unwrap(), "hello");
}

#[test]
fn test_redis_cli_ping_case_insensitive() {
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return;
    }

    thread::sleep(Duration::from_millis(100));

    // redis-cli sends commands as-is, but our server should handle case insensitivity
    let result = run_redis_cli(&["ping"]);
    assert!(result.is_ok(), "ping (lowercase) failed: {:?}", result);
    assert_eq!(result.unwrap(), "PONG");
}

#[test]
fn test_redis_cli_ping_with_spaces() {
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return;
    }

    thread::sleep(Duration::from_millis(100));

    let result = run_redis_cli(&["PING", "hello world"]);
    assert!(result.is_ok(), "PING with spaces failed: {:?}", result);
    assert_eq!(result.unwrap(), "hello world");
}

#[test]
fn test_redis_cli_unknown_command() {
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return;
    }

    thread::sleep(Duration::from_millis(100));

    let result = run_redis_cli(&["NOTACOMMAND"]);
    // Should fail or return an error
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
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return;
    }

    thread::sleep(Duration::from_millis(100));

    // Test that we can handle multiple sequential requests
    for i in 0..5 {
        let msg = format!("message{}", i);
        let result = run_redis_cli(&["PING", &msg]);
        assert!(result.is_ok(), "PING #{} failed: {:?}", i, result);
        assert_eq!(result.unwrap(), msg);
    }
}

#[test]
fn test_redis_cli_ping_empty_string() {
    if !redis_cli_available() {
        eprintln!("redis-cli not found, skipping integration test");
        return;
    }

    thread::sleep(Duration::from_millis(100));

    let result = run_redis_cli(&["PING", ""]);
    assert!(
        result.is_ok(),
        "PING with empty string failed: {:?}",
        result
    );
    // Empty string should be echoed back
    assert_eq!(result.unwrap(), "");
}
