# NexusKV Official TypeScript & JavaScript Client

High-performance, zero-dependency Node.js/TypeScript driver for **NexusKV**, the distributed Raft & LSM-Tree key-value database engine.

## Installation

```bash
npm install nexus-kv
```

## Quick Start

```typescript
import { NexusClient } from 'nexus-kv';

async function main() {
  const client = new NexusClient({ host: '127.0.0.1', port: 6379 });
  await client.connect();

  // Basic CRUD
  await client.set('user:101', 'Ali Hasanifard');
  const user = await client.get('user:101');
  console.log('User:', user); // "Ali Hasanifard"

  // Key Expiration (TTL)
  await client.setEx('auth:token', 'jwt_secret_token', 300);
  const ttl = await client.ttl('auth:token');
  console.log('TTL remaining:', ttl, 'seconds');

  // Wildcard Scanning
  const keys = await client.keys('user:*');
  console.log('Matched keys:', keys);

  // Cluster Role
  console.log('Role:', await client.role()); // "master"

  client.close();
}

main().catch(console.error);
```

## Features
- **Zero External Dependencies:** Built solely on Node.js core `net` module.
- **Strict Typing:** Ships with full `.d.ts` declaration maps.
- **Fast Streaming RESP Parser:** Zero-copy byte manipulation.
- **Key Expiration & TTL Support:** `setEx`, `expire`, `ttl`, `persist`.

## License
MIT License &copy; 2026 Ali Hasanifard
