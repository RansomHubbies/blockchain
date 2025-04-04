#!/bin/bash

# ChatChain Testnet Runner
# This script starts a 4-node ChatChain testnet and ensures all nodes stay running

# Configuration
NODE_COUNT=4
BASE_PORT=8081
CHECK_INTERVAL=30  # seconds between health checks
LOG_DIR="testnet_logs"
PID_DIR="testnet_pids"

# Create directories if they don't exist
mkdir -p "$LOG_DIR"
mkdir -p "$PID_DIR"

# Function to start a node
start_node() {
    local node_id=$1
    local log_file="$LOG_DIR/node_${node_id}.log"
    
    echo "Starting node $node_id..."
    cargo run -- --node-id "$node_id" > "$log_file" 2>&1 &
    local pid=$!
    echo $pid > "$PID_DIR/node_${node_id}.pid"
    echo "Node $node_id started with PID $pid"
}

# Function to check if a node is running
is_node_running() {
    local node_id=$1
    local pid_file="$PID_DIR/node_${node_id}.pid"
    
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null; then
            # Also check if the node is responding to HTTP requests
            local port=$((BASE_PORT + node_id))
            if curl -s "http://127.0.0.1:${port}/nodes" > /dev/null 2>&1; then
                return 0  # Node is running and responding
            else
                echo "Node $node_id (PID $pid) is not responding to HTTP requests"
                return 1
            fi
        else
            echo "Node $node_id (PID $pid) is not running"
            return 1
        fi
    else
        echo "PID file for node $node_id not found"
        return 1
    fi
}

# Function to stop a node
stop_node() {
    local node_id=$1
    local pid_file="$PID_DIR/node_${node_id}.pid"
    
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        echo "Stopping node $node_id (PID $pid)..."
        kill "$pid" 2>/dev/null || true
        rm "$pid_file"
    fi
}

# Function to stop all nodes
stop_all_nodes() {
    echo "Stopping all nodes..."
    for ((i=0; i<NODE_COUNT; i++)); do
        stop_node $i
    done
}

# Handle cleanup on script exit
cleanup() {
    echo "Cleaning up..."
    stop_all_nodes
    exit 0
}

trap cleanup SIGINT SIGTERM

# Initial startup of all nodes
echo "Starting ChatChain testnet with $NODE_COUNT nodes..."
for ((i=0; i<NODE_COUNT; i++)); do
    start_node $i
    sleep 2  # Brief pause between node startups to avoid race conditions
done

# Give nodes time to initialize
echo "Waiting for nodes to initialize..."
sleep 5

# Main monitoring loop
echo "All nodes started. Monitoring health every $CHECK_INTERVAL seconds..."
echo "Press Ctrl+C to stop the testnet"

while true; do
    for ((i=0; i<NODE_COUNT; i++)); do
        if ! is_node_running $i; then
            echo "Restarting node $i..."
            stop_node $i  # Ensure any zombie process is killed
            start_node $i
            echo "Node $i restarted"
        fi
    done
    
    # Display status summary
    echo "$(date): Testnet status check complete. All nodes running."
    
    # Wait before next check
    sleep $CHECK_INTERVAL
done 