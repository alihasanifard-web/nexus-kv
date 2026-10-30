// ==============================================================================
// 🚀 NEXUS-KV: SERVER MAIN ENTRYPOINT
// Architect: Ali Hasanifard
// ==============================================================================

use bytes::BytesMut;
use nexus_kv::engine::{ClientSession, NexusEngine};
use nexus_kv::lsm::LsmConfig;
use nexus_kv::proto::resp::{RespCodec, RespValue};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(6379);
    let node_id: u64 = args.get(2).and_then(|id| id.parse().ok()).unwrap_or(1);
    let data_dir = args.get(3).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("./data/node_{}", node_id)));
    let telemetry_port = port + 1000;

    println!("\n  \x1B[1;37mNexusKV Server v0.1.0\x1B[0m \x1B[90m(x86_64-pc-windows-gnu)\x1B[0m");
    println!("  \x1B[90mDistributed Raft Consensus & LSM-Tree Engine\x1B[0m");
    println!("  \x1B[90m--------------------------------------------\x1B[0m");
    println!("  • Node ID:    \x1B[36m{}\x1B[0m", node_id);
    println!("  • Listen:     \x1B[36m0.0.0.0:{}\x1B[0m \x1B[90m(Redis RESP)\x1B[0m", port);
    println!("  • Telemetry:  \x1B[36m0.0.0.0:{}\x1B[0m \x1B[90m(HTTP REST / SSE)\x1B[0m", telemetry_port);
    println!("  • Storage:    \x1B[36m{}\x1B[0m", data_dir.display());
    println!("  • Architect:  \x1B[37mAli Hasanifard\x1B[0m");
    println!("  \x1B[90m--------------------------------------------\x1B[0m");

    let lsm_config = LsmConfig {
        dir: data_dir,
        memtable_threshold_bytes: 4 * 1024 * 1024,
        compaction_threshold_ssts: 4,
    };

    let engine = Arc::new(NexusEngine::new(node_id, vec![], lsm_config)?);
    // In single node mode, become leader immediately
    engine.raft.become_leader();

    // Spawn Realtime Telemetry HTTP & SSE Server on telemetry_port
    let telem_engine = engine.clone();
    tokio::spawn(async move {
        let _ = run_telemetry_http_server(telemetry_port, telem_engine).await;
    });

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("  \x1B[32m✔ Ready to accept client connections\x1B[0m\n");

    loop {
        let (socket, peer_addr) = listener.accept().await?;
        let engine_clone = engine.clone();

        tokio::spawn(async move {
            handle_client(socket, peer_addr, engine_clone).await;
        });
    }
}

async fn handle_client(mut socket: TcpStream, _addr: std::net::SocketAddr, engine: Arc<NexusEngine>) {
    let mut buf = BytesMut::with_capacity(4096);
    let mut session = ClientSession::default();

    loop {
        let mut read_buf = [0u8; 1024];
        let n = match socket.read(&mut read_buf).await {
            Ok(0) => break, // Client disconnected
            Ok(n) => n,
            Err(_) => break,
        };

        buf.extend_from_slice(&read_buf[..n]);

        while !buf.is_empty() {
            match RespCodec::decode(&mut buf) {
                Ok(Some(RespValue::Array(Some(cmd_parts)))) => {
                    let response = engine.execute_with_session(&mut session, &cmd_parts);
                    let wire_bytes = RespCodec::encode(&response);
                    if socket.write_all(&wire_bytes).await.is_err() {
                        return;
                    }
                }
                Ok(Some(other)) => {
                    let response = engine.execute_with_session(&mut session, &[other]);
                    let wire_bytes = RespCodec::encode(&response);
                    if socket.write_all(&wire_bytes).await.is_err() {
                        return;
                    }
                }
                Ok(None) => break, // Incomplete frame, wait for more data
                Err(e) => {
                    let err_res = RespValue::Error(format!("Protocol error: {}", e));
                    let _ = socket.write_all(&RespCodec::encode(&err_res)).await;
                    return;
                }
            }
        }
    }
}

async fn run_telemetry_http_server(port: u16, engine: Arc<NexusEngine>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    loop {
        let (mut socket, _) = listener.accept().await?;
        let engine = engine.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            let n = match socket.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };
            let req_str = String::from_utf8_lossy(&buf[..n]);
            let first_line = req_str.lines().next().unwrap_or("");
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.is_empty() { return; }
            let method = parts[0];
            let path = parts.get(1).copied().unwrap_or("/");

            // Handle CORS OPTIONS preflight
            if method == "OPTIONS" {
                let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
                return;
            }

            if path == "/api/events" || path == "/events" {
                // Server-Sent Events (SSE) Stream
                let header = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: keep-alive\r\n\r\n";
                if socket.write_all(header.as_bytes()).await.is_err() {
                    return;
                }
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let telem = get_telemetry_json(&engine);
                    let event_line = format!("data: {}\n\n", telem);
                    if socket.write_all(event_line.as_bytes()).await.is_err() {
                        break;
                    }
                }
            } else if path.starts_with("/api/telemetry") || path == "/metrics.json" || path == "/health" {
                let json_data = get_telemetry_json(&engine);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                    json_data.len(),
                    json_data
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            } else if method == "POST" && (path == "/api/cmd" || path == "/cmd") {
                let body = if let Some(idx) = req_str.find("\r\n\r\n") {
                    &req_str[idx + 4..]
                } else if let Some(idx) = req_str.find("\n\n") {
                    &req_str[idx + 2..]
                } else {
                    ""
                };
                let raw_cmd = body.trim();
                let parts: Vec<&str> = raw_cmd.split_whitespace().collect();
                let resp_parts: Vec<RespValue> = parts.into_iter().map(|s| RespValue::BulkString(Some(s.as_bytes().to_vec()))).collect();
                let result = engine.execute_command(&resp_parts);
                let result_str = match result {
                    RespValue::SimpleString(s) => s,
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(&b).to_string(),
                    RespValue::BulkString(None) => "(nil)".to_string(),
                    RespValue::Integer(i) => i.to_string(),
                    RespValue::Error(e) => format!("ERR: {}", e),
                    RespValue::Array(Some(arr)) => format!("Array({} items)", arr.len()),
                    _ => "OK".to_string(),
                };
                let res_json = serde_json::json!({
                    "status": "ok",
                    "command": raw_cmd,
                    "result": result_str
                }).to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                    res_json.len(),
                    res_json
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            } else {
                let not_found = "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 9\r\n\r\nNot Found";
                let _ = socket.write_all(not_found.as_bytes()).await;
            }
        });
    }
}

fn get_telemetry_json(engine: &NexusEngine) -> String {
    let role = engine.raft.state.read().role;
    let role_str = match role {
        nexus_kv::raft::state::NodeRole::Leader => "master",
        nexus_kv::raft::state::NodeRole::Follower => "slave",
        nexus_kv::raft::state::NodeRole::Candidate => "candidate",
    };
    let term = engine.raft.state.read().current_term;
    let commit_idx = engine.raft.state.read().commit_index;
    let mem_bytes = engine.storage.memtable.size();
    let sst_count = engine.storage.sstables.read().len();
    let total_ops = nexus_kv::metrics::Metrics::total_ops();
    let write_ops = nexus_kv::metrics::Metrics::write_ops();
    let read_ops = nexus_kv::metrics::Metrics::read_ops();

    serde_json::json!({
        "status": "online",
        "node_id": engine.raft.id,
        "role": role_str,
        "term": term,
        "commit_index": commit_idx,
        "memtable_bytes": mem_bytes,
        "sstable_count": sst_count,
        "ops_total": total_ops,
        "write_ops": write_ops,
        "read_ops": read_ops,
        "peers_count": engine.raft.peers.len() + 1
    }).to_string()
}
