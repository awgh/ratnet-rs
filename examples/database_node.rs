//! Database Node Example
//!
//! This example demonstrates how to create and use a database-backed RatNet node
//! that persists all data to SQLite.

use ratnet::prelude::*;
use ratnet::{
    database::SqliteDatabase, nodes::DatabaseNode, policy::PollPolicy, router::DefaultRouter,
    transports::UdpTransport,
};
use std::sync::Arc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting RatNet Database Node Example");

    // Initialize component registrations
    ratnet::init();

    // Create in-memory SQLite database for this example
    let database = Arc::new(
        SqliteDatabase::new_memory()
            .await
            .expect("Failed to create database"),
    );

    // Create database-backed node
    let node = Arc::new(
        DatabaseNode::new(database.clone())
            .await
            .expect("Failed to create database node"),
    );

    // Create router and transport
    let router = Arc::new(DefaultRouter::new());
    node.set_router(router);

    let transport = Arc::new(UdpTransport::new("127.0.0.1:8080".to_string()));
    let policy = Arc::new(PollPolicy::new(transport.clone(), node.clone()));
    node.set_policy(vec![policy]);

    // Start the transport and node
    transport
        .listen("127.0.0.1:8080".to_string(), false)
        .await?;
    node.start().await?;

    info!("Database node started and listening on UDP 127.0.0.1:8080");

    // Get node ID
    match node.id().await {
        Ok(id) => info!("Node ID: {}", id),
        Err(e) => error!("Failed to get node ID: {}", e),
    }

    // Demonstrate database persistence
    info!("Adding test data to demonstrate persistence...");

    // Add contacts
    node.add_contact("alice".to_string(), "alice_pubkey_123".to_string())
        .await?;
    node.add_contact("bob".to_string(), "bob_pubkey_456".to_string())
        .await?;

    // Add channels
    node.add_channel("general".to_string(), "general_channel_privkey".to_string())
        .await?;
    node.add_channel("private".to_string(), "private_channel_privkey".to_string())
        .await?;

    // Add profiles
    node.add_profile("default_profile".to_string(), true)
        .await?;

    // Add peers
    node.add_peer(
        "peer1".to_string(),
        true,
        "127.0.0.1:8081".to_string(),
        Some("main".to_string()),
    )
    .await?;
    node.add_peer(
        "peer2".to_string(),
        false,
        "127.0.0.1:8082".to_string(),
        Some("backup".to_string()),
    )
    .await?;

    info!("Test data added successfully");

    // Display current data
    match node.get_contacts().await {
        Ok(contacts) => {
            info!("Contacts: {:?}", contacts);
        }
        Err(e) => error!("Failed to get contacts: {}", e),
    }

    match node.get_channels().await {
        Ok(channels) => {
            info!("Channels: {:?}", channels);
        }
        Err(e) => error!("Failed to get channels: {}", e),
    }

    match node.get_profiles().await {
        Ok(profiles) => {
            info!("Profiles: {:?}", profiles);
        }
        Err(e) => error!("Failed to get profiles: {}", e),
    }

    match node.get_peers(None).await {
        Ok(peers) => {
            info!("Peers: {:?}", peers);
        }
        Err(e) => error!("Failed to get peers: {}", e),
    }

    info!("Running for 10 seconds...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    info!("Shutting down...");

    // Gracefully stop the node
    if let Err(e) = node.stop().await {
        error!("Error stopping node: {}", e);
    }

    // Stop the transport
    if let Err(e) = transport.stop().await {
        error!("Error stopping transport: {}", e);
    }

    info!("Database node example completed");
    info!("This example used an in-memory database for demonstration");
    info!("For file-based persistence, use SqliteDatabase::new(\"sqlite://path/to/file.db\")");

    Ok(())
}
