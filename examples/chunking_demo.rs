//! Chunking Demo - Demonstrates message chunking functionality
//!
//! This example shows how RatNet automatically chunks large messages
//! and reassembles them on the receiving end.

use bytes::Bytes;
use ratnet::api::{Msg, Node, PubKey, Transport};
use ratnet::nodes::MemoryNode;
use ratnet::policy::PollPolicy;
use ratnet::router::DefaultRouter;
use ratnet::transports::UdpTransport;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting RatNet Chunking Demo");

    // Initialize the ratnet components
    ratnet::init();

    // Create nodes
    let sender_node = Arc::new(MemoryNode::new());
    let receiver_node = Arc::new(MemoryNode::new());

    // Create transports with small byte limits to force chunking
    let sender_transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));
    let receiver_transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

    // Set small byte limits to demonstrate chunking (4KB chunks)
    sender_transport.set_byte_limit(4096);
    receiver_transport.set_byte_limit(4096);

    // Create routers
    let sender_router = Arc::new(DefaultRouter::new());
    let receiver_router = Arc::new(DefaultRouter::new());

    // Create policies
    let sender_policy = Arc::new(PollPolicy::new(
        sender_transport.clone(),
        sender_node.clone(),
    ));
    let receiver_policy = Arc::new(PollPolicy::new(
        receiver_transport.clone(),
        receiver_node.clone(),
    ));

    // Start nodes
    info!("Starting nodes...");

    sender_node.start().await?;
    receiver_node.start().await?;

    // Add policies to nodes (simplified for demo)
    // In a real implementation, nodes would manage policies differently

    // Set transport byte limits for chunking demo
    info!(
        "Sender transport byte limit: {}",
        sender_transport.byte_limit()
    );
    info!(
        "Receiver transport byte limit: {}",
        receiver_transport.byte_limit()
    );

    // Create a large message (20KB) that will need chunking
    let large_content = "A".repeat(20 * 1024); // 20KB message
    let large_message = Msg {
        name: "test-channel".to_string(),
        content: Bytes::from(large_content),
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: false,
        stream_header: false,
    };

    info!(
        "Sending large message: {} bytes",
        large_message.content.len()
    );
    info!(
        "With transport limit: {} bytes, this will require chunking",
        sender_transport.byte_limit()
    );

    // Calculate expected chunks
    let chunk_size = ratnet::api::chunking::calculate_chunk_size(sender_node.clone())
        .await
        .unwrap_or(4096);
    let expected_chunks = (large_message.content.len() as u32 + chunk_size - 1) / chunk_size;
    info!("Expected chunk size: {} bytes", chunk_size);
    info!("Expected number of chunks: {}", expected_chunks);

    // Send the large message (will be automatically chunked)
    sender_node.send_msg(large_message).await?;

    info!("Message sent (chunked automatically)");

    // Add a small message to show non-chunked behavior
    let small_message = Msg {
        name: "test-channel".to_string(),
        content: Bytes::from("Small message that won't be chunked"),
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: false,
        stream_header: false,
    };

    info!(
        "Sending small message: {} bytes",
        small_message.content.len()
    );
    sender_node.send_msg(small_message).await?;

    // Demonstrate manual chunking API
    info!("Demonstrating manual chunking API...");
    let manual_message = Msg {
        name: "manual-channel".to_string(),
        content: Bytes::from("B".repeat(15 * 1024)), // 15KB
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: false,
        stream_header: false,
    };

    // Force chunking with a smaller chunk size
    let custom_chunk_size = 2048; // 2KB chunks
    info!(
        "Force chunking {} byte message into {} byte chunks",
        manual_message.content.len(),
        custom_chunk_size
    );

    ratnet::api::chunking::send_chunked(sender_node.clone(), custom_chunk_size, manual_message)
        .await?;

    // Wait for processing
    sleep(Duration::from_secs(2)).await;

    info!("Chunking demo completed!");
    info!("Messages have been sent through the chunking system");
    info!("In a real network, chunked messages would be transmitted and reassembled automatically");

    Ok(())
}
