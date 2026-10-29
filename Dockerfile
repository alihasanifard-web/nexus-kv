# ==============================================================================
# 🐳 NEXUS-KV: DOCKER MULTI-STAGE HIGH-PERFORMANCE BUILDER
# Architect: Ali Hasanifard
# ==============================================================================

FROM rust:1.80-alpine AS builder

WORKDIR /usr/src/nexus-kv
RUN apk add --no-cache musl-dev

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

# Minimal Runtime Stage (< 20MB)
FROM alpine:3.20

WORKDIR /nexus-data
COPY --from=builder /usr/src/nexus-kv/target/release/nexus-kv /usr/local/bin/nexus-kv
COPY --from=builder /usr/src/nexus-kv/target/release/nexus_bench /usr/local/bin/nexus-bench

EXPOSE 6379 9001
VOLUME ["/nexus-data"]

ENTRYPOINT ["nexus-kv"]
CMD ["6379", "1", "/nexus-data"]
