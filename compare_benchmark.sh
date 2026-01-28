#!/bin/bash
# Compare redis-benchmark results between official Redis and Rudis
#
# Usage: ./compare_benchmark.sh [options]
#   (no args)          - Run Phase 2 benchmarks (PING, SET, GET)
#   --phase3-ttl       - Run Phase 3 TTL commands (EXPIRE, TTL, PERSIST)
#   --phase3-keys      - Run Phase 3 KEYS scaling tests
#   --phase3-all       - Run all Phase 3 benchmarks
#   [custom args]      - Pass args directly to redis-benchmark

set -e

PORT=6379
SINGLE_THREAD_ARGS="-t ping,set,get -n 10000 -c 50 -q"
MULTI_THREAD_ARGS="-t ping,set,get -n 100000 -c 100 --threads 8 -q"
OUTPUT_FILE="benchmark_results.md"

# Use provided args or run both modes
CUSTOM_ARGS="$@"

cleanup() {
    [ -n "$SERVER_PID" ] && kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
}

trap cleanup EXIT INT TERM

# Check for redis-server
if ! command -v redis-server &> /dev/null; then
    echo "Error: redis-server not found. Please install Redis."
    exit 1
fi

# Check for redis-benchmark
if ! command -v redis-benchmark &> /dev/null; then
    echo "Error: redis-benchmark not found. Please install Redis."
    exit 1
fi

# Build rudis
echo "Building rudis (release mode)..."
cargo build --release --quiet

populate_ttl_keys() {
    echo "Populating 10,000 keys with expiration..."
    redis-cli -p $PORT FLUSHDB > /dev/null 2>&1
    for i in {0..9999}; do
        redis-cli -p $PORT SETEX "ttlkey_$i" 3600 "value" > /dev/null 2>&1
    done
}

run_phase3_ttl_commands() {
    local LABEL="$1"

    # Benchmark TTL command
    run_benchmark_with_setup populate_ttl_keys "-n 50000 -c 50 -q -r 10000 TTL ttlkey___rand_int__" "$LABEL - TTL"

    # Benchmark EXPIRE command
    run_benchmark_with_setup populate_ttl_keys "-n 50000 -c 50 -q -r 10000 EXPIRE ttlkey___rand_int__ 100" "$LABEL - EXPIRE"

    # Benchmark PERSIST command
    run_benchmark_with_setup populate_ttl_keys "-n 50000 -c 50 -q -r 10000 PERSIST ttlkey___rand_int__" "$LABEL - PERSIST"
}

populate_keys_1000() {
    echo "Populating 1,000 keys..."
    redis-cli -p $PORT FLUSHDB > /dev/null 2>&1
    for i in $(seq 1 1000); do
        redis-cli -p $PORT SET "scankey_$i" "value" > /dev/null 2>&1
    done
}

populate_keys_10000() {
    echo "Populating 10,000 keys..."
    redis-cli -p $PORT FLUSHDB > /dev/null 2>&1
    for i in $(seq 1 10000); do
        redis-cli -p $PORT SET "scankey_$i" "value" > /dev/null 2>&1
    done
}

run_phase3_keys_scaling() {
    local LABEL="$1"

    # 1K keys - use -c 1 since KEYS returns arrays and causes issues with multiple clients
    run_benchmark_with_setup populate_keys_1000 '-n 100 -c 1 -q KEYS "*"' "$LABEL - KEYS * (1000 keys)"
    run_benchmark_with_setup populate_keys_1000 '-n 100 -c 1 -q KEYS "scankey_1*"' "$LABEL - KEYS pattern (1000 keys)"

    # 10K keys
    run_benchmark_with_setup populate_keys_10000 '-n 100 -c 1 -q KEYS "*"' "$LABEL - KEYS * (10000 keys)"
    run_benchmark_with_setup populate_keys_10000 '-n 100 -c 1 -q KEYS "scankey_1*"' "$LABEL - KEYS pattern (10000 keys)"
}

run_benchmark_with_setup() {
    local SETUP_FUNC="$1"
    local BENCH_ARGS="$2"
    local LABEL="$3"

    echo ""
    echo "============================================================"
    echo "  $LABEL"
    echo "  Args: $BENCH_ARGS"
    echo "============================================================"

    # Append to markdown
    {
        echo "## $LABEL"
        echo ""
        echo "**Args:** \`$BENCH_ARGS\`"
        echo ""
    } >> "$OUTPUT_FILE"

    # --- Benchmark Redis ---
    echo "Starting redis-server..."
    redis-server --port $PORT --daemonize no --loglevel warning > /dev/null 2>&1 &
    SERVER_PID=$!
    sleep 1

    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "Error: Failed to start redis-server"
        exit 1
    fi

    # Run setup function to populate keys
    $SETUP_FUNC

    echo "Benchmarking Redis..."
    REDIS_FULL_OUTPUT=$(eval "redis-benchmark -p $PORT $BENCH_ARGS" 2>&1)
    REDIS_RPS=$(echo "$REDIS_FULL_OUTPUT" | grep -oE '[0-9.]+\s+requests per second' | grep -oE '[0-9.]+' | head -1)

    if [ -z "$REDIS_RPS" ]; then
        echo "Warning: Could not parse Redis benchmark output"
        echo "$REDIS_FULL_OUTPUT"
    fi

    # Stop Redis
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    unset SERVER_PID
    sleep 1

    # --- Benchmark Rudis ---
    echo "Starting rudis..."
    ./target/release/rudis > /dev/null 2>&1 &
    SERVER_PID=$!
    sleep 1

    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "Error: Failed to start rudis"
        exit 1
    fi

    # Run setup function to populate keys
    $SETUP_FUNC

    echo "Benchmarking Rudis..."
    RUDIS_FULL_OUTPUT=$(eval "redis-benchmark -p $PORT $BENCH_ARGS" 2>&1)
    RUDIS_RPS=$(echo "$RUDIS_FULL_OUTPUT" | grep -oE '[0-9.]+\s+requests per second' | grep -oE '[0-9.]+' | head -1)

    if [ -z "$RUDIS_RPS" ]; then
        echo "Warning: Could not parse Rudis benchmark output"
        echo "$RUDIS_FULL_OUTPUT"
    fi

    # Stop Rudis
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    unset SERVER_PID

    # Extract command name from benchmark args (first word after flags)
    CMD_NAME=$(echo "$BENCH_ARGS" | grep -oE '[A-Z]+' | head -1)

    # --- Comparison ---
    echo ""
    printf "%-14s %14s %14s %10s\n" "Command" "Redis (req/s)" "Rudis (req/s)" "Ratio"
    printf "%-14s %14s %14s %10s\n" "-----------" "-------------" "-------------" "-------"

    # Markdown table header
    {
        echo "| Command | Redis (req/s) | Rudis (req/s) | Ratio |"
        echo "|---------|---------------|---------------|-------|"
    } >> "$OUTPUT_FILE"

    if [ -n "$REDIS_RPS" ] && [ -n "$RUDIS_RPS" ]; then
        # Calculate ratio (rudis/redis)
        if (( $(echo "$REDIS_RPS > 0" | bc -l) )); then
            ratio=$(echo "scale=2; $RUDIS_RPS / $REDIS_RPS" | bc -l)
        else
            ratio="N/A"
        fi

        # Console output
        printf "%-14s %14.0f %14.0f %9sx\n" "$CMD_NAME" "$REDIS_RPS" "$RUDIS_RPS" "$ratio"

        # Markdown output
        printf "| %s | %.0f | %.0f | %sx |\n" "$CMD_NAME" "$REDIS_RPS" "$RUDIS_RPS" "$ratio" >> "$OUTPUT_FILE"
    fi

    echo "" >> "$OUTPUT_FILE"
}

run_benchmark() {
    local BENCH_ARGS="$1"
    local LABEL="$2"

    echo ""
    echo "============================================================"
    echo "  $LABEL"
    echo "  Args: $BENCH_ARGS"
    echo "============================================================"

    # Append to markdown
    {
        echo "## $LABEL"
        echo ""
        echo "**Args:** \`$BENCH_ARGS\`"
        echo ""
    } >> "$OUTPUT_FILE"

    # --- Benchmark Redis ---
    echo "Starting redis-server..."
    redis-server --port $PORT --daemonize no --loglevel warning > /dev/null 2>&1 &
    SERVER_PID=$!
    sleep 1

    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "Error: Failed to start redis-server"
        exit 1
    fi

    echo "Benchmarking Redis..."
    REDIS_OUTPUT=$(redis-benchmark -p $PORT $BENCH_ARGS 2>/dev/null | tr -d '\r' | grep -oE '[A-Z_]+: [0-9.]+ requests per second')

    # Stop Redis
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    unset SERVER_PID
    sleep 1

    # --- Benchmark Rudis ---
    echo "Starting rudis..."
    ./target/release/rudis > /dev/null 2>&1 &
    SERVER_PID=$!
    sleep 1

    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "Error: Failed to start rudis"
        exit 1
    fi

    echo "Benchmarking Rudis..."
    RUDIS_OUTPUT=$(redis-benchmark -p $PORT $BENCH_ARGS 2>/dev/null | tr -d '\r' | grep -oE '[A-Z_]+: [0-9.]+ requests per second')

    # Stop Rudis
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    unset SERVER_PID

    # --- Comparison ---
    echo ""
    printf "%-14s %14s %14s %10s\n" "Command" "Redis (req/s)" "Rudis (req/s)" "Ratio"
    printf "%-14s %14s %14s %10s\n" "-----------" "-------------" "-------------" "-------"

    # Markdown table header
    {
        echo "| Command | Redis (req/s) | Rudis (req/s) | Ratio |"
        echo "|---------|---------------|---------------|-------|"
    } >> "$OUTPUT_FILE"

    # Parse and compare results
    while IFS= read -r line; do
        if [[ $line =~ ^([A-Z_]+):\ ([0-9.]+)\ requests ]]; then
            cmd="${BASH_REMATCH[1]}"
            redis_rps="${BASH_REMATCH[2]}"

            # Find corresponding rudis result
            rudis_line=$(echo "$RUDIS_OUTPUT" | grep "^$cmd:" || true)
            if [[ $rudis_line =~ ^([A-Z_]+):\ ([0-9.]+)\ requests ]]; then
                rudis_rps="${BASH_REMATCH[2]}"

                # Calculate ratio (rudis/redis)
                if (( $(echo "$redis_rps > 0" | bc -l) )); then
                    ratio=$(echo "scale=2; $rudis_rps / $redis_rps" | bc -l)
                else
                    ratio="N/A"
                fi

                # Console output
                printf "%-14s %14.0f %14.0f %9sx\n" "$cmd" "$redis_rps" "$rudis_rps" "$ratio"

                # Markdown output
                printf "| %s | %.0f | %.0f | %sx |\n" "$cmd" "$redis_rps" "$rudis_rps" "$ratio" >> "$OUTPUT_FILE"
            fi
        fi
    done <<< "$REDIS_OUTPUT"

    echo "" >> "$OUTPUT_FILE"
}

echo "=== Redis vs Rudis Benchmark Comparison ==="

# Parse arguments
RUN_PHASE2=true
RUN_PHASE3_TTL=false
RUN_PHASE3_KEYS=false

if [ "$1" = "--phase3-ttl" ]; then
    RUN_PHASE2=false
    RUN_PHASE3_TTL=true
elif [ "$1" = "--phase3-keys" ]; then
    RUN_PHASE2=false
    RUN_PHASE3_KEYS=true
elif [ "$1" = "--phase3-all" ]; then
    RUN_PHASE2=false
    RUN_PHASE3_TTL=true
    RUN_PHASE3_KEYS=true
elif [ -n "$CUSTOM_ARGS" ]; then
    # Custom args provided - run single benchmark
    run_benchmark "$CUSTOM_ARGS" "CUSTOM BENCHMARK"
    RUN_PHASE2=false
fi

# Run selected benchmarks
if [ "$RUN_PHASE2" = true ]; then
    # Create file with header if it doesn't exist, otherwise append
    if [ ! -f "$OUTPUT_FILE" ]; then
        {
            echo "# Redis vs Rudis Benchmark Results"
            echo ""
            echo "**Date:** $(date '+%Y-%m-%d %H:%M:%S')"
            echo ""
            echo "**System:** $(uname -s) $(uname -r) ($(uname -m))"
            echo ""
            echo "> **Note:** These benchmarks are informal and may not be representative of real-world performance. The multi-threaded benchmark tests the client's ability to generate load, not necessarily the server's multi-threaded capabilities. AFAIK, Redis is primarily single-threaded for command execution."
            echo ""
        } > "$OUTPUT_FILE"
    fi

    run_benchmark "$SINGLE_THREAD_ARGS" "Single-Threaded (50 clients)"
    run_benchmark "$MULTI_THREAD_ARGS" "Multi-Threaded (100 clients, 8 threads)"
fi

if [ "$RUN_PHASE3_TTL" = true ]; then
    # Append Phase 3 results to existing file (or create if doesn't exist)
    if [ ! -f "$OUTPUT_FILE" ]; then
        {
            echo "# Redis vs Rudis Benchmark Results"
            echo ""
            echo "**Date:** $(date '+%Y-%m-%d %H:%M:%S')"
            echo ""
            echo "**System:** $(uname -s) $(uname -r) ($(uname -m))"
            echo ""
        } > "$OUTPUT_FILE"
    fi

    run_phase3_ttl_commands "Phase 3 - TTL Commands (50 clients)"
fi

if [ "$RUN_PHASE3_KEYS" = true ]; then
    # Append Phase 3 results to existing file (or create if doesn't exist)
    if [ ! -f "$OUTPUT_FILE" ]; then
        {
            echo "# Redis vs Rudis Benchmark Results"
            echo ""
            echo "**Date:** $(date '+%Y-%m-%d %H:%M:%S')"
            echo ""
            echo "**System:** $(uname -s) $(uname -r) ($(uname -m))"
            echo ""
        } > "$OUTPUT_FILE"
    fi

    run_phase3_keys_scaling "Phase 3 - KEYS Scaling"
fi

echo ""
echo "Results saved to $OUTPUT_FILE"
echo ""
