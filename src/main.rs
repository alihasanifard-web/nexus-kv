// ==============================================================================
// 🚀 NEXUS-KV: SERVER MAIN ENTRYPOINT
// Architect: Ali Hasanifard
// ==============================================================================

use bytes::BytesMut;
use nexus_kv::engine::NexusEngine;
use nexus_kv::lsm::LsmConfig;
use nexus_kv::proto::resp::{RespCodec, RespValue};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_banner();

    let args: Vec<String> = env::args().collect();
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(6379);
    let node_id: u64 = args.get(2).and_then(|id| id.parse().ok()).unwrap_or(1);
    let data_dir = args.get(3).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("./data/node_{}", node_id)));

    println!("  ⚙ Node ID:      {}", node_id);
    println!("  ⚙ Listen Port:  {}", port);
    println!("  ⚙ Data Path:    {}", data_dir.display());
    println!("  ⚙ Protocol:     Redis RESP v2/v3 Compatible");
    println!("  ⚙ Storage:      LSM-Tree with Bloom Filters & WAL");
    println!("  ⚙ Consensus:    Distributed Raft Protocol\n");

    let lsm_config = LsmConfig {
        dir: data_dir,
        memtable_threshold_bytes: 4 * 1024 * 1024,
        compaction_threshold_ssts: 4,
    };

    let engine = Arc::new(NexusEngine::new(node_id, vec![], lsm_config)?);
    // In single node mode, become leader immediately
    engine.raft.become_leader();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("  ✔ NexusKV Server running on 0.0.0.0:{}", port);
    println!("  ✔ Ready to accept Redis clients (redis-cli -p {})\n", port);

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
                    let response = engine.execute_command(&cmd_parts);
                    let wire_bytes = RespCodec::encode(&response);
                    if socket.write_all(&wire_bytes).await.is_err() {
                        return;
                    }
                }
                Ok(Some(other)) => {
                    let response = engine.execute_command(&[other]);
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

fn print_banner() {
    println!("\x1B[36m");
    println!(r#"
    ███╗   ██╗███████╗██╗  ██╗██╗   ██╗███████╗██╗  ██╗██╗   ██╗
    ████╗  ██║██╔════╝╚██╗██╔╝██║   ██║██╔════╝██║ ██╔╝██║   ██║
    ██╔██╗ ██║█████╗   ╚███╔╝ ██║   ██║███████╗█████╔╝ ██║   ██║
    ██║╚██╗██║██╔══╝   ██╔██╗ ██║   ██║╚════██║██╔═██╗ ╚██╗ ██╔╝
    ██║ ╚████║███████╗██╔╝ ██╗╚██████╔╝███████║██║  ██╗ ╚████╔╝ 
    ╚═╝  ╚═══╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝  ╚═══╝  
    "#);
    println!("\x1B[0m");
    println!("  High-Performance Distributed Raft Key-Value Storage Engine");
    println!("  Architect: Ali Hasanifard (علی حسنی فرد) | Open Source MIT");
    println!("  -------------------------------------------------------------");
}
