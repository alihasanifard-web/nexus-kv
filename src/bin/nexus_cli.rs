// ==============================================================================
// 💻 NEXUS-CLI: INTERACTIVE COMMAND-LINE CLIENT & REPL
// Architect: Ali Hasanifard
// ==============================================================================

use bytes::BytesMut;
use nexus_kv::proto::resp::{RespCodec, RespValue};
use std::env;
use std::io::{self, Write};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let host = get_arg_str(&args, "-h", "127.0.0.1");
    let port = get_arg_u16(&args, "-p", 6379);

    if args.iter().any(|a| a == "--cluster" || a == "-C") {
        show_cluster_topology().await;
        return Ok(());
    }

    println!("\n  \x1B[1;37mNexusKV CLI v0.1.0\x1B[0m \x1B[90m({}:{})\x1B[0m", host, port);
    println!("  \x1B[90mType 'help' for commands, 'cluster' for topology, 'exit' to quit.\x1B[0m\n");

    let mut stream = match TcpStream::connect(format!("{}:{}", host, port)).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  \x1B[31m✖ Could not connect to {}:{}: {}\x1B[0m\n", host, port, e);
            std::process::exit(1);
        }
    };

    let prompt = format!("\x1B[1;36mnexus:{}\x1B[0m> ", port);
    let mut read_buf = BytesMut::with_capacity(4096);
    let mut scratch = [0u8; 1024];

    loop {
        print!("{}", prompt);
        let _ = io::stdout().flush();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
            println!("  \x1B[33mGoodbye!\x1B[0m");
            break;
        }

        if trimmed.eq_ignore_ascii_case("clear") {
            print!("\x1B[2J\x1B[1;1H");
            let _ = io::stdout().flush();
            continue;
        }

        if trimmed.eq_ignore_ascii_case("help") || trimmed == "?" {
            print_help();
            continue;
        }

        if trimmed.eq_ignore_ascii_case("cluster") {
            show_cluster_topology().await;
            continue;
        }

        // Split arguments by whitespace respecting quotes
        let parts = parse_cmd_tokens(trimmed);
        if parts.is_empty() {
            continue;
        }

        // Encode as RESP array
        let resp_array = RespValue::Array(Some(
            parts.into_iter().map(|p| RespValue::BulkString(Some(p.into_bytes()))).collect(),
        ));
        let wire_bytes = RespCodec::encode(&resp_array);

        let start_time = Instant::now();
        if stream.write_all(&wire_bytes).await.is_err() {
            eprintln!("  \x1B[31mConnection lost.\x1B[0m");
            break;
        }

        // Read response
        let resp_value = loop {
            let n = stream.read(&mut scratch).await.unwrap_or(0);
            if n == 0 {
                break None;
            }
            read_buf.extend_from_slice(&scratch[..n]);
            if let Ok(Some(val)) = RespCodec::decode(&mut read_buf) {
                break Some(val);
            }
        };

        let elapsed = start_time.elapsed();
        let elapsed_str = if elapsed.as_millis() > 0 {
            format!("{} ms", elapsed.as_millis())
        } else {
            format!("{} µs", elapsed.as_micros())
        };

        match resp_value {
            Some(val) => print_resp(&val, &elapsed_str),
            None => {
                eprintln!("  \x1B[31mServer closed connection.\x1B[0m");
                break;
            }
        }
    }

    Ok(())
}

fn print_resp(val: &RespValue, elapsed: &str) {
    match val {
        RespValue::SimpleString(s) => {
            println!("  \x1B[32m{}\x1B[0m  \x1B[90m(elapsed: {})\x1B[0m", s, elapsed);
        }
        RespValue::Integer(n) => {
            println!("  \x1B[36m(integer) {}\x1B[0m  \x1B[90m(elapsed: {})\x1B[0m", n, elapsed);
        }
        RespValue::BulkString(Some(bytes)) => {
            let s = String::from_utf8_lossy(bytes);
            if s.contains('\n') {
                println!("{}", s);
            } else {
                println!("  \x1B[33m\"{}\"\x1B[0m  \x1B[90m(elapsed: {})\x1B[0m", s, elapsed);
            }
        }
        RespValue::BulkString(None) => {
            println!("  \x1B[90m(nil)\x1B[0m  \x1B[90m(elapsed: {})\x1B[0m", elapsed);
        }
        RespValue::Error(err) => {
            println!("  \x1B[31m(error) {}\x1B[0m  \x1B[90m(elapsed: {})\x1B[0m", err, elapsed);
        }
        RespValue::Array(Some(elements)) => {
            if elements.is_empty() {
                println!("  \x1B[90m(empty array)\x1B[0m  \x1B[90m(elapsed: {})\x1B[0m", elapsed);
            } else {
                println!("  \x1B[90m--- Results ({} items, {}) ---\x1B[0m", elements.len(), elapsed);
                for (i, elem) in elements.iter().enumerate() {
                    match elem {
                        RespValue::BulkString(Some(b)) => {
                            println!("  {:<3}) \x1B[33m\"{}\"\x1B[0m", i + 1, String::from_utf8_lossy(b));
                        }
                        other => {
                            println!("  {:<3}) {:?}", i + 1, other);
                        }
                    }
                }
            }
        }
        RespValue::Array(None) => {
            println!("  \x1B[90m(null array)\x1B[0m  \x1B[90m(elapsed: {})\x1B[0m", elapsed);
        }
    }
}

fn parse_cmd_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape = false;

    for ch in input.chars() {
        if escape {
            current.push(ch);
            escape = false;
        } else if ch == '\\' {
            escape = true;
        } else if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn print_help() {
    println!("\n  \x1B[1;36mNexusKV Supported Commands Reference:\x1B[0m");
    println!("  -------------------------------------------------------------");
    println!("  • \x1B[32mSET <key> <value> [EX sec]\x1B[0m Set key to hold string value with optional TTL");
    println!("  • \x1B[32mGET <key>\x1B[0m                 Get value of key");
    println!("  • \x1B[32mDEL <key>\x1B[0m                 Delete key (tombstone marker)");
    println!("  • \x1B[32mEXPIRE <key> <sec>\x1B[0m        Set key timeout in seconds");
    println!("  • \x1B[32mTTL <key>\x1B[0m                 Get remaining TTL (-2 expired, -1 persistent)");
    println!("  • \x1B[32mPERSIST <key>\x1B[0m             Remove key expiration");
    println!("  • \x1B[32mMULTI / EXEC / DISCARD\x1B[0m   Atomic command pipeline");
    println!("  • \x1B[32mKEYS <pattern>\x1B[0m            Find all keys matching wildcard (e.g. 'user:*')");
    println!("  • \x1B[32mSCAN <cursor> [pattern]\x1B[0m   Incrementally iterate database keyspace");
    println!("  • \x1B[32mROLE\x1B[0m                      Return replication role (master/slave)");
    println!("  • \x1B[32mMETRICS\x1B[0m                   Export live Prometheus instrumentation");
    println!("  • \x1B[32mCLUSTER\x1B[0m                   Inspect 3-node cluster topology & consensus");
    println!("  • \x1B[32mPING\x1B[0m                      Health check connection");
    println!("  • \x1B[32mclear\x1B[0m                     Clear terminal screen");
    println!("  • \x1B[32mexit\x1B[0m                      Exit interactive console\n");
}

async fn show_cluster_topology() {
    println!("\n  \x1B[1;37mNexusKV 3-Node Raft Cluster Topology & Health Inspector\x1B[0m");
    println!("  --------------------------------------------------------------------------------");
    println!("  \x1B[90m{:<8} {:<18} {:<12} {:<10} {:<10} {:<14}\x1B[0m", "NODE", "ENDPOINT", "STATUS", "ROLE", "TERM", "LATENCY");
    println!("  --------------------------------------------------------------------------------");

    let nodes = [
        (1, "127.0.0.1", 6379),
        (2, "127.0.0.1", 6380),
        (3, "127.0.0.1", 6381),
    ];

    let mut active_nodes = 0;
    for (id, host, port) in nodes {
        let start = std::time::Instant::now();
        match tokio::time::timeout(
            std::time::Duration::from_millis(600),
            TcpStream::connect(format!("{}:{}", host, port))
        ).await {
            Ok(Ok(mut s)) => {
                let elapsed_us = start.elapsed().as_micros();
                active_nodes += 1;
                let mut role = "master".to_string();
                let term = "1".to_string();

                // Query ROLE
                let _ = s.write_all(b"*1\r\n$4\r\nROLE\r\n").await;
                let mut buf = [0u8; 128];
                if let Ok(n) = s.read(&mut buf).await {
                    let res = String::from_utf8_lossy(&buf[..n]);
                    if res.contains("slave") {
                        role = "replica".to_string();
                    } else if res.contains("candidate") {
                        role = "candidate".to_string();
                    }
                }

                println!("  Node {:<3} {:<18} \x1B[32m{:<12}\x1B[0m \x1B[36m{:<10}\x1B[0m {:<10} {:>8} µs",
                    id, format!("{}:{}", host, port), "ONLINE", role.to_uppercase(), format!("Term {}", term), elapsed_us);
            }
            _ => {
                println!("  Node {:<3} {:<18} \x1B[31m{:<12}\x1B[0m {:<10} {:<10} {:>10}",
                    id, format!("{}:{}", host, port), "OFFLINE", "—", "—", "unreachable");
            }
        }
    }

    println!("  --------------------------------------------------------------------------------");
    let quorum_pct = (active_nodes as f64 / 3.0) * 100.0;
    let quorum_status = if active_nodes >= 2 { "\x1B[32m✔ QUORUM HEALTHY (Majority Active)\x1B[0m" } else { "\x1B[31m✖ QUORUM LOST (No Majority)\x1B[0m" };
    println!("  Consensus Agreement: {}/3 Nodes Active ({:.0}%) • {}\n", active_nodes, quorum_pct, quorum_status);
}

fn get_arg_str(args: &[String], flag: &str, default: &str) -> String {
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    default.to_string()
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
