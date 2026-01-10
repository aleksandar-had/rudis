#!/bin/bash
# Compare redis-benchmark results between official Redis and Rudis
#
# Usage: ./compare_benchmark.sh [options]
# Options are passed directly to redis-benchmark (e.g., -n 10000 -c 50)

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

# Initialize markdown output
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

if [ -n "$CUSTOM_ARGS" ]; then
    # Custom args provided - run single benchmark
    run_benchmark "$CUSTOM_ARGS" "CUSTOM BENCHMARK"
else
    # No args - run both single and multi-threaded
    run_benchmark "$SINGLE_THREAD_ARGS" "Single-Threaded (50 clients)"
    run_benchmark "$MULTI_THREAD_ARGS" "Multi-Threaded (100 clients, 8 threads)"
fi

echo ""
echo "Results saved to $OUTPUT_FILE"
echo ""
