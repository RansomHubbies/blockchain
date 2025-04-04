# ChatChain

A simple blockchain-based chat application built with Rust.

## Overview

ChatChain is a simplified implementation that demonstrates how to store chat messages on a blockchain. It provides:

- A mempool for storing pending messages
- A simple commit mechanism (simulating blockchain consensus)
- JSON serialization for messages
- A persistent database using redb for storing committed messages
- A REST API for interacting with the application
- Support for a multi-node testnet

## Project Structure

The application consists of the following components:

- `ChatMessage` and `ChatValue`: Data structures for representing messages and blocks
- `Mempool`: An in-memory storage for pending messages
- `State`: The application state that manages the mempool and committed messages
- `Database`: A persistent store for committed messages using redb
- HTTP endpoints for pushing messages, committing blocks, and retrieving messages
- P2P communication for syncing messages across nodes

## API Endpoints

- `POST /mempool/push`: Push a new message to the mempool
- `POST /commit`: Commit all messages in the mempool to the blockchain
- `GET /messages`: Retrieve all committed messages
- `GET /nodes`: Get information about the current node
- `POST /mempool/sync`: Internal endpoint for node synchronization

## Building and Running

```bash
# Build the project
cargo build

# Run a single node
cargo run
```

## Running a Testnet

ChatChain supports running a local testnet with multiple nodes. By default, it's configured for a 4-node testnet:

```bash
# Run node 0 (default port: 8081)
cargo run -- --node-id 0

# Run node 1 (port: 8082)
cargo run -- --node-id 1

# Run node 2 (port: 8083)
cargo run -- --node-id 2

# Run node 3 (port: 8084)
cargo run -- --node-id 3
```

You can customize the base port and IP address:

```bash
# Run node 0 with custom base port
cargo run -- --node-id 0 --base-port 9000

# Run node 1 with custom base port
cargo run -- --node-id 1 --base-port 9000
```

Each node will:
- Listen on a different port (base_port + node_id)
- Store data in a separate database file
- Automatically sync with other nodes in the network

## API Usage Examples

### Add a message to the mempool (to any node)

```bash
curl -X POST -H "Content-Type: application/json" \
  -d '{"sender": "Alice", "recipient": "Bob", "text": "Hello, blockchain chat!", "timestamp": 1619827200}' \
  http://localhost:8081/mempool/push
```

### Commit messages from mempool to blockchain

```bash
curl -X POST http://localhost:8081/commit
```

### Get all committed messages (from any node)

```bash
curl http://localhost:8081/messages
```

### Get node information

```bash
curl http://localhost:8081/nodes
```

## Persistence

Messages committed to the blockchain are stored in a persistent database using redb. The database files are stored in the `chatchain_db` directory with names like `messages_0.redb` for node 0. This ensures that messages are preserved even if the application is restarted.

## Implementation Notes

This is a simplified implementation that demonstrates the core concepts without the complexity of a full blockchain consensus implementation. In a real-world scenario, you would:

1. Use a distributed consensus algorithm like Malachite BFT for agreeing on the chain state
2. Implement proper blockchain data structures with blocks and transactions
3. Add signature verification for message authenticity
4. Implement more robust peer-to-peer networking for distributed operation

## License

MIT 