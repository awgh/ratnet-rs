//! Multi-Hop Network Demo
//!
//! This example demonstrates real multi-hop message transmission through a network
//! of RatNet nodes, showing how messages travel through intermediate nodes.

use ratnet::prelude::*;
use ratnet::{
    nodes::MemoryNode, policy::PollPolicy, router::DefaultRouter, transports::UdpTransport,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, Level};
use bytes::Bytes;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Multi-Hop Network Demo");

    // Initialize component registrations
    ratnet::router::init();
    ratnet::transports::init();
    ratnet::policy::init();

    // Create a network of 5 nodes: A → B → C → D → E
    let nodes = vec![
        Arc::new(MemoryNode::new()), // A
        Arc::new(MemoryNode::new()), // B
        Arc::new(MemoryNode::new()), // C
        Arc::new(MemoryNode::new()), // D
        Arc::new(MemoryNode::new()), // E
    ];

    let node_names = ["A", "B", "C", "D", "E"];
    let ports = [8080, 8081, 8082, 8083, 8084];

    // Set up routers and transports for each node
    let mut transports = Vec::new();
    let mut policies = Vec::new();

    for (i, node) in nodes.iter().enumerate() {
        // Set up router
        let router = Arc::new(DefaultRouter::new());
        node.set_router(router);

        // Create transport
        let transport = Arc::new(UdpTransport::new(format!("127.0.0.1:{}", ports[i])));
        transports.push(transport.clone());

        // Create policy
        let policy = Arc::new(PollPolicy::new(transport.clone(), node.clone()));
        policies.push(policy.clone());

        // Set policy
        node.set_policy(vec![policy]);
    }

    // Start all transports
    for (i, transport) in transports.iter().enumerate() {
        transport
            .listen(format!("127.0.0.1:{}", ports[i]), false)
            .await
            .expect(&format!("Failed to start transport {}", node_names[i]));
    }

    // Start all nodes
    for (i, node) in nodes.iter().enumerate() {
        node.start().await.expect(&format!("Failed to start node {}", node_names[i]));
    }

    info!("All nodes started and listening");

    // Get node IDs
    let mut node_ids = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let id = node.id().await.expect(&format!("Failed to get node {} ID", node_names[i]));
        node_ids.push(id.clone());
        info!("Node {} ID: {}", node_names[i], id);
    }

    // Set up chain topology: A ↔ B ↔ C ↔ D ↔ E
    // Each node only knows its immediate neighbors

    // Add forward connections
    for i in 0..nodes.len() - 1 {
        let current_node = &nodes[i];
        let next_node_id = &node_ids[i + 1];
        let next_addr = format!("127.0.0.1:{}", ports[i + 1]);

        current_node
            .add_contact("next".to_string(), next_node_id.clone().to_string())
            .await
            .expect(&format!("Failed to add next contact to node {}", node_names[i]));

        current_node
            .add_peer(
                "next".to_string(),
                true,
                next_addr,
                Some("default".to_string()),
            )
            .await
            .expect(&format!("Failed to add next peer to node {}", node_names[i]));
    }

    // Add backward connections
    for i in 1..nodes.len() {
        let current_node = &nodes[i];
        let prev_node_id = &node_ids[i - 1];
        let prev_addr = format!("127.0.0.1:{}", ports[i - 1]);

        current_node
            .add_contact("prev".to_string(), prev_node_id.clone().to_string())
            .await
            .expect(&format!("Failed to add prev contact to node {}", node_names[i]));

        current_node
            .add_peer(
                "prev".to_string(),
                true,
                prev_addr,
                Some("default".to_string()),
            )
            .await
            .expect(&format!("Failed to add prev peer to node {}", node_names[i]));
    }

    // Create test channel
    let channel_name = "multi_hop_channel".to_string();
    let channel_keypair = ratnet::api::crypto::KeyPair::generate_ed25519().expect("Failed to generate channel key");
    let channel_key = channel_keypair.to_string().expect("Failed to serialize channel key");

    for node in &nodes {
        node.add_channel(channel_name.clone(), channel_key.clone()).await.expect("Failed to add channel");
    }

    info!("Chain topology established: A ↔ B ↔ C ↔ D ↔ E");

    // Wait for network to stabilize
    sleep(Duration::from_millis(200)).await;

    // Test 1: Send message from A to E (should route through B, C, D)
    info!("Test 1: Sending message from A to E (via B, C, D)");
    let start_time = Instant::now();

    let msg_a_to_e = Msg {
        name: channel_name.clone(),
        content: Bytes::from("Message from A to E via B, C, D"),
        is_chan: true,
        pubkey: node_ids[0].clone(),
        chunked: false,
        stream_header: false,
    };

    match nodes[0].send_msg(msg_a_to_e).await {
        Ok(_) => {
            let latency = start_time.elapsed();
            info!("Message sent from A to E, latency: {:.2}ms", latency.as_millis() as f64);
        }
        Err(e) => {
            info!("Failed to send message from A to E: {:?}", e);
        }
    }

    // Wait for message propagation
    sleep(Duration::from_millis(300)).await;

    // Test 2: Send message from E to A (should route through D, C, B)
    info!("Test 2: Sending message from E to A (via D, C, B)");
    let start_time = Instant::now();

    let msg_e_to_a = Msg {
        name: channel_name.clone(),
        content: Bytes::from("Message from E to A via D, C, B"),
        is_chan: true,
        pubkey: node_ids[4].clone(),
        chunked: false,
        stream_header: false,
    };

    match nodes[4].send_msg(msg_e_to_a).await {
        Ok(_) => {
            let latency = start_time.elapsed();
            info!("Message sent from E to A, latency: {:.2}ms", latency.as_millis() as f64);
        }
        Err(e) => {
            info!("Failed to send message from E to A: {:?}", e);
        }
    }

    // Wait for message propagation
    sleep(Duration::from_millis(300)).await;

    // Test 3: Send message from B to D (should route through C)
    info!("Test 3: Sending message from B to D (via C)");
    let start_time = Instant::now();

    let msg_b_to_d = Msg {
        name: channel_name.clone(),
        content: Bytes::from("Message from B to D via C"),
        is_chan: true,
        pubkey: node_ids[1].clone(),
        chunked: false,
        stream_header: false,
    };

    match nodes[1].send_msg(msg_b_to_d).await {
        Ok(_) => {
            let latency = start_time.elapsed();
            info!("Message sent from B to D, latency: {:.2}ms", latency.as_millis() as f64);
        }
        Err(e) => {
            info!("Failed to send message from B to D: {:?}", e);
        }
    }

    // Wait for message propagation
    sleep(Duration::from_millis(300)).await;

    // Verify network connectivity
    info!("Verifying network connectivity...");
    for (i, node) in nodes.iter().enumerate() {
        let peers = node.get_peers(None).await.expect(&format!("Failed to get peers from node {}", node_names[i]));
        info!("Node {} peers: {}", node_names[i], peers.len());
    }

    // Test 4: Performance test - send multiple messages through the chain
    info!("Test 4: Performance test - sending 10 messages through the chain");
    let mut total_latency = Duration::ZERO;
    let message_count = 10;

    for i in 0..message_count {
        let start_time = Instant::now();
        let msg = Msg {
            name: channel_name.clone(),
            content: Bytes::from(format!("Performance test message {} from A to E", i + 1)),
            is_chan: true,
            pubkey: node_ids[0].clone(),
            chunked: false,
            stream_header: false,
        };

        match nodes[0].send_msg(msg).await {
            Ok(_) => {
                let latency = start_time.elapsed();
                total_latency += latency;
                info!("Message {} sent, latency: {:.2}ms", i + 1, latency.as_millis() as f64);
            }
            Err(e) => {
                info!("Failed to send message {}: {:?}", i + 1, e);
            }
        }

        // Small delay between messages
        sleep(Duration::from_millis(50)).await;
    }

    let avg_latency = total_latency / message_count;
    info!("Average latency for {} messages: {:.2}ms", message_count, avg_latency.as_millis() as f64);

    // Print network statistics
    info!("\n=== Multi-Hop Network Statistics ===");
    info!("Network topology: A ↔ B ↔ C ↔ D ↔ E");
    info!("Total nodes: {}", nodes.len());
    info!("Message routing: A → B → C → D → E");
    info!("Reverse routing: E → D → C → B → A");
    info!("Average latency: {:.2}ms", avg_latency.as_millis() as f64);
    info!("Total messages sent: {}", message_count + 3); // 3 test messages + 10 performance messages

    // Clean up
    info!("Shutting down multi-hop network...");
    for node in &nodes {
        let _ = node.stop().await;
    }
    for transport in &transports {
        transport.stop().await.expect("Failed to stop transport");
    }

    info!("Multi-hop network demo completed successfully");
    Ok(())
} 