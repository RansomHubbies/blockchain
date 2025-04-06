#!/bin/bash

# Don't exit script on any error - we want to continue even if some nodes fail
# Removing the set -e line

# Add path to cargo
export PATH="$HOME/.cargo/bin:$PATH"

# Load environment variables if needed
# source ~/.profile

# Change to the project directory
cd "$(pwd)"

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
pkill -f "target/release/node" >/dev/null 2>&1 || true
pkill -f "target/release/client" >/dev/null 2>&1 || true

# Build binaries in release mode
echo "Building binaries in release mode..."
cargo build --release --bin node --bin client

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
declare -a ACTIVE_NODES
SUCCESSFUL_NODES=0

for (( i=0; i<$NODES; i++ )); do
  NODE_ID=$i
  echo "Starting node $NODE_ID..."
  
  # Run the binary directly
  ./target/release/node --node-id $NODE_ID &
  
  # Store the process ID for later
  NODE_PIDS[$i]=$!
  
  # Wait a bit between starting nodes to avoid port conflicts
  sleep 2
  
  # Wait until the node is responsive
  NODE_PORT=$(($BASE_PORT + $i))
  if wait_for_node $NODE_PORT; then
    echo "Node $NODE_ID started successfully!"
    ACTIVE_NODES+=($i)
    SUCCESSFUL_NODES=$((SUCCESSFUL_NODES + 1))
  else
    echo "Failed to start node $NODE_ID, continuing with other nodes..."
    # Kill the failed node process
    kill ${NODE_PIDS[$i]} 2>/dev/null || true
  fi
done

# Check if at least one node started successfully
if [ $SUCCESSFUL_NODES -eq 0 ]; then
  echo "Error: No nodes started successfully. Exiting."
  exit 1
fi

echo "$SUCCESSFUL_NODES out of $NODES nodes started successfully!"
echo "Starting client..."

# Start the client
./target/release/client &
CLIENT_PID=$!

# Wait for client to start
sleep 2

echo "======================================================================"
echo "Testnet is running with $SUCCESSFUL_NODES active nodes and client"
echo "Client API available at: http://localhost:8080"
echo "Active node endpoints:"
for node_id in "${ACTIVE_NODES[@]}"; do
  PORT=$(($BASE_PORT + $node_id))
  echo "  Node $node_id: http://localhost:$PORT"
done
echo "======================================================================"

# For systemd, we don't need the cleanup function, as systemd manages the service lifecycle
# Just wait for all background processes
wait 