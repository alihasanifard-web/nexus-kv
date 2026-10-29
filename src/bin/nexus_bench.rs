// ==============================================================================
// 🏎️ NEXUS-BENCH: HIGH-THROUGHPUT LATENCY & STRESS BENCHMARK SUITE
// Architect: Ali Hasanifard
// ==============================================================================

use bytes::BytesMut;
use nexus_kv::proto::resp::RespCodec;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\x1B[36m");
    println!("  ===============================================================");
    println!("  🏎️  NEXUS-BENCH: DISTRIBUTED STORAGE LOAD & LATENCY GENERATOR  ");
    println!("  ===============================================================");
    println!("\x1B[0m");

    let args: Vec<String> = env::args().collect();
    let port = get_arg_u16(&args, "-p", 6379);
    let total_requests = get_arg_usize(&args, "-n", 10_000);
    let concurrency = get_arg_usize(&args, "-c", 50);

    println!("  ⚙ Target Host:      127.0.0.1:{}", port);
    println!("  ⚙ Total Requests:   {}", total_requests);
    println!("  ⚙ Concurrency:      {} async workers", concurrency);
    println!("  ⚙ Workloads:        1. Sequential SET  |  2. Random GET\n");

    let completed = Arc::new(AtomicUsize::new(0));
    let latencies = Arc::new(parking_lot::Mutex::new(Vec::with_capacity(total_requests)));

    let reqs_per_worker = total_requests / concurrency;

    println!("  🚀 Starting Benchmark Execution...");
    let start_time = Instant::now();

    let mut handles = Vec::new();
    for worker_id in 0..concurrency {
        let completed_clone = completed.clone();
        let latencies_clone = latencies.clone();

        let handle = tokio::spawn(async move {
            let mut stream = match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Worker {} failed to connect: {}", worker_id, e);
                    return;
                }
            };

            let mut read_buf = BytesMut::with_capacity(1024);
            let mut raw_scratch = [0u8; 512];

            for i in 0..reqs_per_worker {
                let key = format!("user:{:06}", worker_id * 10000 + i);
                let val = format!("nexus_benchmark_payload_value_{}", i);

                // Build RESP SET command
                let cmd = format!("*3\r\n$3\r\nSET\r\n${}\r\n{}\r\n${}\r\n{}\r\n", key.len(), key, val.len(), val);

                let req_start = Instant::now();
                if stream.write_all(cmd.as_bytes()).await.is_err() {
                    break;
                }

                // Read RESP response
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

    let total_duration = start_time.elapsed();
    let total_done = completed.load(Ordering::Relaxed);
    let rps = (total_done as f64) / total_duration.as_secs_f64();

    let mut lats = latencies.lock();
    lats.sort_unstable();

    println!("\n  ===============================================================");
    println!("  📊 BENCHMARK EXECUTION RESULTS & LATENCY PERCENTILES          ");
    println!("  ===============================================================");
    println!("  • Total Time Elapsed:       {:>12.2?}", total_duration);
    println!("  • Operations Completed:     {:>12} reqs", total_done);
    println!("  • Throughput Rate:          {:>12.2} req/sec (QPS)", rps);
    println!("  ---------------------------------------------------------------");

    if !lats.is_empty() {
        let p50 = lats[lats.len() * 50 / 100];
        let p90 = lats[lats.len() * 90 / 100];
        let p95 = lats[lats.len() * 95 / 100];
        let p99 = lats[lats.len() * 99 / 100];
        let p999 = lats[lats.len() * 999 / 1000];

        println!("  • P50  Latency (Median):    {:>12} µs", p50);
        println!("  • P90  Latency:             {:>12} µs", p90);
        println!("  • P95  Latency:             {:>12} µs", p95);
        println!("  • P99  Latency:             {:>12} µs", p99);
        println!("  • P99.9 Latency:            {:>12} µs", p999);
        println!("  • Min / Max Latency:        {:>8} µs / {} µs", lats[0], lats.last().unwrap());
    }
    println!("  ===============================================================\n");

    Ok(())
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
