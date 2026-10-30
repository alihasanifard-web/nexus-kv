// ==============================================================================
// 🌐 NEXUS-KV: RAFT TCP NETWORK TRANSPORT & CLUSTER TICKER
// Architect: Ali Hasanifard
// ==============================================================================

use crate::raft::message::*;
use crate::raft::node::RaftNode;
use crate::raft::state::NodeRole;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::sleep;

pub struct RaftTransport {
    pub node: Arc<RaftNode>,
    pub peer_addresses: Arc<HashMap<NodeId, String>>,
    #[allow(dead_code)]
    connections: Arc<Mutex<HashMap<NodeId, TcpStream>>>,
}

impl RaftTransport {
    pub fn new(node: Arc<RaftNode>, peer_addresses: HashMap<NodeId, String>) -> Self {
        Self {
            node,
            peer_addresses: Arc::new(peer_addresses),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Send a length-prefixed binary Raft message to a peer over TCP
    pub async fn send_message(&self, peer_id: NodeId, msg: &RaftMessage) -> std::io::Result<RaftMessage> {
        let addr = self.peer_addresses.get(&peer_id)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("Peer {} address unknown", peer_id)))?;

        let mut stream = TcpStream::connect(addr).await?;
        let payload = bincode::serialize(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let len_bytes = (payload.len() as u32).to_be_bytes();
        stream.write_all(&len_bytes).await?;
        stream.write_all(&payload).await?;
        stream.flush().await?;

        // Read response
        let mut resp_len_buf = [0u8; 4];
        stream.read_exact(&mut resp_len_buf).await?;
        let resp_len = u32::from_be_bytes(resp_len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf).await?;

        let reply: RaftMessage = bincode::deserialize(&resp_buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(reply)
    }

    /// Start inbound TCP server on raft_port for peer messages
    pub async fn start_listener(node: Arc<RaftNode>, listen_addr: SocketAddr) -> std::io::Result<()> {
        let listener = TcpListener::bind(listen_addr).await?;
        println!("  📡 Raft Inter-Node Listener active on {}", listen_addr);

        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let node_ref = node.clone();
                    tokio::spawn(async move {
                        let mut len_buf = [0u8; 4];
                        if stream.read_exact(&mut len_buf).await.is_err() {
                            return;
                        }
                        let len = u32::from_be_bytes(len_buf) as usize;
                        let mut buf = vec![0u8; len];
                        if stream.read_exact(&mut buf).await.is_err() {
                            return;
                        }

                        if let Ok(msg) = bincode::deserialize::<RaftMessage>(&buf) {
                            let reply_msg = match msg {
                                RaftMessage::RequestVote(args) => {
                                    let rep = node_ref.handle_request_vote(&args);
                                    RaftMessage::RequestVoteResponse(rep)
                                }
                                RaftMessage::AppendEntries(args) => {
                                    let rep = node_ref.handle_append_entries(&args);
                                    RaftMessage::AppendEntriesResponse(rep)
                                }
                                _ => return,
                            };

                            if let Ok(reply_bytes) = bincode::serialize(&reply_msg) {
                                let r_len = (reply_bytes.len() as u32).to_be_bytes();
                                let _ = stream.write_all(&r_len).await;
                                let _ = stream.write_all(&reply_bytes).await;
                                let _ = stream.flush().await;
                            }
                        }
                    });
                }
            }
        });

        Ok(())
    }

    /// Background Heartbeat / Election Ticker
    pub fn start_cluster_ticker(transport: Arc<RaftTransport>) {
        tokio::spawn(async move {
            loop {
                let role = transport.node.state.read().role;

                match role {
                    NodeRole::Leader => {
                        // Leader sends periodic Heartbeats to all peers
                        for &peer_id in &transport.node.peers {
                            let t = transport.clone();
                            let heartbeat = transport.node.create_heartbeat(peer_id);
                            let msg = RaftMessage::AppendEntries(heartbeat);

                            tokio::spawn(async move {
                                if let Ok(RaftMessage::AppendEntriesResponse(reply)) = t.send_message(peer_id, &msg).await {
                                    t.node.handle_peer_append_reply(peer_id, &reply);
                                }
                            });
                        }
                        sleep(Duration::from_millis(60)).await;
                    }
                    NodeRole::Follower | NodeRole::Candidate => {
                        // Random timeout between 200ms and 350ms to trigger election if quiet
                        let timeout = 200 + rand::random::<u64>() % 150;
                        sleep(Duration::from_millis(timeout)).await;

                        // Check if we should trigger election
                        let (vote_args, mut votes_won) = transport.node.start_election();
                        let total_nodes = transport.node.peers.len() + 1;
                        let majority = total_nodes / 2 + 1;

                        let msg = RaftMessage::RequestVote(vote_args);
                        for &peer_id in &transport.node.peers {
                            let t = transport.clone();
                            let req = msg.clone();
                            if let Ok(RaftMessage::RequestVoteResponse(reply)) = t.send_message(peer_id, &req).await {
                                if reply.vote_granted {
                                    votes_won += 1;
                                    if votes_won >= majority && t.node.state.read().role == NodeRole::Candidate {
                                        t.node.become_leader();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
