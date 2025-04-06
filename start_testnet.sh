#!/bin/bash

# exit script on any error
set -e

# Load environment variables if needed
# source ~/.profile

# Change to the project directory
cd /home/iiitd/blockchain

# Directory for chatchain database
DB_DIR="db"

# Create database directory if it doesn't exist
if [ ! -d "$DB_DIR" ]; then
  echo "Creating database directory: $DB_DIR"
  mkdir -p "$DB_DIR"
fi

# Function to wait for a node to be ready
wait_for_node() {
  local port=$1
  local max_attempts=30
  local attempt=1

  echo "Waiting for node on port $port to be ready..."
  
  while ! curl --silent --output /dev/null --fail http://localhost:$port/nodes; do
    if [ $attempt -ge $max_attempts ]; then
      echo "Node on port $port did not become ready after $max_attempts attempts."
      return 1
    fi
    
    echo "Attempt $attempt/$max_attempts: Node on port $port not ready yet, waiting..."
    sleep 1
    ((attempt++))
  done
  
  echo "Node on port $port is ready!"
  return 0
}

# Kill any existing node processes if they exist
echo "Checking for existing nodes and client processes..."
pkill -f "cargo run --bin node" >/dev/null 2>&1 || true
pkill -f "cargo run --bin client" >/dev/null 2>&1 || true

# Node configuration
NODES=4
BASE_PORT=8081
IP="0.0.0.0"

# Generate the config.toml file
echo "Generating config.toml..."
cat > config.toml << EOF
[client]
bind_addr = "0.0.0.0:8080"
request_timeout_ms = 5000

[nodes]
addresses = [
EOF

# Add node addresses to config
for (( i=0; i<$NODES; i++ )); do
  PORT=$(($BASE_PORT + $i))
  echo "    \"127.0.0.1:$PORT\"," >> config.toml
done

# Close the addresses array
echo "]" >> config.toml

# Add node configurations
for (( i=0; i<$NODES; i++ )); do
  PORT=$(($BASE_PORT + $i))
  
  # Build peer list for this node
  PEERS=""
  for (( j=0; j<$NODES; j++ )); do
    if [ $i -ne $j ]; then
      PEER_PORT=$(($BASE_PORT + $j))
      if [ -z "$PEERS" ]; then
        PEERS="127.0.0.1:$PEER_PORT"
      else
        PEERS="$PEERS,127.0.0.1:$PEER_PORT"
      fi
    fi
  done
  
  # Add node config to the file
  cat >> config.toml << EOF

[[node_config]]
id = $i
base_port = $PORT
ip = "$IP"
peers = "$PEERS"
EOF
done

echo "Config file generated."

# Start the nodes
declare -a NODE_PIDS
for (( i=0; i<$NODES; i++ )); do
  NODE_ID=$i
  echo "Starting node $NODE_ID..."
  
  # Use cargo to start the node with the config file
  cargo run --bin node -- --node-id $NODE_ID &
  
  # Store the process ID for later
  NODE_PIDS[$i]=$!
  
  # Wait a bit between starting nodes to avoid port conflicts
  sleep 2
  
  # Wait until the node is responsive
  NODE_PORT=$(($BASE_PORT + $i))
  if ! wait_for_node $NODE_PORT; then
    echo "Failed to start node $NODE_ID"
    exit 1
  fi
done

echo "All nodes started successfully!"
echo "Starting client..."

# Start the client
cargo run --bin client &
CLIENT_PID=$!

# Wait for client to start
sleep 2

echo "======================================================================"
echo "Testnet is running with $NODES nodes and client"
echo "Client API available at: http://localhost:8080"
echo "Node endpoints:"
for (( i=0; i<$NODES; i++ )); do
  PORT=$(($BASE_PORT + $i))
  echo "  Node $i: http://localhost:$PORT"
done
echo "======================================================================"

# For systemd, we don't need the cleanup function, as systemd manages the service lifecycle
# Just wait for all background processes
wait 