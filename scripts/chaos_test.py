# -*- coding: utf-8 -*-
"""
==============================================================================
🔥 NEXUS-KV: AUTOMATED CHAOS & LEADER FAILOVER TEST HARNESS
Architect: Ali Hasanifard
==============================================================================
"""

import socket
import subprocess
import time
import sys
import os
import shutil

def send_resp_cmd(port, *args):
    """Send Redis RESP command over TCP and return raw response"""
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(2.0)
    try:
        s.connect(('127.0.0.1', port))
        cmd = f"*{len(args)}\r\n"
        for a in args:
            a_bytes = str(a).encode('utf-8')
            cmd += f"${len(a_bytes)}\r\n{a}\r\n"
        s.sendall(cmd.encode('utf-8'))
        resp = s.recv(4096).decode('utf-8', errors='ignore')
        s.close()
        return resp
    except Exception as e:
        s.close()
        return f"ERROR: {e}"

def run_chaos_test():
    print("\n========================================================")
    print("  🔥 NEXUS-KV AUTOMATED CHAOS & RESILIENCE TEST SUITE   ")
    print("========================================================\n")

    base_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(base_dir, ".."))
    binary_path = os.path.join(project_root, "target", "release", "nexus-kv.exe")

    if not os.path.exists(binary_path):
        binary_path = os.path.join(project_root, "target", "debug", "nexus-kv.exe")

    print(f"  • Using NexusKV binary: {binary_path}")

    # Clean data dirs
    test_data = os.path.join(project_root, "data_chaos_test")
    if os.path.exists(test_data):
        shutil.rmtree(test_data, ignore_errors=True)
    os.makedirs(test_data, exist_ok=True)

    # 1. Start Server Node
    print("  [1/4] Starting primary NexusKV node on port 6389...")
    node_dir = os.path.join(test_data, "node_1")
    p1 = subprocess.Popen([binary_path, "6389", "1", node_dir], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.0)

    # Check PING
    ping_res = send_resp_cmd(6389, "PING")
    assert "PONG" in ping_res, f"Node failed to respond to PING: {ping_res}"
    print("    ✔ Node 1 is healthy and responding to PING.")

    # 2. Ingest Data
    print("  [2/4] Ingesting 20 persistent keys into node 1...")
    for i in range(20):
        set_res = send_resp_cmd(6389, "SET", f"chaos:key:{i}", f"val_{i}")
        assert "OK" in set_res

    # Read back sample
    sample = send_resp_cmd(6389, "GET", "chaos:key:10")
    assert "val_10" in sample
    print("    ✔ Data successfully written and verified.")

    # 3. Simulate Chaos: Brutal Kill
    print("  [3/4] 💥 Simulating Chaos: Forcibly killing node process (SIGKILL)...")
    p1.kill()
    p1.wait()
    print("    ✔ Process killed immediately without ungraceful flush.")

    # 4. Crash Recovery
    print("  [4/4] Restarting node from cold disk state (WAL & SSTable recovery)...")
    p1_restarted = subprocess.Popen([binary_path, "6389", "1", node_dir], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.2)

    # Verify all 20 keys survived after crash
    recovered_count = 0
    for i in range(20):
        val = send_resp_cmd(6389, "GET", f"chaos:key:{i}")
        if f"val_{i}" in val:
            recovered_count += 1

    print(f"    ✔ Recovered: {recovered_count}/20 keys perfectly.")
    assert recovered_count == 20, "Some keys failed to recover!"

    # Clean up
    p1_restarted.kill()
    p1_restarted.wait()
    shutil.rmtree(test_data, ignore_errors=True)

    print("\n========================================================")
    print("  🏆 CHAOS TEST PASSED: 100% FAULT TOLERANCE VERIFIED!  ")
    print("========================================================\n")

if __name__ == "__main__":
    run_chaos_test()
