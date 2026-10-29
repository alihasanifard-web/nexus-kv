// ==============================================================================
// 📊 NEXUS-KV: PROMETHEUS METRICS EXPORTER
// Architect: Ali Hasanifard
// ==============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

pub static METRIC_OPS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static METRIC_READS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static METRIC_WRITES_TOTAL: AtomicU64 = AtomicU64::new(0);

pub struct Metrics;

impl Metrics {
    pub fn inc_op() {
        METRIC_OPS_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_read() {
        METRIC_READS_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_write() {
        METRIC_WRITES_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    pub fn render_prometheus(node_id: u64, current_term: u64, memtable_bytes: usize, sstable_count: usize) -> String {
        let mut out = String::new();

        out.push_str("# HELP nexus_ops_total Total operations handled\n");
        out.push_str("# TYPE nexus_ops_total counter\n");
        out.push_str(&format!("nexus_ops_total{{node_id=\"{}\"}} {}\n\n", node_id, METRIC_OPS_TOTAL.load(Ordering::Relaxed)));

        out.push_str("# HELP nexus_reads_total Total read operations handled\n");
        out.push_str("# TYPE nexus_reads_total counter\n");
        out.push_str(&format!("nexus_reads_total{{node_id=\"{}\"}} {}\n\n", node_id, METRIC_READS_TOTAL.load(Ordering::Relaxed)));

        out.push_str("# HELP nexus_writes_total Total write operations handled\n");
        out.push_str("# TYPE nexus_writes_total counter\n");
        out.push_str(&format!("nexus_writes_total{{node_id=\"{}\"}} {}\n\n", node_id, METRIC_WRITES_TOTAL.load(Ordering::Relaxed)));

        out.push_str("# HELP nexus_raft_term Current Raft term\n");
        out.push_str("# TYPE nexus_raft_term gauge\n");
        out.push_str(&format!("nexus_raft_term{{node_id=\"{}\"}} {}\n\n", node_id, current_term));

        out.push_str("# HELP nexus_memtable_bytes Approximate MemTable RAM usage in bytes\n");
        out.push_str("# TYPE nexus_memtable_bytes gauge\n");
        out.push_str(&format!("nexus_memtable_bytes{{node_id=\"{}\"}} {}\n\n", node_id, memtable_bytes));

        out.push_str("# HELP nexus_sstable_count Total on-disk SSTable files\n");
        out.push_str("# TYPE nexus_sstable_count gauge\n");
        out.push_str(&format!("nexus_sstable_count{{node_id=\"{}\"}} {}\n", node_id, sstable_count));

        out
    }
}
