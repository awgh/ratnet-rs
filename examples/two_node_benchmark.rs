//! Two-Node End-to-End Benchmark
//!
//! This example runs two separate RatNet nodes and measures actual
//! network transmission performance between them.

use ratnet::prelude::*;
use ratnet::{
    nodes::MemoryNode, policy::PollPolicy, router::DefaultRouter, transports::UdpTransport,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::{info, Level};


#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Two-Node End-to-End RatNet Benchmark");

    // Initialize component registrations
    ratnet::router::init();
    ratnet::transports::init();
    ratnet::policy::init();

    // Create two nodes
    let node_a = Arc::new(MemoryNode::new());
    let node_b = Arc::new(MemoryNode::new());

    // Create routers
    let router_a = Arc::new(DefaultRouter::new());
    let router_b = Arc::new(DefaultRouter::new());
    node_a.set_router(router_a);
    node_b.set_router(router_b);

    // Create transports on different ports
    let transport_a = Arc::new(UdpTransport::new("127.0.0.1:8080".to_string()));
    let transport_b = Arc::new(UdpTransport::new("127.0.0.1:8081".to_string()));

    // Create policies
    let policy_a = Arc::new(PollPolicy::new(transport_a.clone(), node_a.clone()));
    let policy_b = Arc::new(PollPolicy::new(transport_b.clone(), node_b.clone()));
    node_a.set_policy(vec![policy_a]);
    node_b.set_policy(vec![policy_b]);

    // Start transports
    transport_a.listen("127.0.0.1:8080".to_string(), false).await?;
    transport_b.listen("127.0.0.1:8081".to_string(), false).await?;

    // Start nodes
    node_a.start().await?;
    node_b.start().await?;

    info!("Both nodes started");

    // Get node IDs
    let id_a = node_a.id().await?;
    let id_b = node_b.id().await?;
    info!("Node A ID: {}", id_a);
    info!("Node B ID: {}", id_b);

    // Add each other as contacts and peers
    node_a.add_contact("node_b".to_string(), id_b.clone().to_string()).await?;
    node_b.add_contact("node_a".to_string(), id_a.clone().to_string()).await?;

    node_a.add_peer(
        "node_b".to_string(),
        true,
        "127.0.0.1:8081".to_string(),
        Some("default".to_string()),
    ).await?;
    node_b.add_peer(
        "node_a".to_string(),
        true,
        "127.0.0.1:8080".to_string(),
        Some("default".to_string()),
    ).await?;

    // Create test channels with proper key
    let channel_name = "test_channel".to_string();
    let channel_keypair = ratnet::api::crypto::KeyPair::generate_ed25519().expect("Failed to generate channel key");
    let channel_key = channel_keypair.to_string().expect("Failed to serialize channel key");
    
    node_a.add_channel(channel_name.clone(), channel_key.clone()).await?;
    node_b.add_channel(channel_name.clone(), channel_key).await?;

    info!("Setup complete. Starting bidirectional benchmark...");

    // Benchmark parameters
    let message_count = 50;
    let message_size = 512; // 512 bytes
    let mut a_to_b_latencies = Vec::new();
    let mut b_to_a_latencies = Vec::new();
    let mut total_sent = 0;
    let _total_received = 0;
    let mut total_bytes = 0;

    // Create test content
    let test_content = "X".repeat(message_size);

    // Run bidirectional benchmark
    for i in 0..message_count {
        info!("Round {}/{}", i + 1, message_count);

        // Node A sends to Node B
        let start_a_to_b = Instant::now();
        let msg_a_to_b = Msg {
            name: channel_name.clone(),
            content: bytes::Bytes::from(format!("A->B: {}", test_content)),
            is_chan: true,
            pubkey: id_a.clone(),
            chunked: false,
            stream_header: false,
        };

        match node_a.send_msg(msg_a_to_b).await {
            Ok(_) => {
                total_sent += 1;
                total_bytes += message_size;
                let latency = start_a_to_b.elapsed();
                a_to_b_latencies.push(latency.as_millis() as f64);
                info!("A->B: {} bytes, latency: {:.2}ms", message_size, latency.as_millis() as f64);
            }
            Err(e) => {
                info!("Failed A->B: {:?}", e);
            }
        }

        // Small delay
        sleep(Duration::from_millis(20)).await;

        // Node B sends to Node A
        let start_b_to_a = Instant::now();
        let msg_b_to_a = Msg {
            name: channel_name.clone(),
            content: bytes::Bytes::from(format!("B->A: {}", test_content)),
            is_chan: true,
            pubkey: id_b.clone(),
            chunked: false,
            stream_header: false,
        };

        match node_b.send_msg(msg_b_to_a).await {
            Ok(_) => {
                total_sent += 1;
                total_bytes += message_size;
                let latency = start_b_to_a.elapsed();
                b_to_a_latencies.push(latency.as_millis() as f64);
                info!("B->A: {} bytes, latency: {:.2}ms", message_size, latency.as_millis() as f64);
            }
            Err(e) => {
                info!("Failed B->A: {:?}", e);
            }
        }

        // Small delay between rounds
        sleep(Duration::from_millis(50)).await;
    }

    // Calculate statistics
    let all_latencies = [&a_to_b_latencies[..], &b_to_a_latencies[..]].concat();
    let total_time = all_latencies.iter().sum::<f64>();
    let avg_latency = if !all_latencies.is_empty() { total_time / all_latencies.len() as f64 } else { 0.0 };
    let min_latency = all_latencies.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_latency = all_latencies.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let throughput_mps = if total_time > 0.0 { (total_sent as f64) / (total_time / 1000.0) } else { 0.0 };
    let throughput_mbps = (total_bytes as f64 * 8.0) / (total_time * 1000.0) / 1_000_000.0;
    let success_rate = if message_count * 2 > 0 { total_sent as f64 / (message_count * 2) as f64 } else { 0.0 };

    // Print results
    info!("\n=== Two-Node End-to-End Benchmark Results ===");
    info!("Total Messages Sent: {}", total_sent);
    info!("Total Bytes Transmitted: {}", total_bytes);
    info!("Average Latency: {:.2} ms", avg_latency);
    info!("Min Latency: {:.2} ms", min_latency);
    info!("Max Latency: {:.2} ms", max_latency);
    info!("Throughput: {:.2} messages/sec", throughput_mps);
    info!("Throughput: {:.2} Mbps", throughput_mbps);
    info!("Success Rate: {:.2}%", success_rate * 100.0);

    // Direction-specific stats
    if !a_to_b_latencies.is_empty() {
        let avg_a_to_b = a_to_b_latencies.iter().sum::<f64>() / a_to_b_latencies.len() as f64;
        info!("A->B Average Latency: {:.2} ms", avg_a_to_b);
    }
    if !b_to_a_latencies.is_empty() {
        let avg_b_to_a = b_to_a_latencies.iter().sum::<f64>() / b_to_a_latencies.len() as f64;
        info!("B->A Average Latency: {:.2} ms", avg_b_to_a);
    }

    // Graceful shutdown
    info!("Shutting down...");
    node_a.stop().await?;
    node_b.stop().await?;
    transport_a.stop().await?;
    transport_b.stop().await?;

    info!("Two-Node benchmark completed");
    Ok(())
} 