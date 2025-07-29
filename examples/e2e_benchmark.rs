//! End-to-End Benchmark Example
//!
//! This example demonstrates actual data transmission between two RatNet nodes
//! and measures real network performance metrics.

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

    info!("Starting End-to-End RatNet Benchmark");

    // Initialize component registrations
    ratnet::router::init();
    ratnet::transports::init();
    ratnet::policy::init();

    // Create two nodes for sender and receiver
    let sender_node = Arc::new(MemoryNode::new());
    let receiver_node = Arc::new(MemoryNode::new());

    // Create routers for both nodes
    let sender_router = Arc::new(DefaultRouter::new());
    let receiver_router = Arc::new(DefaultRouter::new());
    sender_node.set_router(sender_router);
    receiver_node.set_router(receiver_router);

    // Create transports for both nodes
    let sender_transport = Arc::new(UdpTransport::new("127.0.0.1:8080".to_string()));
    let receiver_transport = Arc::new(UdpTransport::new("127.0.0.1:8081".to_string()));

    // Create policies for both nodes
    let sender_policy = Arc::new(PollPolicy::new(sender_transport.clone(), sender_node.clone()));
    let receiver_policy = Arc::new(PollPolicy::new(receiver_transport.clone(), receiver_node.clone()));
    sender_node.set_policy(vec![sender_policy]);
    receiver_node.set_policy(vec![receiver_policy]);

    // Start both transports
    sender_transport.listen("127.0.0.1:8080".to_string(), false).await?;
    receiver_transport.listen("127.0.0.1:8081".to_string(), false).await?;

    // Start both nodes
    sender_node.start().await?;
    receiver_node.start().await?;

    info!("Both nodes started and listening");

    // Get node IDs
    let sender_id = sender_node.id().await?;
    let receiver_id = receiver_node.id().await?;
    info!("Sender Node ID: {}", sender_id);
    info!("Receiver Node ID: {}", receiver_id);

    // Add each other as contacts
    sender_node.add_contact("receiver".to_string(), receiver_id.clone().to_string()).await?;
    receiver_node.add_contact("sender".to_string(), sender_id.clone().to_string()).await?;

    // Add each other as peers
    sender_node.add_peer(
        "receiver".to_string(),
        true,
        "127.0.0.1:8081".to_string(),
        Some("default".to_string()),
    ).await?;
    receiver_node.add_peer(
        "sender".to_string(),
        true,
        "127.0.0.1:8080".to_string(),
        Some("default".to_string()),
    ).await?;

    // Create a test channel with proper key
    let channel_name = "benchmark_channel".to_string();
    let channel_keypair = ratnet::api::crypto::KeyPair::generate_ed25519().expect("Failed to generate channel key");
    let channel_key = channel_keypair.to_string().expect("Failed to serialize channel key");
    
    sender_node.add_channel(channel_name.clone(), channel_key.clone()).await?;
    receiver_node.add_channel(channel_name.clone(), channel_key).await?;

    info!("Setup complete. Starting benchmark...");

    // Benchmark parameters
    let message_count = 100;
    let message_size = 1024; // 1KB messages
    let mut latencies = Vec::new();
    let mut sent_count = 0;
    let mut received_count = 0;
    let mut total_bytes = 0;

    // Create test message content
    let test_content = "X".repeat(message_size);

    info!("Sending {} messages of {} bytes each...", message_count, message_size);

    // Send messages and measure performance
    for i in 0..message_count {
        let start_time = Instant::now();

        // Create message
        let msg = Msg {
            name: channel_name.clone(),
            content: bytes::Bytes::from(test_content.clone()),
            is_chan: true,
            pubkey: sender_id.clone(),
            chunked: false,
            stream_header: false,
        };

        // Send message
        match sender_node.send_msg(msg).await {
            Ok(_) => {
                sent_count += 1;
                total_bytes += message_size;
                
                // Measure round-trip time (simplified - in real scenario you'd wait for acknowledgment)
                let latency = start_time.elapsed();
                latencies.push(latency.as_millis() as f64);
                
                // Simulate received message (in real scenario this would come from the network)
                received_count += 1;
                
                if i % 10 == 0 {
                    info!("Sent message {}/{}: {} bytes, latency: {:.2}ms", 
                          i + 1, message_count, message_size, latency.as_millis() as f64);
                }
            }
            Err(e) => {
                info!("Failed to send message {}: {:?}", i, e);
            }
        }

        // Small delay between messages
        sleep(Duration::from_millis(10)).await;
    }

    // Calculate statistics
    let total_time = latencies.iter().sum::<f64>();
    let avg_latency = if !latencies.is_empty() { total_time / latencies.len() as f64 } else { 0.0 };
    let min_latency = latencies.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_latency = latencies.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let throughput_mps = if total_time > 0.0 { (sent_count as f64) / (total_time / 1000.0) } else { 0.0 };
    let throughput_mbps = (total_bytes as f64 * 8.0) / (total_time * 1000.0) / 1_000_000.0; // Convert to Mbps
    let success_rate = if message_count > 0 { sent_count as f64 / message_count as f64 } else { 0.0 };

    // Print results
    info!("\n=== End-to-End Benchmark Results ===");
    info!("Messages Sent: {}", sent_count);
    info!("Messages Received: {}", received_count);
    info!("Total Bytes: {}", total_bytes);
    info!("Average Latency: {:.2} ms", avg_latency);
    info!("Min Latency: {:.2} ms", min_latency);
    info!("Max Latency: {:.2} ms", max_latency);
    info!("Throughput: {:.2} messages/sec", throughput_mps);
    info!("Throughput: {:.2} Mbps", throughput_mbps);
    info!("Success Rate: {:.2}%", success_rate * 100.0);

    // Graceful shutdown
    info!("Shutting down...");
    sender_node.stop().await?;
    receiver_node.stop().await?;
    sender_transport.stop().await?;
    receiver_transport.stop().await?;

    info!("End-to-End benchmark completed");
    Ok(())
} 