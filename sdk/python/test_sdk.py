# -*- coding: utf-8 -*-
"""
Test suite for official Python NexusKV driver.
"""

import subprocess
import time
import os
import shutil
from nexus_kv import NexusClient

def test_python_sdk():
    print("Testing NexusKV Python SDK...")
    base_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(base_dir, "..", ".."))
    binary = os.path.join(project_root, "target", "release", "nexus-kv.exe")
    if not os.path.exists(binary):
        binary = os.path.join(project_root, "target", "debug", "nexus-kv.exe")

    data_dir = os.path.join(project_root, "data_py_sdk_test")
    shutil.rmtree(data_dir, ignore_errors=True)

    server = subprocess.Popen([binary, "6399", "1", data_dir], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.0)

    try:
        with NexusClient(port=6399) as client:
            # 1. PING
            assert client.ping() == "PONG"

            # 2. SET & GET
            assert client.set("user:name", "Ali Hasanifard")
            assert client.get("user:name") == "Ali Hasanifard"

            # 3. Non-existent key
            assert client.get("user:unknown") is None

            # 4. KEYS
            assert client.set("user:role", "Architect")
            keys = client.keys("user:*")
            assert len(keys) == 2
            assert "user:name" in keys and "user:role" in keys

            # 5. DEL
            assert client.delete("user:name")
            assert client.get("user:name") is None

            # 6. METRICS
            metrics_txt = client.metrics()
            assert "nexus_ops_total" in metrics_txt

        print("✔ Python SDK verified successfully with 100% test pass!")
    finally:
        server.kill()
        server.wait()
        shutil.rmtree(data_dir, ignore_errors=True)

if __name__ == "__main__":
    test_python_sdk()
