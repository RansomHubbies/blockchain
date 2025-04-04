FROM rust:1.70-slim as builder

WORKDIR /usr/src/chatchain
COPY . .

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev && \
    rm -rf /var/lib/apt/lists/*

RUN cargo build --release

FROM debian:bullseye-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/chatchain/target/release/malachite-test /app/chatchain

# Create directory for the database files
RUN mkdir -p /app/chatchain_db

# Expose ports for the 4-node testnet
EXPOSE 8081 8082 8083 8084

ENTRYPOINT ["/app/chatchain"] 