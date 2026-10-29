// ==============================================================================
// 🧪 NEXUS-KV: INTEGRATION & STRESS TEST SUITE
// Architect: Ali Hasanifard
// ==============================================================================

use nexus_kv::engine::NexusEngine;
use nexus_kv::lsm::LsmConfig;
use nexus_kv::proto::resp::RespValue;
use nexus_kv::raft::log::RaftCommand;
use nexus_kv::raft::node::RaftNode;
use nexus_kv::raft::state::NodeRole;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_e2e_stress_write_and_compaction() {
    let dir = tempdir().unwrap();
    let config = LsmConfig {
        dir: dir.path().to_path_buf(),
        memtable_threshold_bytes: 512, // small to trigger multiple SSTable flushes
        compaction_threshold_ssts: 3,  // trigger auto-compaction
    };

    let engine = NexusEngine::new(1, vec![], config).unwrap();
    engine.raft.become_leader();

    // Ingest 500 records
    for i in 0..500 {
        let key = format!("bench:user:{:04}", i);
        let val = format!("profile_payload_for_user_{}", i);
        let set_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"SET".to_vec())),
            RespValue::BulkString(Some(key.into_bytes())),
            RespValue::BulkString(Some(val.into_bytes())),
        ]);
        assert_eq!(set_res, RespValue::SimpleString("OK".to_string()));
    }

    // Force compaction
    engine.storage.flush().unwrap();
    engine.storage.compact().unwrap();

    // Verify all 500 records are readable
    for i in 0..500 {
        let key = format!("bench:user:{:04}", i);
        let expected_val = format!("profile_payload_for_user_{}", i);
        let get_res = engine.execute_command(&[
            RespValue::BulkString(Some(b"GET".to_vec())),
            RespValue::BulkString(Some(key.into_bytes())),
        ]);
        assert_eq!(get_res, RespValue::BulkString(Some(expected_val.into_bytes())));
    }
}

#[test]
fn test_distributed_raft_leader_failover_simulation() {
    // 3-node cluster
    let n1 = Arc::new(RaftNode::new(1, vec![2, 3]));
    let n2 = Arc::new(RaftNode::new(2, vec![1, 3]));
    let n3 = Arc::new(RaftNode::new(3, vec![1, 2]));

    // Node 1 becomes leader
    let (vote_args, mut v1) = n1.start_election();
    if n2.handle_request_vote(&vote_args).vote_granted { v1 += 1; }
    if n3.handle_request_vote(&vote_args).vote_granted { v1 += 1; }
    assert!(v1 >= 2);
    n1.become_leader();
    assert_eq!(n1.state.read().role, NodeRole::Leader);

    // Leader proposes data
    n1.propose(RaftCommand::Put {
        key: b"config:cluster".to_vec(),
        value: b"active".to_vec(),
    }).unwrap();

    // Replicate to n2
    let append_to_n2 = n1.create_heartbeat(2);
    let reply_n2 = n2.handle_append_entries(&append_to_n2);
    assert!(reply_n2.success);
    n1.handle_peer_append_reply(2, &reply_n2);
    assert_eq!(n1.state.read().commit_index, 1);

    // SIMULATE LEADER CRASH: Node 1 goes offline!
    // Node 2 detects timeout and initiates election for term 2
    let (v2_args, mut v2) = n2.start_election();
    assert_eq!(v2_args.term, 2);

    // Node 3 votes for Node 2
    let r3_for_2 = n3.handle_request_vote(&v2_args);
    if r3_for_2.vote_granted { v2 += 1; }
    assert!(v2 >= 2);

    // Node 2 becomes the new Leader!
    n2.become_leader();
    assert_eq!(n2.state.read().role, NodeRole::Leader);
    assert_eq!(n2.state.read().current_term, 2);

    // New leader successfully writes new data
    let idx2 = n2.propose(RaftCommand::Put {
        key: b"config:leader".to_vec(),
        value: b"node_2".to_vec(),
    }).unwrap();
    assert_eq!(idx2, 2);
}

#[test]
fn test_crash_consistency_with_wal_replay() {
    let dir = tempdir().unwrap();
    let dir_path = dir.path().to_path_buf();

    // Session 1: write, delete, and crash without explicit flush
    {
        let config = LsmConfig {
            dir: dir_path.clone(),
            memtable_threshold_bytes: 10 * 1024 * 1024,
            compaction_threshold_ssts: 4,
        };
        let engine = NexusEngine::new(1, vec![], config).unwrap();
        engine.raft.become_leader();

        engine.execute_command(&[
            RespValue::BulkString(Some(b"SET".to_vec())),
            RespValue::BulkString(Some(b"persistent_key".to_vec())),
            RespValue::BulkString(Some(b"survives_crash".to_vec())),
        ]);

        engine.execute_command(&[
            RespValue::BulkString(Some(b"SET".to_vec())),
            RespValue::BulkString(Some(b"deleted_key".to_vec())),
            RespValue::BulkString(Some(b"to_be_removed".to_vec())),
        ]);

        engine.execute_command(&[
            RespValue::BulkString(Some(b"DEL".to_vec())),
            RespValue::BulkString(Some(b"deleted_key".to_vec())),
        ]);
        // Drops engine here -> simulating sudden shutdown!
    }

    // Session 2: recover from WAL
    {
        let config = LsmConfig {
            dir: dir_path,
            memtable_threshold_bytes: 10 * 1024 * 1024,
            compaction_threshold_ssts: 4,
        };
        let recovered_engine = NexusEngine::new(1, vec![], config).unwrap();
        recovered_engine.raft.become_leader();

        let val1 = recovered_engine.execute_command(&[
            RespValue::BulkString(Some(b"GET".to_vec())),
            RespValue::BulkString(Some(b"persistent_key".to_vec())),
        ]);
        assert_eq!(val1, RespValue::BulkString(Some(b"survives_crash".to_vec())));

        let val2 = recovered_engine.execute_command(&[
            RespValue::BulkString(Some(b"GET".to_vec())),
            RespValue::BulkString(Some(b"deleted_key".to_vec())),
        ]);
        assert_eq!(val2, RespValue::BulkString(None));
    }
}
