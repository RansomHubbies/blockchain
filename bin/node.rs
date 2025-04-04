use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use eyre::Result as EyreResult;
use redb::{Database, ReadableTable, TableDefinition};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

// Table definition for our messages
const MESSAGES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("messages");

// Step 2: Define the ChatMessage and ChatValue types
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChatMessage {
    pub sender: String,
    pub recipient: String,
    pub timestamp: i64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChatValue {
    pub messages: Vec<ChatMessage>,
}

// Step 3: Implement value ID for ChatValue
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatValueId(pub Vec<u8>);

// Implement Ord for ChatValueId
impl Ord for ChatValueId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for ChatValueId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Implement Display for ChatValueId
impl fmt::Display for ChatValueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChatValueId({:?})", self.0)
    }
}

// Node configuration for the testnet
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub id: usize,
    pub bind_addr: SocketAddr,
    pub peers: Vec<SocketAddr>,
    pub external_addr: Option<SocketAddr>, // For when the node's advertised address differs from bind_addr
}

// Node configuration from config.toml
#[derive(Clone, Debug, Deserialize)]
struct NodeConfigFile {
    id: usize,
    base_port: u16,
    ip: String,
    peers: String,
    external_addr: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct ConfigFile {
    node_config: Vec<NodeConfigFile>,
}

pub struct Mempool {
    pending: Vec<ChatMessage>,
    seen_message_ids: HashSet<String>, // Track messages we've already seen
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            pending: vec![],
            seen_message_ids: HashSet::new(),
        }
    }

    pub fn push(&mut self, msg: ChatMessage) -> bool {
        // Create a unique ID for the message to avoid duplicates
        let msg_id = format!("{}-{}-{}", msg.sender, msg.recipient, msg.timestamp);

        if self.seen_message_ids.contains(&msg_id) {
            return false; // We've already seen this message
        }

        self.seen_message_ids.insert(msg_id);
        self.pending.push(msg);
        true
    }

    pub fn drain(&mut self) -> Vec<ChatMessage> {
        std::mem::take(&mut self.pending)
    }
}

pub struct ChainState {
    pub mempool: Mempool,
    pub db: Database,
    pub node_config: NodeConfig,
    pub http_client: HttpClient,
}

impl ChainState {
    pub fn new(node_config: NodeConfig, db_path: &Path) -> EyreResult<Self> {
        // Open or create the database
        let db = Database::create(db_path)?;

        // Initialize the database schema if needed
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(MESSAGES_TABLE)?;
        }
        write_txn.commit()?;

        // Create HTTP client for peer communication
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        Ok(Self {
            mempool: Mempool::new(),
            db,
            node_config,
            http_client,
        })
    }

    // Persist a message to the database
    pub fn persist_message(&self, msg: &ChatMessage) -> EyreResult<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(MESSAGES_TABLE)?;
            let key = format!("{}-{}", msg.timestamp, msg.sender);
            let value = serde_json::to_vec(msg)?;
            table.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    // Get all messages from the database
    pub fn get_all_messages(&self) -> EyreResult<Vec<ChatMessage>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(MESSAGES_TABLE)?;

        let mut messages = Vec::new();
        for result in table.iter()? {
            let (_, value_data) = result?;
            let value_bytes = value_data.value();
            let message: ChatMessage = serde_json::from_slice(value_bytes)?;
            messages.push(message);
        }

        // Sort messages by timestamp
        messages.sort_by_key(|msg| msg.timestamp);

        Ok(messages)
    }
}

// Axum handlers
#[derive(Deserialize)]
struct MempoolPayload {
    sender: String,
    recipient: String,
    text: String,
    timestamp: i64,
}

/// POST /messages - Simplified endpoint to send a message - handles push, sync and commit in one operation
async fn send_message(
    State(state): State<Arc<Mutex<ChainState>>>,
    Json(payload): Json<MempoolPayload>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    // First, create the message
    let message = ChatMessage {
        sender: payload.sender.clone(),
        recipient: payload.recipient.clone(),
        timestamp: payload.timestamp,
        text: payload.text.clone(),
    };

    let node_id;
    let peers;
    let http_client;

    // Add to mempool and persist to local DB
    {
        let mut st = state.lock().await;
        node_id = st.node_config.id;
        peers = st.node_config.peers.clone();
        http_client = st.http_client.clone();
        
        st.mempool.push(message.clone());
        debug!(
            "Node {}: Added message from {} to {}",
            node_id, payload.sender, payload.recipient
        );

        if let Err(e) = st.persist_message(&message) {
            error!("Node {}: Failed to persist message: {}", node_id, e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
        debug!("Node {}: Committed message to local DB", node_id);
    }

    // Immediately broadcast to peers (synchronous attempt)
    {
        for peer_addr in peers {
            // Skip if peer is ourselves - using node_id comparison
            if is_self_addr(&peer_addr, state.clone()).await {
                continue;
            }

            let peer_url = format!("http://{}/sync", peer_addr);

            // Basic retry logic
            let mut retry_count = 0;
            let max_retries = 3;
            let mut backoff_ms = 100;

            loop {
                match http_client
                    .post(&peer_url)
                    .json(&vec![message.clone()])
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            debug!(
                                "Node {}: Successfully synced message to peer {}",
                                node_id, peer_addr
                            );
                            break;
                        } else {
                            warn!(
                                "Node {}: Failed to sync message with peer {}: status {}",
                                node_id,
                                peer_addr,
                                resp.status()
                            );
                            break;
                        }
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count >= max_retries {
                            warn!(
                                "Node {}: Failed to sync message with peer {} after retries: {}",
                                node_id, peer_addr, e
                            );
                            break;
                        }

                        // Exponential backoff
                        sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;
                    }
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "status": "success",
        "node_id": node_id
    })))
}

// Helper function to determine if an address is this node's address
async fn is_self_addr(addr: &SocketAddr, state: Arc<Mutex<ChainState>>) -> bool {
    let st = state.lock().await;
    
    // Check against external address if available
    if let Some(external) = &st.node_config.external_addr {
        return addr == external;
    }
    
    // Check against bind address
    if addr == &st.node_config.bind_addr {
        return true;
    }
    
    // Handle localhost variations
    if (addr.ip().is_loopback() || addr.ip().is_unspecified()) && 
       st.node_config.bind_addr.ip().is_loopback() &&
       addr.port() == st.node_config.bind_addr.port() {
        return true;
    }
    
    false
}

/// POST /sync - Internal endpoint for peers to sync messages with us
async fn sync_messages(
    State(state): State<Arc<Mutex<ChainState>>>,
    Json(messages): Json<Vec<ChatMessage>>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let mut st = state.lock().await;
    let node_id = st.node_config.id;
    let mut new_count = 0;

    // Store any new messages we receive
    for msg in messages {
        let key = format!("{}-{}", msg.timestamp, msg.sender);
        // Check if we already have this message
        let exists = {
            let read_txn = match st.db.begin_read() {
                Ok(txn) => txn,
                Err(e) => {
                    error!("Node {}: Database error: {}", node_id, e);
                    return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
                }
            };

            let table = match read_txn.open_table(MESSAGES_TABLE) {
                Ok(table) => table,
                Err(e) => {
                    error!("Node {}: Failed to open table: {}", node_id, e);
                    return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
                }
            };

            match table.get(key.as_str()) {
                Ok(opt) => opt.is_some(),
                Err(e) => {
                    error!("Node {}: Database query error: {}", node_id, e);
                    return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        };

        if !exists {
            // We don't have this message, so store it
            if let Err(e) = st.persist_message(&msg) {
                error!("Node {}: Failed to persist message: {}", node_id, e);
                continue;
            }
            new_count += 1;
        }

        // Also add to mempool to propagate to other peers if needed
        st.mempool.push(msg);
    }

    if new_count > 0 {
        debug!(
            "Node {}: Added {} new messages from peer sync",
            node_id, new_count
        );
    }

    Ok(Json(serde_json::json!({
        "status": "success",
        "new_messages": new_count,
        "node_id": node_id
    })))
}

/// GET /messages - Get all messages in the system
async fn get_messages(
    State(state): State<Arc<Mutex<ChainState>>>,
) -> Result<Json<Vec<ChatMessage>>, axum::http::StatusCode> {
    let st = state.lock().await;
    let node_id = st.node_config.id;

    match st.get_all_messages() {
        Ok(messages) => {
            debug!("Node {}: Returning {} messages", node_id, messages.len());
            Ok(Json(messages))
        }
        Err(e) => {
            error!("Node {}: Failed to get messages: {}", node_id, e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /nodes - Get information about all nodes in the network
async fn get_nodes(
    State(state): State<Arc<Mutex<ChainState>>>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let st = state.lock().await;
    let node_id = st.node_config.id;

    Ok(Json(serde_json::json!({
        "node_id": node_id,
        "bind_addr": st.node_config.bind_addr.to_string(),
        "peers": st.node_config.peers.iter().map(|p| p.to_string()).collect::<Vec<String>>(),
        "mempool_size": st.mempool.pending.len()
    })))
}

// Function to load config from file
fn load_config_for_node(node_id: usize) -> EyreResult<NodeConfigFile> {
    // Read config.toml
    let config_content = std::fs::read_to_string("config.toml")?;
    
    // Parse the TOML
    let config: ConfigFile = toml::from_str(&config_content)?;
    
    // Find the configuration for our node ID
    config.node_config.into_iter()
        .find(|cfg| cfg.id == node_id)
        .ok_or_else(|| eyre::eyre!("No configuration found for node ID {}", node_id))
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Node ID (0-3 for a 4-node testnet)
    #[clap(short, long, default_value = "0")]
    node_id: usize,
}

// Main function
#[tokio::main]
async fn main() -> EyreResult<()> {
    // Initialize logging
    tracing_subscriber::fmt().init();

    // Load config for node
    let args = Args::parse();
    let config = load_config_for_node(args.node_id).unwrap();

    // Node configuration
    let node_id = config.id;
    let bind_port = config.base_port; // Will be set from config or CLI
    let ip_addr_str = config.ip; // Default
    let peers_str = config.peers; // Default empty
    let external_addr_str = config.external_addr; // Default none

    let ip_addr = ip_addr_str.parse().unwrap();

    // Configure this node
    let bind_addr = SocketAddr::new(ip_addr, bind_port);
    
    // Parse external address if provided
    let external_addr = if let Some(addr_str) = external_addr_str {
        match addr_str.parse() {
            Ok(addr) => Some(addr),
            Err(e) => {
                warn!("Invalid external address format: {}, error: {}", addr_str, e);
                None
            }
        }
    } else {
        None
    };

    // Set up peer list
    let mut peers = Vec::new();
    for peer_str in peers_str.split(',') {
        if !peer_str.trim().is_empty() {
            match peer_str.trim().parse() {
                Ok(addr) => peers.push(addr),
                Err(e) => warn!("Invalid peer address format: {}, error: {}", peer_str, e),
            }
        }
    }

    let node_config = NodeConfig {
        id: node_id,
        bind_addr,
        peers,
        external_addr,
    };

    // Print a separator line
    info!("====================================================================");
    info!("STARTING CHATCHAIN TESTNET NODE {}", node_id);
    info!("====================================================================");
    info!("Node {} binding to {}", node_id, bind_addr);
    info!("Peers: {:?}", node_config.peers);
    info!("====================================================================");

    // Create database directory if it doesn't exist
    let db_path = Path::new("db");
    if !db_path.exists() {
        std::fs::create_dir_all(db_path)?;
    }

    // Create a separate database file for each node
    let db_file = db_path.join(format!("node_{}.redb", node_id));
    info!("Using database file: {}", db_file.display());

    // Create shared app state
    let state = Arc::new(Mutex::new(ChainState::new(node_config, &db_file)?));

    // Start background sync task
    let sync_state = state.clone();
    tokio::spawn(async move {
        sync_task(sync_state).await;
    });

    // Create Axum routes
    let app = Router::new()
        .route("/messages", post(send_message))
        .route("/messages", get(get_messages))
        .route("/sync", post(sync_messages)) // Internal endpoint for peer sync
        .route("/nodes", get(get_nodes)) // Keep this for diagnostics
        .with_state(state);

    // Serve Axum
    info!(
        "Node {} HTTP server listening on {}",
        node_id, bind_addr
    );
    info!("====================================================================");
    axum::Server::bind(&bind_addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

// Background task to periodically sync with peers
async fn sync_task(state: Arc<Mutex<ChainState>>) {
    loop {
        // Sleep to avoid continuous syncing
        sleep(Duration::from_secs(60)).await;

        let (node_id, peers, http_client) = {
            let st = state.lock().await;
            let node_id = st.node_config.id;
            let peers = st.node_config.peers.clone();
            let client = st.http_client.clone();
            (node_id, peers, client)
        };

        debug!("Node {}: Starting sync with {} peers", node_id, peers.len());

        for peer_addr in &peers {
            // Skip if peer is ourselves, using the new helper function
            if is_self_addr(&peer_addr, state.clone()).await {
                continue;
            }

            // Attempt to pull peer's messages
            let peer_url = format!("http://{}/messages", peer_addr);
            let peer_messages = match http_client.get(&peer_url).send().await {
                Ok(resp) => match resp.json::<Vec<ChatMessage>>().await {
                    Ok(msgs) => {
                        debug!(
                            "Node {}: Received {} messages from peer {}",
                            node_id,
                            msgs.len(),
                            peer_addr
                        );
                        msgs
                    }
                    Err(e) => {
                        warn!(
                            "Node {}: Failed to parse messages from peer {}: {}",
                            node_id, peer_addr, e
                        );
                        continue;
                    }
                },
                Err(e) => {
                    warn!(
                        "Node {}: Failed to connect to peer {}: {}",
                        node_id, peer_addr, e
                    );
                    continue;
                }
            };

            // Store any new messages we discover
            let mut new_count = 0;
            {
                let st = &mut *state.lock().await;
                for msg in peer_messages {
                    let key = format!("{}-{}", msg.timestamp, msg.sender);
                    let read_txn = match st.db.begin_read() {
                        Ok(txn) => txn,
                        Err(e) => {
                            error!("Node {}: Database error: {}", node_id, e);
                            continue;
                        }
                    };

                    let table = match read_txn.open_table(MESSAGES_TABLE) {
                        Ok(table) => table,
                        Err(e) => {
                            error!("Node {}: Failed to open table: {}", node_id, e);
                            continue;
                        }
                    };

                    let exists = match table.get(key.as_str()) {
                        Ok(opt) => opt.is_some(),
                        Err(e) => {
                            error!("Node {}: Database query error: {}", node_id, e);
                            continue;
                        }
                    };

                    if !exists {
                        // We don't have this message, so store it
                        if let Err(e) = st.persist_message(&msg) {
                            error!("Node {}: Failed to persist message: {}", node_id, e);
                            continue;
                        }
                        new_count += 1;
                    }
                }

                // Also attempt to send our mempool to this peer
                let our_mempool = st.mempool.pending.clone();
                let _ = st; // release lock for network call

                if !our_mempool.is_empty() {
                    let sync_url = format!("http://{}/sync", peer_addr);
                    match http_client.post(&sync_url).json(&our_mempool).send().await {
                        Ok(_) => debug!(
                            "Node {}: Sent {} mempool messages to peer {}",
                            node_id,
                            our_mempool.len(),
                            peer_addr
                        ),
                        Err(e) => warn!(
                            "Node {}: Failed to send mempool to peer {}: {}",
                            node_id, peer_addr, e
                        ),
                    }
                }
            }

            if new_count > 0 {
                debug!(
                    "Node {}: Synced {} new messages from peer {}",
                    node_id, new_count, peer_addr
                );
            }
        }

        debug!("Node {}: Completed sync with peers", node_id);
    }
}
