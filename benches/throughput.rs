//! Throughput benchmark for rudis
//!
//! This benchmark measures operations per second for various Redis commands
//! using both single-threaded and multi-threaded client configurations.
//!
//! Run with: cargo run && cargo bench --bench throughput
//! Or use the script: ./run_benchmark.sh

use rand::{Rng, SeedableRng};
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const SERVER_ADDR: &str = "127.0.0.1:6379";
const WARMUP_DURATION: Duration = Duration::from_secs(2);
const BENCHMARK_DURATION: Duration = Duration::from_secs(15);

/// RESP protocol helpers
fn encode_command(args: &[&str]) -> Vec<u8> {
    let mut buf = format!("*{}\r\n", args.len());
    for arg in args {
        buf.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
    }
    buf.into_bytes()
}

fn read_response(stream: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<usize> {
    stream.read(buf)
}

/// Single connection benchmark runner
struct BenchmarkRunner {
    stream: TcpStream,
    read_buf: Vec<u8>,
}

impl BenchmarkRunner {
    fn new() -> std::io::Result<Self> {
        let stream = TcpStream::connect(SERVER_ADDR)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            read_buf: vec![0u8; 4096],
        })
    }

    fn run_command(&mut self, cmd: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(cmd)?;
        read_response(&mut self.stream, &mut self.read_buf)?;
        Ok(())
    }

    fn set(&mut self, key: &str, value: &str) -> std::io::Result<()> {
        let cmd = encode_command(&["SET", key, value]);
        self.run_command(&cmd)
    }

    fn get(&mut self, key: &str) -> std::io::Result<()> {
        let cmd = encode_command(&["GET", key]);
        self.run_command(&cmd)
    }

    fn incr(&mut self, key: &str) -> std::io::Result<()> {
        let cmd = encode_command(&["INCR", key]);
        self.run_command(&cmd)
    }

    fn mset(&mut self, pairs: &[(&str, &str)]) -> std::io::Result<()> {
        let mut args: Vec<&str> = vec!["MSET"];
        for (k, v) in pairs {
            args.push(k);
            args.push(v);
        }
        let cmd = encode_command(&args);
        self.run_command(&cmd)
    }

    fn mget(&mut self, keys: &[&str]) -> std::io::Result<()> {
        let mut args: Vec<&str> = vec!["MGET"];
        args.extend_from_slice(keys);
        let cmd = encode_command(&args);
        self.run_command(&cmd)
    }
}

/// Benchmark result
#[derive(Debug, Clone)]
struct BenchmarkResult {
    name: String,
    threads: usize,
    duration: Duration,
    operations: u64,
    ops_per_sec: f64,
    avg_latency_us: f64,
}

impl BenchmarkResult {
    fn print(&self) {
        println!(
            "{:25} {:>12.0} ops/sec  {:>8.2} µs/op  ({} ops in {:.2}s)",
            self.name,
            self.ops_per_sec,
            self.avg_latency_us,
            self.operations,
            self.duration.as_secs_f64()
        );
    }

    fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{:.2},{:.2}",
            self.name,
            self.threads,
            self.operations,
            self.duration.as_secs_f64(),
            self.ops_per_sec,
            self.avg_latency_us
        )
    }
}

/// All benchmark results
#[derive(Debug)]
struct BenchmarkReport {
    timestamp: String,
    server_addr: String,
    warmup_duration: Duration,
    benchmark_duration: Duration,
    results: Vec<BenchmarkResult>,
}

impl BenchmarkReport {
    fn new() -> Self {
        let timestamp = chrono_lite_timestamp();
        Self {
            timestamp,
            server_addr: SERVER_ADDR.to_string(),
            warmup_duration: WARMUP_DURATION,
            benchmark_duration: BENCHMARK_DURATION,
            results: Vec::new(),
        }
    }

    fn add(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let mut file = File::create(path)?;

        // Write header
        writeln!(file, "# Rudis Throughput Benchmark Results")?;
        writeln!(file, "# Timestamp: {}", self.timestamp)?;
        writeln!(file, "# Server: {}", self.server_addr)?;
        writeln!(file, "# Warmup: {:?}", self.warmup_duration)?;
        writeln!(file, "# Benchmark Duration: {:?}", self.benchmark_duration)?;
        writeln!(file, "#")?;

        // CSV header
        writeln!(
            file,
            "command,threads,operations,duration_secs,ops_per_sec,avg_latency_us"
        )?;

        // Data rows
        for result in &self.results {
            writeln!(file, "{}", result.to_csv_row())?;
        }

        writeln!(file)?;
        writeln!(file, "# Summary")?;

        // Single-threaded summary
        let single_threaded: Vec<_> = self.results.iter().filter(|r| r.threads == 1).collect();
        if !single_threaded.is_empty() {
            writeln!(file, "# Single-threaded:")?;
            for r in &single_threaded {
                writeln!(file, "#   {}: {:.0} ops/sec", r.name, r.ops_per_sec)?;
            }
        }

        // Multi-threaded peak
        if let Some(peak) = self
            .results
            .iter()
            .max_by(|a, b| a.ops_per_sec.partial_cmp(&b.ops_per_sec).unwrap())
        {
            writeln!(
                file,
                "# Peak throughput: {:.0} ops/sec ({}, {} threads)",
                peak.ops_per_sec, peak.name, peak.threads
            )?;
        }

        // Scaling analysis for SET
        let set_1t = self
            .results
            .iter()
            .find(|r| r.name == "SET" && r.threads == 1);
        let set_16t = self
            .results
            .iter()
            .find(|r| r.name == "SET" && r.threads == 16);
        if let (Some(s1), Some(s16)) = (set_1t, set_16t) {
            writeln!(
                file,
                "# SET scaling (1T -> 16T): {:.2}x",
                s16.ops_per_sec / s1.ops_per_sec
            )?;
        }

        Ok(())
    }

    fn print_summary(&self) {
        println!("\n=== Summary ===\n");

        // Single-threaded peaks
        let single_threaded: Vec<_> = self.results.iter().filter(|r| r.threads == 1).collect();
        if !single_threaded.is_empty() {
            if let Some(peak) = single_threaded
                .iter()
                .max_by(|a, b| a.ops_per_sec.partial_cmp(&b.ops_per_sec).unwrap())
            {
                println!(
                    "Single-threaded peak: {:.0} ops/sec ({})",
                    peak.ops_per_sec, peak.name
                );
            }
        }

        // Overall peak
        if let Some(peak) = self
            .results
            .iter()
            .max_by(|a, b| a.ops_per_sec.partial_cmp(&b.ops_per_sec).unwrap())
        {
            println!(
                "Overall peak: {:.0} ops/sec ({}, {} threads)",
                peak.ops_per_sec, peak.name, peak.threads
            );
        }

        // Scaling
        let set_1t = self
            .results
            .iter()
            .find(|r| r.name == "SET" && r.threads == 1);
        let set_16t = self
            .results
            .iter()
            .find(|r| r.name == "SET" && r.threads == 16);
        if let (Some(s1), Some(s16)) = (set_1t, set_16t) {
            println!(
                "SET scaling (1T -> 16T): {:.2}x",
                s16.ops_per_sec / s1.ops_per_sec
            );
        }
    }
}

/// Simple timestamp without external dependencies
fn chrono_lite_timestamp() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    format!("{}", duration.as_secs())
}

/// Run a single-threaded benchmark
fn run_single_threaded_benchmark<F>(name: &str, mut op: F) -> BenchmarkResult
where
    F: FnMut() -> std::io::Result<()>,
{
    // Warmup
    let warmup_start = Instant::now();
    while warmup_start.elapsed() < WARMUP_DURATION {
        for _ in 0..1000 {
            let _ = op();
        }
    }

    // Benchmark
    let mut operations = 0u64;
    let start = Instant::now();
    while start.elapsed() < BENCHMARK_DURATION {
        for _ in 0..1000 {
            if op().is_ok() {
                operations += 1;
            }
        }
    }
    let duration = start.elapsed();

    let ops_per_sec = operations as f64 / duration.as_secs_f64();
    let avg_latency_us = duration.as_micros() as f64 / operations as f64;

    BenchmarkResult {
        name: name.to_string(),
        threads: 1,
        duration,
        operations,
        ops_per_sec,
        avg_latency_us,
    }
}

/// Run a multi-threaded benchmark
fn run_multi_threaded_benchmark<F>(name: &str, num_threads: usize, op_factory: F) -> BenchmarkResult
where
    F: Fn(usize) -> Box<dyn FnMut() -> std::io::Result<()> + Send> + Send + Sync,
{
    let total_ops = Arc::new(AtomicU64::new(0));
    let start_barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let total_ops = Arc::clone(&total_ops);
            let start_barrier = Arc::clone(&start_barrier);
            let stop_flag = Arc::clone(&stop_flag);
            let mut op = op_factory(thread_id);

            thread::spawn(move || {
                // Warmup
                let warmup_start = Instant::now();
                while warmup_start.elapsed() < WARMUP_DURATION {
                    for _ in 0..100 {
                        let _ = op();
                    }
                }

                // Wait for all threads to be ready
                start_barrier.wait();

                // Benchmark
                let mut local_ops = 0u64;
                while !stop_flag.load(Ordering::Relaxed) {
                    for _ in 0..100 {
                        if op().is_ok() {
                            local_ops += 1;
                        }
                    }
                }
                total_ops.fetch_add(local_ops, Ordering::Relaxed);
            })
        })
        .collect();

    // Start all threads
    start_barrier.wait();
    let start = Instant::now();

    // Let them run
    thread::sleep(BENCHMARK_DURATION);
    stop_flag.store(true, Ordering::Relaxed);

    // Wait for completion
    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let operations = total_ops.load(Ordering::Relaxed);
    let ops_per_sec = operations as f64 / duration.as_secs_f64();
    let avg_latency_us = (duration.as_micros() as f64 * num_threads as f64) / operations as f64;

    BenchmarkResult {
        name: name.to_string(),
        threads: num_threads,
        duration,
        operations,
        ops_per_sec,
        avg_latency_us,
    }
}

fn check_server() -> bool {
    match TcpStream::connect(SERVER_ADDR) {
        Ok(mut stream) => {
            let cmd = encode_command(&["PING"]);
            stream.write_all(&cmd).is_ok()
        }
        Err(_) => false,
    }
}

fn main() {
    println!("=== Rudis Throughput Benchmark ===\n");

    if !check_server() {
        eprintln!("Error: Cannot connect to rudis server at {}", SERVER_ADDR);
        eprintln!("Please start the server first: cargo run");
        std::process::exit(1);
    }

    println!("Server: {}", SERVER_ADDR);
    println!("Warmup: {:?}", WARMUP_DURATION);
    println!("Benchmark duration: {:?}", BENCHMARK_DURATION);
    println!();

    let mut report = BenchmarkReport::new();

    // --- Single-threaded benchmarks ---
    println!("--- Single-threaded Benchmarks ---\n");

    // SET (fixed key)
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        let result = run_single_threaded_benchmark("SET", || runner.set("benchmark:key", "value"));
        result.print();
        report.add(result);
    }

    // SET (random keys)
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        let mut rng = rand::rngs::SmallRng::from_entropy();
        let result = run_single_threaded_benchmark("SET (random keys)", || {
            let key = format!("benchmark:rand:{}", rng.r#gen::<u32>());
            runner.set(&key, "value")
        });
        result.print();
        report.add(result);
    }

    // GET (existing key)
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        runner.set("benchmark:get", "testvalue").unwrap();
        let result = run_single_threaded_benchmark("GET", || runner.get("benchmark:get"));
        result.print();
        report.add(result);
    }

    // GET (missing key)
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        let result =
            run_single_threaded_benchmark("GET (missing)", || runner.get("benchmark:nonexistent"));
        result.print();
        report.add(result);
    }

    // INCR
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        runner.set("benchmark:counter", "0").unwrap();
        let result = run_single_threaded_benchmark("INCR", || runner.incr("benchmark:counter"));
        result.print();
        report.add(result);
    }

    // SET + GET pipeline
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        let result = run_single_threaded_benchmark("SET+GET", || {
            runner.set("benchmark:pipeline", "value")?;
            runner.get("benchmark:pipeline")
        });
        result.print();
        report.add(result);
    }

    // MSET (3 keys)
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        let result = run_single_threaded_benchmark("MSET (3 keys)", || {
            runner.mset(&[("k1", "v1"), ("k2", "v2"), ("k3", "v3")])
        });
        result.print();
        report.add(result);
    }

    // MGET (3 keys)
    {
        let mut runner = BenchmarkRunner::new().expect("Failed to connect");
        runner
            .mset(&[("mk1", "v1"), ("mk2", "v2"), ("mk3", "v3")])
            .unwrap();
        let result =
            run_single_threaded_benchmark("MGET (3 keys)", || runner.mget(&["mk1", "mk2", "mk3"]));
        result.print();
        report.add(result);
    }

    println!();

    // --- Multi-threaded benchmarks ---
    for num_threads in [4, 8, 16] {
        println!(
            "--- Multi-threaded Benchmarks ({} threads) ---\n",
            num_threads
        );

        // SET (different keys per thread)
        let result = run_multi_threaded_benchmark("SET", num_threads, |thread_id| {
            let mut runner = BenchmarkRunner::new().expect("Failed to connect");
            let mut counter = 0u64;
            Box::new(move || {
                let key = format!("benchmark:t{}:{}", thread_id, counter);
                counter += 1;
                runner.set(&key, "value")
            })
        });
        result.print();
        report.add(result);

        // GET (shared key - read-heavy)
        {
            // Setup: create the key first
            let mut setup = BenchmarkRunner::new().expect("Failed to connect");
            setup.set("benchmark:shared", "sharedvalue").unwrap();
        }
        let result = run_multi_threaded_benchmark("GET (shared)", num_threads, |_| {
            let mut runner = BenchmarkRunner::new().expect("Failed to connect");
            Box::new(move || runner.get("benchmark:shared"))
        });
        result.print();
        report.add(result);

        // INCR (contended counter)
        {
            let mut setup = BenchmarkRunner::new().expect("Failed to connect");
            setup.set("benchmark:contended", "0").unwrap();
        }
        let result = run_multi_threaded_benchmark("INCR (contended)", num_threads, |_| {
            let mut runner = BenchmarkRunner::new().expect("Failed to connect");
            Box::new(move || runner.incr("benchmark:contended"))
        });
        result.print();
        report.add(result);

        // Mixed workload (80% GET, 20% SET)
        let result = run_multi_threaded_benchmark("Mixed 80/20", num_threads, |thread_id| {
            let mut runner = BenchmarkRunner::new().expect("Failed to connect");
            let mut rng = rand::rngs::SmallRng::from_entropy();
            let mut counter = 0u64;
            Box::new(move || {
                if rng.r#gen::<f32>() < 0.8 {
                    runner.get("benchmark:mixed")
                } else {
                    let key = format!("benchmark:mixed:{}:{}", thread_id, counter);
                    counter += 1;
                    runner.set(&key, "value")
                }
            })
        });
        result.print();
        report.add(result);

        println!();
    }

    // Print summary
    report.print_summary();

    // Save results to file
    let results_file = format!("benchmark_results_{}.csv", report.timestamp);
    match report.save_to_file(&results_file) {
        Ok(_) => println!("\nResults saved to: {}", results_file),
        Err(e) => eprintln!("\nFailed to save results: {}", e),
    }
}
