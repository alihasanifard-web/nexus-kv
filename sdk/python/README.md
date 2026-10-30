# NexusKV Python Client Library

Official high-performance Python driver for **NexusKV**, the distributed Raft & LSM-Tree key-value database engine.

## Installation

```bash
pip install nexus-kv
```

## Quick Start

```python
from nexus_kv import NexusClient

# Connect to NexusKV node
with NexusClient(host="127.0.0.1", port=6379) as client:
    # Basic CRUD
    client.set("user:101", "Ali Hasanifard")
    print(client.get("user:101"))  # "Ali Hasanifard"

    # Expiration & TTL
    client.set_ex("session:token", "jwt_abc123", seconds=300)
    print("TTL remaining:", client.ttl("session:token"))

    # Wildcard Pattern Scanning
    keys = client.keys("user:*")
    print("Matched keys:", keys)
```

## License
MIT License &copy; 2026 Ali Hasanifard
