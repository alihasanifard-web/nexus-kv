# ⚡ NexusKV — Distributed Raft & LSM-Tree Storage Engine

```
    ███╗   ██╗███████╗██╗   ██╗██╗   ██╗███████╗██╗  ██╗██╗   ██╗
    ████╗  ██║██╔════╝╚██╗ ██╔╝██║   ██║██╔════╝██║ ██╔╝██║   ██║
    ██╔██╗ ██║█████╗   ╚████╔╝ ██║   ██║███████╗█████╔╝ ██║   ██║
    ██║╚██╗██║██╔══╝    ╚██╔╝  ██║   ██║╚════██║██╔═██╗ ╚██╗ ██╔╝
    ██║ ╚████║███████╗   ██║   ╚██████╔╝███████║██║  ██╗ ╚████╔╝ 
    ╚═╝  ╚═══╝╚══════╝   ╚═╝    ╚═════╝ ╚══════╝╚═╝  ╚═╝  ╚═══╝  
```

> **A high-performance, fault-tolerant distributed Key-Value storage engine written in Rust.**  
> Powered by a custom **Log-Structured Merge-Tree (LSM-Tree)**, **Raft Consensus Protocol**, and the standard **Redis RESP Wire Protocol**.

---

## 👨‍💻 Author & Lead Systems Architect
- **Ali Hasanifard (علی حسنی فرد)** — [GitHub](https://github.com/alihasanifard-web) • [Email](mailto:hasanifardalipa@gmail.com)

---

## 🌟 Key Architecture & Highlights

```
                               +-----------------------------+
                               |     Redis / RESP Client     |
                               +--------------+--------------+
                                              |
                                     (RESP Protocol)
                                              |
                                              v
                               +-----------------------------+
                               |    Tokio Async TCP Server   |
                               +--------------+--------------+
                                              |
                     +------------------------+------------------------+
                     |                                                 |
                     v                                                 v
      +-----------------------------+                   +-----------------------------+
      |    Distributed Raft Core    |                   |    Local LSM Storage Engine |
      |                             |                   |                             |
      |  • Quorum Replication       |                   |  • Thread-Safe MemTable     |
      |  • Term & Election Timers   |                   |  • Write-Ahead Log (WAL)    |
      |  • Heartbeats & Log Sync    |                   |  • Immutable SSTables       |
      |  • Linearizable Consistency |                   |  • Optimal Bloom Filters    |
      +-----------------------------+                   |  • Compaction & GC Engine   |
                                                        +-----------------------------+
```

### 1. Log-Structured Merge-Tree (LSM-Tree) Engine
- **In-Memory MemTable:** High-throughput sorted key-value store using `parking_lot::RwLock` for concurrent reads without global lock contention.
- **Write-Ahead Log (WAL):** Sequential disk write with **CRC32 data integrity validation** to survive sudden power failures and machine crashes with zero data loss.
- **Sorted String Tables (SSTables):** Immutable disk-backed files with sparse block indexes for fast binary search without full table scans.
- **Embedded Bloom Filters:** Double-hashing probabilistic filter ($O(1)$) skipping unnecessary disk I/O when a key does not exist.
- **Compaction Engine:** Background consolidation merging multiple SSTables, resolving conflicting versions, and purging deleted tombstones to reclaim disk space.

### 2. Distributed Raft Consensus
- Full implementation of the **Raft consensus algorithm** (Leader, Candidate, Follower).
- Leader election with randomized heartbeat timers to prevent split votes.
- Quorum-based log replication across multiple cluster nodes with automatic conflict resolution.
- Leader failover: cluster automatically elects a new leader and resumes operations if the primary node goes offline.

### 3. Redis RESP Wire Protocol Compatibility
- Direct out-of-the-box compatibility with existing tools (`redis-cli`, Python `redis-py`, Node.js `ioredis`, Go `go-redis`):
  - `PING` / `PONG`
  - `SET key value`
  - `GET key`
  - `DEL key`
  - `KEYS pattern` (Wildcard & prefix matching)
  - `SCAN cursor [MATCH pattern]` (Incremental iteration)
  - `ROLE`
  - `METRICS` (Live Prometheus metrics exporter)
  - `FLUSHALL`

### 4. Distributed ACID Transactions (2PC & Percolator Model)
- Hybrid Logical Clock (HLC) timestamps for linearizable ordering.
- Two-Phase Commit with Prewrite, Commit, and Rollback stages.
- Optimistic Concurrency Control (OCC) preventing write-write conflicts across transactions.

### 5. Multi-Raft & Dynamic Sharding
- Region-based key routing with automatic region splitting as data scales.
- Scales horizontally across multiple Raft groups.

---

## 📊 Benchmark & Latency Percentiles (Tested on NVMe SSD)

```
===============================================================
🏎️  NEXUS-BENCH: DISTRIBUTED STORAGE LOAD & LATENCY GENERATOR  
===============================================================
• Workload: Sequential SET & Random GET (50 concurrent workers)
• Operations Completed:   100,000 reqs
• Throughput Rate:        102,100 req/sec (QPS)
---------------------------------------------------------------
• P50  Latency (Median):    340 µs
• P90  Latency:             780 µs
• P95  Latency:             960 µs
• P99  Latency:           1,240 µs
• P99.9 Latency:          2,100 µs
===============================================================
```

---

## 🚀 Quick Start

### Build from Source
```bash
# Clone the repository
git clone https://github.com/alihasanifard-web/nexus-kv.git
cd nexus-kv

# Build optimized binary
cargo build --release

# Run unit and integration tests
cargo test
```

### Start Standalone Node
```bash
# Run server on port 6379 (default)
./target/release/nexus-kv 6379 1 ./data/node_1
```

### Run High-Throughput Benchmark
```bash
# Run benchmark tool against running server
cargo run --release --bin nexus_bench -- -p 6379 -n 100000 -c 50
```

### Run Chaos & Failover Resilience Test
```bash
# Automated chaos test: writes data, kills process with SIGKILL, and verifies 100% recovery
python scripts/chaos_test.py
```

### Kubernetes & Docker Deployment
```bash
# Build minimal Docker image (< 20MB)
docker build -t alihasanifard/nexus-kv:latest .

# Deploy 3-node HA Raft cluster to Kubernetes
kubectl apply -f deploy/kubernetes.yaml
```

---

## 📑 Technical Whitepaper
For an in-depth exploration of the LSM-Tree disk layout, Raft invariants, and concurrency architecture, read our [Engineering Whitepaper](docs/WHITEPAPER.md).

### Connect via any Redis Client
```bash
redis-cli -p 6379
127.0.0.1:6379> PING
PONG
127.0.0.1:6379> SET user:101 "Ali Hasanifard"
OK
127.0.0.1:6379> GET user:101
"Ali Hasanifard"
127.0.0.1:6379> DEL user:101
(integer) 1
127.0.0.1:6379> GET user:101
(nil)
```

---

## 🧪 Rigorous Automated Test Suite
NexusKV is developed using **Test-Driven Development (TDD)** with 100% green coverage:
- `test_memtable_crud`: Verifies put, get, delete, size tracking, and drain sorted.
- `test_bloom_filter_accuracy`: Tests false positive rates and guarantees 0% false negatives.
- `test_wal_append_and_recovery`: Simulates crash and verifies CRC32 integrity check.
- `test_sstable_lifecycle`: Verifies binary search sparse index and disk deserialization.
- `test_compaction_merges_and_purges`: Multi-table merge and tombstone garbage collection.
- `test_raft_election_and_quorum_replication`: Verifies vote counts, leader promotion, and term safety.
- `test_distributed_raft_leader_failover_simulation`: Simulates total leader crash and automated cluster recovery.
- `test_crash_consistency_with_wal_replay`: Verifies cold-start crash consistency.
- `test_e2e_stress_write_and_compaction`: Heavy write workloads and end-to-end verification.

---

## 📄 License
This project is licensed under the **MIT License**.
