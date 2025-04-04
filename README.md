# ChatChain - Distributed Chat Message System

This is a simple distributed chat system that demonstrates network resilience with multiple nodes. The client proxies requests to available nodes, providing fault tolerance.

## Project Structure

- `bin/node.rs` - Individual chat nodes that store and sync messages
- `bin/client.rs` - Client API that forwards requests to nodes
- `config.toml` - Configuration for both client and nodes

## Dependencies

Add these dependencies to your Cargo.toml:

```toml
[dependencies]
axum = "0.6.0"
clap = { version = "4.0.0", features = ["derive"] }
eyre = "0.6.0"
redb = "1.0.0"  
reqwest = { version = "0.11.0", features = ["json"] }
serde = { version = "1.0.0", features = ["derive"] }
serde_json = "1.0.0"
tokio = { version = "1.0.0", features = ["full"] }
toml = "0.7.0"
tracing = "0.1.0"
tracing-subscriber = "0.3.0"
```

## Running the System

### Start the Nodes

Start multiple nodes to create a resilient network. Each node needs a unique ID:

```bash
# Node 0
cargo run --bin node -- --node-id 0 --peers "127.0.0.1:8082,127.0.0.1:8083,127.0.0.1:8084"

# Node 1
cargo run --bin node -- --node-id 1 --peers "127.0.0.1:8081,127.0.0.1:8083,127.0.0.1:8084"

# Node 2
cargo run --bin node -- --node-id 2 --peers "127.0.0.1:8081,127.0.0.1:8082,127.0.0.1:8084"

# Node 3  
cargo run --bin node -- --node-id 3 --peers "127.0.0.1:8081,127.0.0.1:8082,127.0.0.1:8083"
```

### Start the Client

The client will read configuration from `config.toml` and forward requests to available nodes:

```bash
cargo run --bin client
```

## API Endpoints

### Client Endpoints

The client API exposes the following endpoints:

- `POST /messages` - Send a new chat message
- `GET /messages` - Get all chat messages
- `GET /network` - Get network status

### Node Endpoints

Each node exposes:

- `POST /messages` - Send a new chat message
- `GET /messages` - Get all chat messages
- `POST /sync` - Internal endpoint for node synchronization
- `GET /nodes` - Get node information

## Message Format

To send a message:

```json
{
  "sender": "user1",
  "recipient": "user2",
  "text": "Hello world!",
  "timestamp": 1697913600
}
```

## Fault Tolerance

The system is designed to be resilient:

1. The client tries each node until it gets a successful response
2. Nodes sync messages with each other periodically
3. Even if some nodes are down, the system continues to work as long as at least one node is available

## How It Works

1. Messages are stored in a local database (redb) on each node 
2. Nodes sync messages with each other in the background
3. The client forwards requests to all nodes and returns the first successful response
4. If any node is up, the client can still successfully process requests

This design demonstrates a simple way to build distributed systems with redundancy and fault tolerance.

## License

MIT 