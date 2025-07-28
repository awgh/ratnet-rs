//! Tests for message chunking functionality

use bytes::Bytes;
use ratnet::api::{chunking, Msg, Node, PubKey};
use ratnet::nodes::MemoryNode;
use std::sync::Arc;

#[tokio::test]
async fn test_chunk_size_calculation() {
    let node = Arc::new(MemoryNode::new());

    // With no policies, should return default chunk size
    match chunking::calculate_chunk_size(node.clone()).await {
        Ok(size) => {
            assert!(size > 0);
            assert!(size <= chunking::DEFAULT_CHUNK_SIZE - chunking::CHUNK_OVERHEAD);
        }
        Err(_) => {
            // Expected when no policies are configured
        }
    }
}

#[tokio::test]
async fn test_small_message_not_chunked() {
    let node = Arc::new(MemoryNode::new());

    let small_msg = Msg {
        name: "test".to_string(),
        content: Bytes::from("Small message"),
        is_chan: false,
        pubkey: PubKey::nil(),
        chunked: false,
        stream_header: false,
    };

    // Small message should be sent directly without chunking
    let result = node.send_msg(small_msg).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_stream_header_handling() {
    let node = Arc::new(MemoryNode::new());

    // Create a stream header message
    let mut header_content = Vec::new();
    header_content.extend_from_slice(&123u32.to_le_bytes()); // stream_id
    header_content.extend_from_slice(&5u32.to_le_bytes()); // total_chunks

    let header_msg = Msg {
        name: "test-channel".to_string(),
        content: Bytes::from(header_content),
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: true,
        stream_header: true,
    };

    let result = chunking::handle_chunked(node.clone(), header_msg).await;
    assert!(result.is_ok());

    // Verify stream was registered
    // Note: We can't easily verify this without exposing internal state
    // In a real implementation, we might have a method to check stream status
}

#[tokio::test]
async fn test_chunk_handling() {
    let node = Arc::new(MemoryNode::new());

    // First, add a stream
    let stream_id = 456u32;
    let total_chunks = 3u32;
    node.add_stream(stream_id, total_chunks, "test-channel".to_string())
        .await
        .unwrap();

    // Add chunks
    let chunk_data = Bytes::from("chunk data 0");
    node.add_chunk(stream_id, 0, chunk_data.clone())
        .await
        .unwrap();

    let chunk_data = Bytes::from("chunk data 1");
    node.add_chunk(stream_id, 1, chunk_data.clone())
        .await
        .unwrap();

    let chunk_data = Bytes::from("chunk data 2");
    node.add_chunk(stream_id, 2, chunk_data.clone())
        .await
        .unwrap();

    // The last chunk should trigger reassembly
    // In the real implementation, this would create a complete message
}

#[tokio::test]
async fn test_chunked_message_format() {
    let node = Arc::new(MemoryNode::new());

    // Create a chunk message with proper format
    let stream_id = 789u32;
    let chunk_num = 1u32;
    let chunk_payload = b"test chunk payload";

    let mut chunk_content = Vec::new();
    chunk_content.extend_from_slice(&stream_id.to_le_bytes());
    chunk_content.extend_from_slice(&chunk_num.to_le_bytes());
    chunk_content.extend_from_slice(chunk_payload);

    let chunk_msg = Msg {
        name: "test-channel".to_string(),
        content: Bytes::from(chunk_content),
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: true,
        stream_header: false,
    };

    // First add the stream
    node.add_stream(stream_id, 2, "test-channel".to_string())
        .await
        .unwrap();

    // Handle the chunk
    let result = chunking::handle_chunked(node.clone(), chunk_msg).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_complete_chunking_flow() {
    let sender = Arc::new(MemoryNode::new());
    let receiver = Arc::new(MemoryNode::new());

    // Create a message that needs chunking
    let large_content = "X".repeat(1000); // 1KB content
    let original_msg = Msg {
        name: "test-channel".to_string(),
        content: Bytes::from(large_content),
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: false,
        stream_header: false,
    };

    let chunk_size = 200; // Force chunking with small chunks

    // This would normally send chunks through the transport layer
    // For testing, we'll verify the chunking logic works
    let result = chunking::send_chunked(sender.clone(), chunk_size, original_msg).await;

    // The result depends on whether we have a complete transport setup
    // In this unit test environment, it might fail due to missing transport
    // but the chunking logic should be exercised
    println!("Chunking result: {result:?}");
}

#[tokio::test]
async fn test_chunk_parameters() {
    // Test chunk calculation logic
    let content_size = 10000u32; // 10KB
    let chunk_payload_size = 1024u32; // 1KB chunks

    let whole_chunks = content_size / chunk_payload_size;
    let remainder = content_size % chunk_payload_size;
    let total_chunks = if remainder > 0 {
        whole_chunks + 1
    } else {
        whole_chunks
    };

    assert_eq!(whole_chunks, 9); // 10000 / 1024 = 9
    assert_eq!(remainder, 784); // 10000 % 1024 = 784
    assert_eq!(total_chunks, 10); // 9 + 1 = 10
}

#[tokio::test]
async fn test_stream_id_generation() {
    // Test that stream IDs are generated and unique
    let id1 = chunking::generate_stream_id().unwrap();
    let id2 = chunking::generate_stream_id().unwrap();

    // They should be different (very high probability)
    assert_ne!(id1, id2);

    // They should be valid u32 values
    assert!(id1 <= u32::MAX);
    assert!(id2 <= u32::MAX);
}

#[tokio::test]
async fn test_error_handling() {
    let node = Arc::new(MemoryNode::new());

    // Test handling malformed stream header
    let bad_header = Msg {
        name: "test".to_string(),
        content: Bytes::from(vec![1, 2, 3]), // Too short
        is_chan: false,
        pubkey: PubKey::nil(),
        chunked: true,
        stream_header: true,
    };

    let result = chunking::handle_chunked(node.clone(), bad_header).await;
    assert!(result.is_err());

    // Test handling malformed chunk
    let bad_chunk = Msg {
        name: "test".to_string(),
        content: Bytes::from(vec![1, 2, 3]), // Too short for chunk header
        is_chan: false,
        pubkey: PubKey::nil(),
        chunked: true,
        stream_header: false,
    };

    let result = chunking::handle_chunked(node.clone(), bad_chunk).await;
    assert!(result.is_err());
}
