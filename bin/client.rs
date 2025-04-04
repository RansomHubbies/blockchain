use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Json, State},
    routing::{get, post},
    Router,
};
use eyre::Result as EyreResult;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

// Reuse the same message types as the node for compatibility
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub sender: String,
    pub recipient: String,
    pub timestamp: i64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MempoolPayload {
    sender: String,
    recipient: String,
    text: String,
    timestamp: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct ClientConfig {
    bind_addr: String,
    request_timeout_ms: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct NodesConfig {
    addresses: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Config {
    client: ClientConfig,
    nodes: NodesConfig,
}

#[derive(Clone)]
struct AppState {
    node_addresses: Vec<String>,
    http_client: HttpClient,
}

// Handler for sending messages
async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MempoolPayload>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let node_addrs = &state.node_addresses;
    if node_addrs.is_empty() {
        error!("No node addresses configured");
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Track successful responses
    let mut successful_responses = Vec::new();

    // Try each node one by one, breaking on first success
    for addr in node_addrs {
        debug!("Sending message to node at {}", addr);
        let url = format!("http://{}/messages", addr);
        
        match state.http_client.post(&url).json(&payload).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) => {
                            successful_responses.push((addr.clone(), json));
                            break; // Stop on first success
                        }
                        Err(e) => {
                            warn!("Failed to parse response from {}: {}", addr, e);
                            continue;
                        }
                    }
                } else {
                    warn!("Request to {} failed with status: {}", addr, resp.status());
                    continue;
                }
            }
            Err(e) => {
                warn!("Failed to connect to node {}: {}", addr, e);
                continue;
            }
        }
    }
    
    if successful_responses.is_empty() {
        error!("Failed to send message to any node");
        return Err(axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    // Return the first successful response
    let (node_addr, response) = &successful_responses[0];
    debug!("Got successful response from node {}", node_addr);
    
    // Add client metadata to the response
    let mut response_with_metadata = response.clone();
    if let serde_json::Value::Object(ref mut map) = response_with_metadata {
        map.insert(
            "client_info".to_string(),
            serde_json::json!({
                "responded_node": node_addr,
                "total_responses": successful_responses.len(),
            }),
        );
    }

    Ok(Json(response_with_metadata))
}

// Handler for getting all messages
async fn get_messages(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ChatMessage>>, axum::http::StatusCode> {
    let node_addrs = &state.node_addresses;
    if node_addrs.is_empty() {
        error!("No node addresses configured");
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Try each node one by one, breaking on first success
    for addr in node_addrs {
        debug!("Getting messages from node at {}", addr);
        let url = format!("http://{}/messages", addr);
        
        match state.http_client.get(&url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<Vec<ChatMessage>>().await {
                        Ok(messages) => {
                            debug!("Got {} messages from {}", messages.len(), addr);
                            return Ok(Json(messages));
                        }
                        Err(e) => {
                            warn!("Failed to parse messages from {}: {}", addr, e);
                            continue;
                        }
                    }
                } else {
                    warn!("Request to {} failed with status: {}", addr, resp.status());
                    continue;
                }
            }
            Err(e) => {
                warn!("Failed to connect to node {}: {}", addr, e);
                continue;
            }
        }
    }
    
    error!("Failed to get messages from any node");
    Err(axum::http::StatusCode::SERVICE_UNAVAILABLE)
}

// Handler for getting network status
async fn get_network_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let node_addrs = &state.node_addresses;
    
    // Check status of each node
    let mut node_statuses = Vec::new();
    
    for (idx, addr) in node_addrs.iter().enumerate() {
        let url = format!("http://{}/nodes", addr);
        
        match state.http_client.get(&url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<serde_json::Value>().await {
                        Ok(status) => {
                            node_statuses.push((idx, addr.clone(), status));
                        }
                        Err(e) => {
                            warn!("Failed to parse status from {}: {}", addr, e);
                        }
                    }
                } else {
                    warn!("Request to {} failed with status: {}", addr, resp.status());
                }
            }
            Err(e) => {
                warn!("Failed to connect to node {}: {}", addr, e);
            }
        }
    }
    
    // Construct network status response
    let response = serde_json::json!({
        "network_size": node_addrs.len(),
        "online_nodes": node_statuses.len(),
        "nodes": node_statuses.into_iter().map(|(idx, addr, status)| {
            serde_json::json!({
                "index": idx,
                "address": addr,
                "status": status,
            })
        }).collect::<Vec<_>>(),
    });
    
    Ok(Json(response))
}

#[tokio::main]
async fn main() -> EyreResult<()> {
    // Initialize logging
    tracing_subscriber::fmt().init();

    // Load configuration from config.toml
    let config = load_config()?;
    
    // Parse the bind address
    let bind_addr: SocketAddr = config.client.bind_addr.parse()?;
    
    // Create HTTP client with appropriate timeout
    let http_client = HttpClient::builder()
        .timeout(Duration::from_millis(config.client.request_timeout_ms))
        .build()?;
    
    // Create app state
    let app_state = Arc::new(AppState {
        node_addresses: config.nodes.addresses,
        http_client,
    });
    
    // Print startup information
    info!("====================================================================");
    info!("STARTING CHATCHAIN CLIENT");
    info!("====================================================================");
    info!("Client binding to {}", bind_addr);
    info!("Configured nodes: {:?}", app_state.node_addresses);
    info!("Request timeout: {} ms", config.client.request_timeout_ms);
    info!("====================================================================");
    
    // Create Axum routes
    let app = Router::new()
        .route("/messages", post(send_message))
        .route("/messages", get(get_messages))
        .route("/network", get(get_network_status))
        .with_state(app_state);
    
    // Start the server
    info!("Client HTTP server listening on {}", bind_addr);
    info!("====================================================================");
    axum::Server::bind(&bind_addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

fn load_config() -> EyreResult<Config> {
    // Read config.toml
    let config_content = std::fs::read_to_string("config.toml")?;
    
    // Parse the TOML into our Config struct
    // Note: Need to add toml = "0.x" to Cargo.toml
    let config: Config = toml::from_str(&config_content)?;
    
    Ok(config)
}