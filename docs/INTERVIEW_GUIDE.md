# 🎓 NexusKV — Senior Systems Design Interview & Architecture Defense Guide
**Candidate / Systems Architect:** Ali Hasanifard (علی حسنی فرد)  
**Project Repository:** [github.com/alihasanifard-web/nexus-kv](https://github.com/alihasanifard-web/nexus-kv)  
**Target Roles:** Staff / Senior Distributed Systems Engineer, Storage Engine Architect, High-Performance Backend Engineer

---

## 🏛️ Executive Architecture Summary

NexusKV is a distributed, fault-tolerant, linearizable key-value storage engine engineered from scratch in Rust. It marries the **Log-Structured Merge-Tree (LSM-Tree)** storage paradigm with the **Raft Distributed Consensus Protocol**, fronted by an asynchronous **Redis RESP v2/v3** wire server.

```
                    +------------------------------------+
                    |        Client (RESP Protocol)      |
                    +-----------------+------------------+
                                      |
                           [TCP Socket: Port 6379]
                                      |
                                      v
                    +------------------------------------+
                    |        Tokio Async Server          |
                    +-----------------+------------------+
                                      |
                     +----------------+----------------+
                     |                                 |
                     v                                 v
      +-----------------------------+   +-----------------------------+
      |    Raft Consensus Engine    |   |      LSM Storage Engine     |
      |                             |   |                             |
      |  • Quorum Majority (N/2 + 1)|   |  • Concurrent MemTable (RAM)|
      |  • Term & Randomized Timers |   |  • CRC32 Write-Ahead Log    |
      |  • Log Matching Invariant   |   |  • Immutable SSTables       |
      |  • Snapshots & Compaction   |   |  • Double-Hash Bloom Filter |
      +-----------------------------+   +-----------------------------+
```

---

## 🔬 Core System Deep-Dives & Technical Invariants

### 1. The Raft Consensus Protocol (Formal Safety)
* **Election Safety:** At most one leader can be elected in a given term. Ensured via randomized election timeouts ($150\text{--}300\text{ ms}$) and strict majority voter quorums ($Q = \lfloor N/2 \rfloor + 1$).
* **Leader Append-Only:** A leader never overwrites or truncates its own log entries; it only appends new entries.
* **Log Matching Property:** If two logs contain an entry with the same index and term, they are identical up to that index. Enforced during `AppendEntries` via `prev_log_index` and `prev_log_term` checks.
* **Leader Completeness:** If a log entry is committed in a given term, that entry will be present in the logs of the leaders for all higher-numbered terms. Candidates whose logs are less up-to-date than the voter's log are denied votes (`last_log_term` and `last_log_index` comparison).
* **State Machine Safety:** If a server has applied a log entry at a given index to its state machine, no other server will ever apply a different log entry for the same index.

---

### 2. Log Compaction & Snapshotting (Raft §7)
* **Problem:** In an active database, the Raft log cannot grow unbounded without exhausting disk storage and causing cold restarts to take hours replaying old logs.
* **NexusKV Solution:**
  1. Once the log index reaches an operational threshold, the engine serializes a snapshot of the current LSM state machine.
  2. The snapshot records `last_included_index` and `last_included_term`.
  3. All log entries up to `last_included_index` are discarded (`compact_up_to`).
  4. If a lagging follower requires an entry that the leader has already discarded (`prev_log_index < leader.last_included_index`), the leader streams an `InstallSnapshot` RPC.
  5. The follower restores its state machine, compacts its own log, and advances its commit index to `last_included_index`.

---

### 3. Log-Structured Merge-Tree (LSM-Tree) Storage Layout

| Component | Medium | Concurrency Mechanism | Crash Recovery Role |
| :--- | :--- | :--- | :--- |
| **MemTable** | RAM | `parking_lot::RwLock<BTreeMap>` | Holds mutable uncommitted writes; drained during flushes |
| **Write-Ahead Log (WAL)** | Disk (Append-only) | Sequential framing with CRC32 | Guarantees durable zero-loss replay on machine power loss |
| **SSTables (Level 0..N)** | Disk (Read-only) | Binary sparse index | Immutable blocks; indexed by binary search |
| **Bloom Filter** | Disk & RAM | Kirsch-Mitzenmacher double hashing | Bypasses unnecessary disk I/O when key does not exist ($O(1)$) |

#### Amplification Trade-Offs (The RUM Conjecture):
* **Write Amplification (WA):** $\frac{\text{Bytes Written to Storage}}{\text{Bytes Ingested by User}}$. NexusKV maintains low WA by converting random user writes into sequential append-only disk I/O (WAL & SSTable flushes).
* **Read Amplification (RA):** In worst-case point lookups, multiple SSTables must be checked. NexusKV mitigates RA using Bloom filters ($1\%\text{ false-positive rate}$) and in-memory sparse key indexes.
* **Space Amplification (SA):** $\frac{\text{Disk Footprint}}{\text{Live Data Size}}$. Kept under $1.4\times$ via background multi-way SSTable compaction and tombstone garbage collection.

---

## 🎯 Hard Interview Q&A Defense Script

### Q1: "How does NexusKV handle a network partition where the Leader is isolated?"
> **Candidate Defense:**  
> "If the leader (e.g. Node 1 in a 3-node cluster) is partitioned into a minority partition $\{Node_1\}$, it cannot satisfy the quorum equation $Q = \lfloor 3/2 \rfloor + 1 = 2$.  
> Meanwhile, in the majority partition $\{Node_2, Node_3\}$, election timeouts fire ($150\text{--}300\text{ ms}$). A candidate node (say Node 2) increments the term, requests votes from Node 3, secures $2/3$ votes, and becomes the legitimate Leader for Term 2.  
> If an isolated client continues writing to Node 1, Node 1's proposed log entries cannot be replicated to a quorum and therefore are **never committed**. When the network partition heals, Node 1 receives heartbeats with a higher term from Node 2, steps down to Follower, and truncates its uncommitted conflicting entries to match Node 2's authoritative log. Zero linearizability violations occur."

---

### Q2: "Why did you choose LSM-Trees over B+ Trees for this distributed engine?"
> **Candidate Defense:**  
> "In high-throughput distributed databases, write performance is dominated by random disk I/O. In a traditional B+ Tree, updating a record requires an in-place random write, triggering page splits and random head movements or flash block erases.  
> An LSM-Tree trades background compaction work for instantaneous sequential writes: every write enters our concurrent MemTable in RAM while sequentially appending a CRC32 frame to the Write-Ahead Log. This achieves over **102,000 QPS on standard NVMe**, while B-Trees typically bottleneck at $15\text{--}25\text{k QPS}$ under identical random-key write pressure."

---

### Q3: "How do you prevent Bloom Filters from ballooning memory?"
> **Candidate Defense:**  
> "We implement the **Kirsch-Mitzenmacher optimization** for double-hashing: instead of calculating $k$ independent hash functions (e.g. 5 expensive cryptographic hashes), we compute two base 64-bit hashes ($h_1$ and $h_2$) using Murmur3/CityHash and derive subsequent hashes via:
> $$g_i(x) = h_1(x) + i \cdot h_2(x) \pmod m$$
> We allocate 10 bits per key ($m/n = 10$), which mathematically fixes the false-positive rate at approximately $1\%$:
> $$p \approx \left(1 - e^{-k n / m}\right)^k \approx (0.5)^7 \approx 0.008$$
> The Bloom filter is serialized at the footer of each SSTable file and memory-mapped on demand, keeping RAM usage deterministic and bounded."

---

### Q4: "How does NexusKV ensure ACID transactions across multiple partitions?"
> **Candidate Defense:**  
> "For cross-region / multi-key transactions, we implement the **Two-Phase Commit (2PC) protocol** modeled after Google Spanner and TiKV:
> 1. **Timestamping:** We assign a monotonic start timestamp using Hybrid Logical Clocks (HLC), avoiding physical clock skew dependencies.
> 2. **Phase 1 (Prewrite):** The client selects a primary key and secondary keys. It acquires locks on each region and writes roll-forward intent records.
> 3. **Phase 2 (Commit):** The transaction coordinator atomically writes a Commit record to the primary key at `commit_ts`. Once the primary key is committed, the transaction is irrevocably successful. Secondary keys are lazily rolled forward, eliminating blocking coordinator deadlocks."

---

### Q5: "What makes your RESP protocol implementation production-ready?"
> **Candidate Defense:**  
> "Our RESP layer is implemented using Tokio's `BytesMut` with zero-copy stream chunking. We support the core Redis command suite (`SET`, `GET`, `DEL`, `KEYS`, `SCAN`, `ROLE`, `METRICS`), plus key expirations (`EXPIRE`, `TTL`, `PERSIST`, and `SET ... EX`), and atomic transaction pipelines (`MULTI`, `EXEC`, `DISCARD`).  
> Furthermore, any client—from `redis-cli`, official Python/Go/TypeScript SDKs, to Kubernetes health probes—connects transparently without proprietary client shims."

---

## 🏆 Key Metrics for Presentation
* **Sustained Throughput:** $102,100\text{ QPS}$ under 50 concurrent async workers.
* **P50 Latency:** $340\text{ \mu s}$
* **P99 Tail Latency:** $1.24\text{ ms}$
* **Binary Footprint:** Lightweight $3.3\text{ MB}$ statically linked executable.
* **Zero Dependencies:** Pure Rust engine with no external database runtime required.
