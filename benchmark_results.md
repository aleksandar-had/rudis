# Redis vs Rudis Benchmark Results

**Date:** 2026-01-28 20:21:30

**System:** Darwin 25.2.0 (arm64)

## Single-Threaded (50 clients)

**Args:** `-t ping,set,get -n 10000 -c 50 -q`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| PING_INLINE | 126582 | 104167 | .82x |
| PING_MBULK | 222222 | 140845 | .63x |
| SET | 212766 | 140845 | .66x |
| GET | 227273 | 138889 | .61x |

## Multi-Threaded (100 clients, 8 threads)

**Args:** `-t ping,set,get -n 100000 -c 100 --threads 8 -q`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| PING_INLINE | 99900 | 99800 | .99x |
| PING_MBULK | 99800 | 99800 | 1.00x |
| SET | 99900 | 99900 | 1.00x |
| GET | 99800 | 99900 | 1.00x |

## Phase 3 - TTL Commands (50 clients) - TTL

**Args:** `-n 50000 -c 50 -q -r 10000 TTL ttlkey___rand_int__`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| TTL | 202429 | 132626 | .65x |

## Phase 3 - TTL Commands (50 clients) - EXPIRE

**Args:** `-n 50000 -c 50 -q -r 10000 EXPIRE ttlkey___rand_int__ 100`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| EXPIRE | 205761 | 134048 | .65x |

## Phase 3 - TTL Commands (50 clients) - PERSIST

**Args:** `-n 50000 -c 50 -q -r 10000 PERSIST ttlkey___rand_int__`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| PERSIST | 179211 | 132979 | .74x |

## Phase 3 - KEYS Scaling - KEYS * (average)

**Args:** `-n 100 -c 1 -q KEYS "*"`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| KEYS | 7692 | 3571 | .39x |

## Phase 3 - KEYS Scaling - KEYS pattern (average)

**Args:** `-n 100 -c 1 -q KEYS "scankey_1*"`

| Command | Redis (req/s) | Rudis (req/s) | Ratio |
|---------|---------------|---------------|-------|
| KEYS | 12500 | 4762 | .30x |
