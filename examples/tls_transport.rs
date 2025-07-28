//! TLS Transport Example
//!
//! This example demonstrates how to use the TLS transport for secure
//! communication in RatNet.

#[cfg(feature = "tls")]
use ratnet::prelude::*;
#[cfg(feature = "tls")]
use ratnet::{
    crypto::generate_test_cert, nodes::MemoryNode, policy::PollPolicy, router::DefaultRouter,
    transports::TlsTransport,
};
#[cfg(feature = "tls")]
use std::sync::Arc;
#[cfg(feature = "tls")]
use tracing::{error, info};
#[cfg(feature = "tls")]
#[cfg(feature = "tls")]
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting RatNet TLS Transport Example");

    // Initialize component registrations
    ratnet::init();

    // Generate test certificates
    let (cert_pem, key_pem) = generate_test_cert().expect("Failed to generate test certificate");

    info!("Generated test certificate for TLS transport");

    // Create memory node
    let node = Arc::new(MemoryNode::new());

    // Create router and set it
    let router = Arc::new(DefaultRouter::new());
    node.set_router(router);

    // Create TLS transport with certificates
    let transport = Arc::new(TlsTransport::new_with_certs(
        "127.0.0.1:8443".to_string(),
        cert_pem,
        key_pem,
        true, // ECC mode
        node.clone(),
    ));

    // Create policy
    let policy = Arc::new(PollPolicy::new(transport.clone(), node.clone()));
    node.set_policy(vec![policy]);

    // Start the transport and node
    transport
        .listen("127.0.0.1:8443".to_string(), false)
        .await?;
    node.start().await?;

    info!("TLS transport started and listening on 127.0.0.1:8443");

    // Get node ID
    match node.id().await {
        Ok(id) => info!("Node ID: {}", id),
        Err(e) => error!("Failed to get node ID: {}", e),
    }

    // Add some test data
    node.add_contact("alice".to_string(), "alice_tls_pubkey".to_string())
        .await?;
    node.add_peer(
        "tls_peer".to_string(),
        true,
        "127.0.0.1:8444".to_string(),
        Some("secure".to_string()),
    )
    .await?;

    info!("Added test contacts and peers");

    // Display transport information
    info!("Transport name: {}", transport.name());
    info!("Transport byte limit: {}", transport.byte_limit());
    info!(
        "Transport JSON: {}",
        transport.to_json().unwrap_or("error".to_string())
    );

    info!("Running for 10 seconds...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    info!("Shutting down...");

    // Gracefully stop
    if let Err(e) = node.stop().await {
        error!("Error stopping node: {}", e);
    }

    if let Err(e) = transport.stop().await {
        error!("Error stopping transport: {}", e);
    }

    info!("TLS transport example completed");

    Ok(())
}

#[cfg(not(feature = "tls"))]
fn main() {
    println!("This example requires the 'tls' feature to be enabled.");
    println!("Run with: cargo run --example tls_transport --features tls");
}
