# ChatChain Client

A robust Rust client for interacting with the ChatChain network. This client automatically sends requests to multiple nodes in parallel and uses the first successful response, providing fault tolerance if some nodes are down.

## Features

- **Fault Tolerance**: Communicates with all available nodes and uses the first successful response
- **Automatic Retries**: Configurable retry mechanism with backoff
- **Interactive Chat Mode**: Simple command-line chat interface
- **Command-line Client**: Full-featured CLI for all network operations

## Building

```bash
cargo build --release
```

The client binary will be available at `target/release/chatchain-client`.

## Usage

### Basic Commands

```bash
# Get help
./chatchain-client --help

# Send a message (with default node configuration)
./chatchain-client send --sender alice --recipient bob --text "Hello, Bob!"

# Get all messages from the network
./chatchain-client get-messages

# Get information about network nodes
./chatchain-client get-node-info

# Start an interactive chat session
./chatchain-client chat --user alice
```

### Configuring Nodes

By default, the client connects to a standard 4-node testnet running on localhost. You can specify custom node addresses:

```bash
./chatchain-client --nodes "http://192.168.1.10:8081,http://192.168.1.11:8082" get-messages
```

### Interactive Chat

The interactive chat mode allows you to send messages using a simple syntax:

```
> @recipient message text
```

For example:
```
> @bob Hey, how are you doing?
```

Type `exit` or `quit` to leave the chat session.

### Timeout and Retry Configuration

You can customize request timeout and retry behavior:

```bash
./chatchain-client --timeout-ms 5000 --max-retries 3 get-messages
```

## Library Usage

You can also use the client as a library in your Rust applications:

```rust
use chatchain::client::{ChatClient, ChatClientConfig};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Create a custom configuration
    let config = ChatClientConfig {
        node_addresses: vec![
            "http://127.0.0.1:8081".to_string(),
            "http://127.0.0.1:8082".to_string(),
        ],
        request_timeout_ms: 2000,
        max_retries: 2,
        retry_delay_ms: 500,
    };

    // Create the client
    let client = ChatClient::new(config);
    
    // Refresh active nodes list
    client.refresh_active_nodes().await?;
    
    // Send a message
    let response = client.send_message("alice", "bob", "Hello from Rust!").await?;
    println!("Message sent via node {}", response.node_id);
    
    // Get all messages
    let messages = client.get_messages().await?;
    println!("Retrieved {} messages", messages.len());
    
    Ok(())
}
``` 