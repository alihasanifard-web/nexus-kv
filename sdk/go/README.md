# NexusKV Official Go Driver

High-performance, idiomatic Go driver for **NexusKV**, the distributed Raft & LSM-Tree key-value database engine.

## Installation

```bash
go get github.com/alihasanifard-web/nexus-kv/sdk/go
```

## Quick Start

```go
package main

import (
	"context"
	"fmt"
	"log"
	"time"

	nexus "github.com/alihasanifard-web/nexus-kv/sdk/go"
)

func main() {
	ctx := context.Background()

	// Connect to NexusKV
	client, err := nexus.NewClient("127.0.0.1:6379", nexus.WithTimeout(2*time.Second))
	if err != nil {
		log.Fatalf("Failed to connect: %v", err)
	}
	defer client.Close()

	// Ping
	pong, err := client.Ping(ctx)
	fmt.Println("Ping reply:", pong) // PONG

	// Basic SET & GET
	err = client.Set(ctx, "user:101", "Ali Hasanifard")
	if err != nil {
		log.Fatalf("Set failed: %v", err)
	}

	val, found, err := client.Get(ctx, "user:101")
	if err != nil || !found {
		log.Fatalf("Key not found or error: %v", err)
	}
	fmt.Printf("Retrieved user:101 = %s\n", val)

	// Set with TTL (5 minutes)
	_ = client.SetEx(ctx, "auth:token", "jwt_secret_99", 300)
	ttl, _ := client.TTL(ctx, "auth:token")
	fmt.Printf("TTL remaining: %d seconds\n", ttl)

	// Wildcard Keys Scan
	matched, _ := client.Keys(ctx, "user:*")
	fmt.Printf("Matched keys: %v\n", matched)
}
```

## Features
- **Zero External Dependencies:** Built solely on standard Go libraries (`net`, `bufio`, `sync`).
- **High Concurrency:** Thread-safe socket pipeline with automatic connection recovery.
- **RESP v2 Compliant:** Native binary encoding and streaming parser.
- **Full Key Expiration Support:** `SetEx`, `Expire`, `TTL`, `Persist`.

## License
MIT License &copy; 2026 Ali Hasanifard
