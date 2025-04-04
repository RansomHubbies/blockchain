# ChatChain Testnet

This directory contains a script to easily spin up a local testnet for ChatChain with 4 nodes and a client.

## Prerequisites

- Rust and Cargo installed
- `curl` and `lsof` commands available in your terminal
- Ports 8080-8084 available on your machine

## Quick Start

To start the testnet with 4 nodes and a client, simply run:

```bash
./start_testnet.sh
```

This will:
1. Create a `db` directory if it doesn't exist
2. Kill any existing node or client processes
3. Clean up any existing database files
4. Generate a `config.toml` file for the testnet configuration
5. Start 4 nodes (on ports 8081-8084)
6. Start the client (on port 8080)
7. Wait for all nodes to be ready

## Testnet Configuration

The testnet is configured with:
- 4 nodes, each with its own database file (`node_0.redb`, `node_1.redb`, etc.)
- Each node connects to all other nodes in the network
- The client connects to all nodes for redundancy
- Nodes listen on ports 8081, 8082, 8083, and 8084
- The client listens on port 8080

## Using the Testnet

Once the testnet is running, you can interact with it via the client API:

- **Send a message**: `curl -X POST http://localhost:8080/messages -H "Content-Type: application/json" -d '{"sender":"alice", "recipient":"bob", "text":"Hello!", "timestamp":1234567890}'`
- **Get all messages**: `curl http://localhost:8080/messages`
- **Get network status**: `curl http://localhost:8080/network`

You can also interact directly with individual nodes using similar endpoints.

## Stopping the Testnet

To stop the testnet, press `Ctrl+C` in the terminal where it's running. The script will handle proper shutdown of all processes. 