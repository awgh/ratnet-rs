//! Basic RatNet node example
//!
//! This example demonstrates how to create and run a basic RatNet node
//! with UDP transport and polling policy.

use ratnet::prelude::*;
use ratnet::{
    nodes::MemoryNode, policy::PollPolicy, router::DefaultRouter, transports::UdpTransport,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting RatNet Rust example");

    // Initialize component registrations
    ratnet::router::init();
    ratnet::transports::init();
    ratnet::policy::init();

    // Create a new memory node
    let node = Arc::new(MemoryNode::new());

    // Create and set a default router
    let router = Arc::new(DefaultRouter::new());
    node.set_router(router);

    // Create UDP transport
    let transport = Arc::new(UdpTransport::new("127.0.0.1:8080".to_string()));

    // Create and set a polling policy
    let policy = Arc::new(PollPolicy::new(transport.clone(), node.clone()));
    node.set_policy(vec![policy]);

    // Start the transport
    transport
        .listen("127.0.0.1:8080".to_string(), false)
        .await?;

    // Start the node
    node.start().await?;

    info!("Node started and listening on UDP 127.0.0.1:8080");

    // Get and display the node's ID
    let node_id = node.id().await?;
    info!("Node ID: {}", node_id);

    // Add some test data
    node.add_contact("test_contact".to_string(), "test_pubkey_123".to_string())
        .await?;
    node.add_peer(
        "test_peer".to_string(),
        true,
        "127.0.0.1:8081".to_string(),
        Some("default".to_string()),
    )
    .await?;

    // Display contacts and peers
    let contacts = node.get_contacts().await?;
    info!("Contacts: {:?}", contacts);

    let peers = node.get_peers(None).await?;
    info!("Peers: {:?}", peers);

    // Run for a bit to see the polling in action
    info!("Running for 10 seconds...");
    sleep(Duration::from_secs(10)).await;

    // Graceful shutdown
    info!("Shutting down...");
    node.stop().await?;
    transport.stop().await?;

    info!("RatNet example completed");
    Ok(())
}
