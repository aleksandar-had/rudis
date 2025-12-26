#!/bin/bash
# Script to run integration tests with redis-cli

set -e

echo "Building rudis..."
cargo build

echo "Starting rudis server in background..."
cargo run &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
sleep 2

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "Error: Server failed to start"
    exit 1
fi

echo "Server started with PID: $SERVER_PID"

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "Shutting down server..."
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    echo "Server stopped"
}

trap cleanup EXIT INT TERM

# Check if redis-cli is available
if ! command -v redis-cli &> /dev/null; then
    echo "Warning: redis-cli not found. Integration tests will be skipped."
    echo "To install redis-cli:"
    echo "  macOS: brew install redis"
    echo "  Ubuntu: sudo apt-get install redis-tools"
    echo ""
fi

echo "Running integration tests..."
cargo test --test redis_cli_integration -- --test-threads=1

echo ""
echo "All integration tests passed!"
