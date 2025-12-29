#!/bin/bash
# Script to run throughput benchmarks

set -e

echo "Building rudis..."
cargo build --release

echo "Building benchmark..."
cargo build --release --bench throughput

echo ""
echo "Starting rudis server (release mode) in background..."
cargo run --release &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "Error: Server failed to start"
    exit 1
fi

echo "Server started with PID: $SERVER_PID"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "Shutting down server..."
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    echo "Server stopped"
}

trap cleanup EXIT INT TERM

# Run the benchmark
cargo bench --bench throughput

echo ""
echo "Benchmark complete!"
