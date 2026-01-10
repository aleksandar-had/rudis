# Redis vs Rudis Benchmark Results

**Date:** 2026-01-10 14:07:06

**System:** Darwin 25.2.0 (arm64)

## Single-Threaded (50 clients)

**Args:** `-t ping,set,get -n 10000 -c 50 -q`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| PING_INLINE | 136986 | 103093 | .75x |
| PING_MBULK | 217391 | 138889 | .63x |
| SET | 227273 | 133333 | .58x |
| GET | 217391 | 138889 | .63x |

## Multi-Threaded (100 clients, 8 threads)

**Args:** `-t ping,set,get -n 100000 -c 100 --threads 8 -q`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| PING_INLINE | 99800 | 99800 | 1.00x |
| PING_MBULK | 99900 | 99800 | .99x |
| SET | 99800 | 99800 | 1.00x |
| GET | 99800 | 99900 | 1.00x |

