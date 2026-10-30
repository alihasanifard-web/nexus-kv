// ==============================================================================
// 🏎️ NEXUS-BENCH: HIGH-THROUGHPUT LATENCY & STRESS BENCHMARK SUITE
// Architect: Ali Hasanifard
// ==============================================================================

use bytes::BytesMut;
use nexus_kv::proto::resp::RespCodec;
use std::env;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let port = get_arg_u16(&args, "-p", 6379);
    let total_requests = get_arg_usize(&args, "-n", 10_000);
    let concurrency = get_arg_usize(&args, "-c", 50);

    println!("\n  \x1B[1;37mNexusKV Benchmark Suite v0.1.0\x1B[0m \x1B[90m(x86_64-pc-windows-gnu)\x1B[0m");
    println!("  \x1B[90mMulti-Threaded Async Load & Latency Distribution Analyzer\x1B[0m");
    println!("  \x1B[90m--------------------------------------------------------\x1B[0m");
    println!("  • Target:        \x1B[36m127.0.0.1:{}\x1B[0m \x1B[90m(Redis RESP)\x1B[0m", port);
    println!("  • Requests:      \x1B[36m{}\x1B[0m", to_formatted_number(total_requests));
    println!("  • Concurrency:   \x1B[36m{} async workers\x1B[0m", concurrency);
    println!("  • Workload:      \x1B[37mSequential Pipeline SET & Verify\x1B[0m");
    println!("  \x1B[90m--------------------------------------------------------\x1B[0m\n");

    let completed = Arc::new(AtomicUsize::new(0));
    let latencies = Arc::new(parking_lot::Mutex::new(Vec::with_capacity(total_requests)));
    let is_done = Arc::new(AtomicBool::new(false));

    let reqs_per_worker = total_requests / concurrency;
    let start_time = Instant::now();

    // Spawn live terminal progress monitor
    let comp_mon = completed.clone();
    let done_mon = is_done.clone();
    let monitor_handle = tokio::spawn(async move {
        let bar_width = 28usize;
        while !done_mon.load(Ordering::Relaxed) {
            let done = comp_mon.load(Ordering::Relaxed);
            let pct = (done as f64 / total_requests as f64).min(1.0);
            let filled = (pct * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(filled);
            let elapsed = start_time.elapsed().as_secs_f64();
            let qps = if elapsed > 0.0 { (done as f64) / elapsed } else { 0.0 };

            print!("\r  \x1B[36m[\x1B[32m{}\x1B[90m{}\x1B[36m]\x1B[0m {:>3.0}% | \x1B[1m{}/{} reqs\x1B[0m | \x1B[33m{:>8.0} QPS\x1B[0m",
                "█".repeat(filled),
                "░".repeat(empty),
                pct * 100.0,
                done,
                total_requests,
                qps
            );
            let _ = stdout().flush();
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    });

    let mut handles = Vec::new();
    for worker_id in 0..concurrency {
        let completed_clone = completed.clone();
        let latencies_clone = latencies.clone();

        let handle = tokio::spawn(async move {
            let mut stream = match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("\n  \x1B[31mWorker {} failed to connect: {}\x1B[0m", worker_id, e);
                    return;
                }
            };

            let mut read_buf = BytesMut::with_capacity(1024);
            let mut raw_scratch = [0u8; 512];

            for i in 0..reqs_per_worker {
                let key = format!("user:{:06}", worker_id * 10000 + i);
                let val = format!("bench_payload_{}", i);

                let cmd = format!("*3\r\n$3\r\nSET\r\n${}\r\n{}\r\n${}\r\n{}\r\n", key.len(), key, val.len(), val);

                let req_start = Instant::now();
                if stream.write_all(cmd.as_bytes()).await.is_err() {
                    break;
                }

                loop {
                    let n = stream.read(&mut raw_scratch).await.unwrap_or(0);
                    if n == 0 { break; }
                    read_buf.extend_from_slice(&raw_scratch[..n]);
                    if RespCodec::decode(&mut read_buf).unwrap_or(None).is_some() {
                        break;
                    }
                }

                let elapsed_us = req_start.elapsed().as_micros() as u64;
                latencies_clone.lock().push(elapsed_us);
                completed_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }

    is_done.store(true, Ordering::Relaxed);
    let _ = monitor_handle.await;

    let total_duration = start_time.elapsed();
    let total_done = completed.load(Ordering::Relaxed);
    let rps = (total_done as f64) / total_duration.as_secs_f64();

    let mut lats = latencies.lock();
    lats.sort_unstable();

    println!("\r  \x1B[36m[\x1B[32m{}\x1B[36m]\x1B[0m 100% | \x1B[1m{}/{} reqs\x1B[0m | \x1B[33m{:>8.0} QPS\x1B[0m\n",
        "█".repeat(28),
        total_done,
        total_requests,
        rps
    );

    println!("  \x1B[1;37mExecution Summary:\x1B[0m");
    println!("  --------------------------------------------------------");
    println!("  • Time Elapsed:       {:>12.2?}", total_duration);
    println!("  • Total Ingested:     {:>12} reqs", total_done);
    println!("  • Throughput Rate:    \x1B[1;32m{:>12.2} req/sec (QPS)\x1B[0m", rps);
    println!("  --------------------------------------------------------");

    if !lats.is_empty() {
        let p50 = lats[lats.len() * 50 / 100];
        let p90 = lats[lats.len() * 90 / 100];
        let p95 = lats[lats.len() * 95 / 100];
        let p99 = lats[lats.len() * 99 / 100];
        let p999 = lats[lats.len() * 999 / 1000];

        println!("  \x1B[1;37mLatency Percentiles:\x1B[0m");
        println!("  • P50  (Median):      \x1B[36m{:>10} µs\x1B[0m", p50);
        println!("  • P90:                \x1B[36m{:>10} µs\x1B[0m", p90);
        println!("  • P95:                \x1B[36m{:>10} µs\x1B[0m", p95);
        println!("  • P99  (Tail):        \x1B[33m{:>10} µs\x1B[0m  ({:.2} ms)", p99, p99 as f64 / 1000.0);
        println!("  • P99.9:              \x1B[33m{:>10} µs\x1B[0m", p999);
        println!("  • Min / Max:          {:>10} µs / {} µs", lats[0], lats.last().unwrap());
        println!("  --------------------------------------------------------");

        render_histogram(&lats);
    }
    println!();
    Ok(())
}

fn render_histogram(lats: &[u64]) {
    println!("  \x1B[1;37mEmpirical Latency Histogram:\x1B[0m");
    let total = lats.len() as f64;
    let buckets = [
        ("< 300 µs", 300),
        ("300-600 µs", 600),
        ("600-1000 µs", 1000),
        ("1.0-2.0 ms", 2000),
        ("> 2.0 ms", u64::MAX),
    ];

    let mut prev_threshold = 0;
    for (label, threshold) in buckets {
        let count = lats.iter().filter(|&&v| v >= prev_threshold && v < threshold).count();
        let pct = (count as f64 / total) * 100.0;
        let bar_len = (pct * 0.25) as usize;
        let bar = "█".repeat(bar_len);
        let pad = "░".repeat(25usize.saturating_sub(bar_len));
        println!("  • {:<12} \x1B[32m{}\x1B[90m{}\x1B[0m {:>5.1}% ({})", label, bar, pad, pct, count);
        prev_threshold = threshold;
    }
}

fn to_formatted_number(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            out.push(',');
        }
        out.push(c);
        count += 1;
    }
    out.chars().rev().collect()
}

fn get_arg_usize(args: &[String], flag: &str, default: usize) -> usize {
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() {
            if let Ok(val) = args[i + 1].parse() {
                return val;
            }
        }
    }
    default
}

fn get_arg_u16(args: &[String], flag: &str, default: u16) -> u16 {
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() {
            if let Ok(val) = args[i + 1].parse() {
                return val;
            }
        }
    }
    default
}
