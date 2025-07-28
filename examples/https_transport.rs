//! HTTPS Transport Example
//! 
//! This example demonstrates how to use the HTTPS transport for
//! web-based communication in RatNet.

#[cfg(feature = "https")]
use ratnet::prelude::*;
#[cfg(feature = "https")]
use ratnet::{
    nodes::MemoryNode,
    router::DefaultRouter,
    transports::HttpsTransport,
    policy::PollPolicy,
    crypto::generate_test_cert,
};
#[cfg(feature = "https")]
use std::sync::Arc;
#[cfg(feature = "https")]
use tracing::{info, error};
#[cfg(feature = "https")]
use tracing_subscriber;

#[cfg(feature = "https")]
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting RatNet HTTPS Transport Example");
    
    // Initialize component registrations
    ratnet::init();
    
    // Generate test certificates
    let (cert_pem, key_pem) = generate_test_cert()
        .expect("Failed to generate test certificate");
    
    info!("Generated test certificate for HTTPS transport");
    
    // Create memory node
    let node = Arc::new(MemoryNode::new());
    
    // Create router and set it
    let router = Arc::new(DefaultRouter::new());
    node.set_router(router);
    
    // Create HTTPS transport with certificates
    let transport = Arc::new(HttpsTransport::new_with_certs(
        "127.0.0.1:8080".to_string(),
        cert_pem,
        key_pem,
        true, // ECC mode
        node.clone()
    ));
    
    // Create policy
    let policy = Arc::new(PollPolicy::new(transport.clone(), node.clone()));
    node.set_policy(vec![policy]);
    
    // Start the transport and node
    transport.listen("127.0.0.1:8080".to_string(), false).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    node.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    
    info!("HTTPS transport started and listening on 127.0.0.1:8080");
    
    // Get node ID
    let node_id = match node.id().await {
        Ok(id) => {
            info!("Node ID: {}", id);
            id
        },
        Err(e) => {
            error!("Failed to get node ID: {}", e);
            return Err(Box::new(e) as Box<dyn std::error::Error>);
        }
    };
    
    // Add some test data
    node.add_contact("bob".to_string(), "bob_https_pubkey".to_string()).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    node.add_peer("https_peer".to_string(), true, "127.0.0.1:8081".to_string(), Some("web".to_string())).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    
    info!("Added test contacts and peers");
    
    // Display transport information
    info!("Transport name: {}", transport.name());
    info!("Transport byte limit: {}", transport.byte_limit());
    info!("Transport JSON: {}", transport.to_json().unwrap_or("error".to_string()));
    
    // Wait a moment to ensure the server is ready
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Test RPC functionality
    info!("Testing RPC functionality...");
    let rpc_result = transport.rpc("http://127.0.0.1:8080", Action::ID, vec![]).await;
    match rpc_result {
        Ok(response) => {
            info!("RPC response: {:?}", response);
            // The response should be the node's ID
            if let Some(id_str) = response.as_str() {
                if id_str == node_id.to_string() {
                    info!("RPC test succeeded: returned node ID matches");
                } else {
                    error!("RPC test failed: returned ID does not match node's ID");
                }
            } else {
                error!("RPC test failed: response is not a string");
            }
        },
        Err(e) => error!("RPC call failed: {}", e),
    }
    
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
    
    info!("HTTPS transport example completed");
    info!("You can test the HTTP endpoint at: http://127.0.0.1:8080/");
    
    Ok(())
}

#[cfg(not(feature = "https"))]
fn main() {
    println!("This example requires the 'https' feature to be enabled.");
    println!("Run with: cargo run --example https_transport --features https");
} 