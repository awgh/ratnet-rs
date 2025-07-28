//! Policy demonstration example
//!
//! This example shows how to use the different policy types:
//! - Server policy: Simple listen-only policy
//! - P2P policy: Peer-to-peer discovery with mDNS

use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber;

use ratnet::api::{Node, Policy, JSON};
use ratnet::nodes::MemoryNode;
use ratnet::policy::{P2PPolicy, ServerPolicy};
use ratnet::router::DefaultRouter;
use ratnet::transports::UdpTransport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== RatNet Policy Demo ===\n");

    // Create a node
    let node = Arc::new(MemoryNode::new());
    let router = Arc::new(DefaultRouter::new());
    node.set_router(router);

    // Example 1: Server Policy
    println!("1. Server Policy Example");
    println!("   - Simple listen-only policy");
    println!("   - Starts transport in server mode");
    println!("   - Keeps running until stopped\n");

    let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));
    let server_policy = Arc::new(ServerPolicy::new(
        transport.clone(),
        "127.0.0.1:8080".to_string(),
        false, // admin_mode
    ));

    println!("   Server Policy Configuration:");
    println!("   - Listen URI: {}", server_policy.listen_uri());
    println!("   - Admin Mode: {}", server_policy.admin_mode());
    println!("   - Running: {}", server_policy.is_running());

    // Start the server policy
    println!("\n   Starting server policy...");
    server_policy.run_policy().await?;
    println!("   Server policy started successfully!");
    println!("   - Running: {}", server_policy.is_running());

    // Stop the server policy
    println!("\n   Stopping server policy...");
    server_policy.stop().await?;
    println!("   Server policy stopped successfully!");
    println!("   - Running: {}", server_policy.is_running());

    println!("\n{}", "=".repeat(50));
    println!();

    // Example 2: P2P Policy (if feature is enabled)
    #[cfg(feature = "p2p")]
    {
        println!("2. P2P Policy Example");
        println!("   - Peer-to-peer discovery with mDNS");
        println!("   - Automatic connection management");
        println!("   - Negotiation to prevent connection loops\n");

        let p2p_transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));
        let p2p_policy = Arc::new(P2PPolicy::new(
            p2p_transport.clone(),
            "127.0.0.1:8081".to_string(),
            node.clone(),
            false,                   // admin_mode
            Duration::from_secs(30), // listen_interval
            Duration::from_secs(10), // advertise_interval
        ));

        println!("   P2P Policy Configuration:");
        println!("   - Listen Interval: {:?}", p2p_policy.listen_interval());
        println!(
            "   - Advertise Interval: {:?}",
            p2p_policy.advertise_interval()
        );
        println!("   - Negotiation Rank: {}", p2p_policy.negotiation_rank());
        println!("   - Running: {}", p2p_policy.is_listening());

        // Try to start the P2P policy (might fail in test environment)
        println!("\n   Starting P2P policy...");
        match p2p_policy.run_policy().await {
            Ok(_) => {
                println!("   P2P policy started successfully!");
                println!("   - Listening: {}", p2p_policy.is_listening());
                println!("   - Advertising: {}", p2p_policy.is_advertising());

                // Let it run for a moment
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Stop the P2P policy
                println!("\n   Stopping P2P policy...");
                p2p_policy.stop().await?;
                println!("   P2P policy stopped successfully!");
                println!("   - Listening: {}", p2p_policy.is_listening());
                println!("   - Advertising: {}", p2p_policy.is_advertising());
            }
            Err(e) => {
                println!(
                    "   P2P policy start failed (expected in some environments): {:?}",
                    e
                );
                println!("   This is normal in test environments where socket binding may be restricted.");
            }
        }

        // P2P Policy JSON Serialization (inside the same block)
        println!("\n   P2P Policy JSON:");
        match p2p_policy.to_json() {
            Ok(json) => println!("   {}", json),
            Err(e) => println!("   Error serializing: {:?}", e),
        }
    }

    #[cfg(not(feature = "p2p"))]
    {
        println!("2. P2P Policy Example (not available)");
        println!("   - P2P policy requires the 'p2p' feature to be enabled");
        println!("   - Enable with: cargo run --example policy_demo --features p2p");
    }

    println!("\n{}", "=".repeat(50));
    println!();

    // Example 3: Policy JSON Serialization
    println!("3. Policy JSON Serialization");
    println!("   - Policies can be serialized to/from JSON");
    println!("   - Useful for configuration management\n");

    println!("   Server Policy JSON:");
    match server_policy.to_json() {
        Ok(json) => println!("   {}", json),
        Err(e) => println!("   Error serializing: {:?}", e),
    }

    #[cfg(not(feature = "p2p"))]
    {
        println!("\n   P2P Policy JSON: (not available - enable 'p2p' feature)");
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}
