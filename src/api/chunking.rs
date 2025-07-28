//! Message chunking functionality for handling large messages
//!
//! This module provides utilities to break large messages into smaller chunks
//! for transports that have size limitations, and reassemble them on the receiving end.

use crate::api::{Msg, Node};
use crate::error::{RatNetError, Result};
use bytes::{BufMut, Bytes, BytesMut};
use std::sync::Arc;
use tracing::{debug, trace};

/// Default minimum chunk size (64KB)
pub const DEFAULT_CHUNK_SIZE: u32 = 64 * 1024;

/// Overhead for encryption and transport headers
pub const CHUNK_OVERHEAD: u32 = 132; // ECC overhead

/// Header size for chunk metadata (stream_id + chunk_num)
pub const CHUNK_HEADER_SIZE: u32 = 8;

/// Calculate the optimal chunk size based on all active transport limits
pub async fn calculate_chunk_size(node: Arc<dyn Node>) -> Result<u32> {
    let mut chunk_size = DEFAULT_CHUNK_SIZE;

    // Get all policies and their transport limits
    let policies = node.get_policies();
    for policy in policies {
        let transport = policy.get_transport();
        let limit = transport.byte_limit() as u32;

        if limit > 0 && limit < chunk_size {
            chunk_size = limit;
        }
    }

    // Ensure we have enough space for headers and overhead
    if chunk_size <= CHUNK_OVERHEAD + CHUNK_HEADER_SIZE {
        return Err(RatNetError::InvalidArgument(format!(
            "Transport byte limit {chunk_size} too small for chunking"
        )));
    }

    // Subtract overhead to get actual payload size per chunk
    Ok(chunk_size - CHUNK_OVERHEAD)
}

/// Send a message, automatically chunking it if necessary
pub async fn send_chunked(node: Arc<dyn Node>, chunk_size: u32, msg: Msg) -> Result<()> {
    let content = msg.content.as_ref();
    let content_len = content.len() as u32;

    // Calculate chunk parameters
    let chunk_payload_size = chunk_size.saturating_sub(CHUNK_HEADER_SIZE);

    if chunk_payload_size == 0 {
        return Err(RatNetError::InvalidArgument(
            "Chunk size too small".to_string(),
        ));
    }

    let whole_chunks = content_len / chunk_payload_size;
    let remainder = content_len % chunk_payload_size;
    let total_chunks = if remainder > 0 {
        whole_chunks + 1
    } else {
        whole_chunks
    };

    // If message fits in one chunk, send normally
    if total_chunks <= 1 && content_len <= chunk_payload_size {
        return node.send_msg(msg).await;
    }

    debug!(
        "Chunking message: {} bytes into {} chunks",
        content_len, total_chunks
    );

    // Generate random stream ID
    let stream_id = generate_stream_id()?;

    // Send stream header first
    send_stream_header(node.clone(), &msg, stream_id, total_chunks).await?;

    // Send all chunks
    for chunk_num in 0..whole_chunks {
        let start = (chunk_num * chunk_payload_size) as usize;
        let end = ((chunk_num + 1) * chunk_payload_size) as usize;
        let chunk_data = &content[start..end];

        send_chunk(node.clone(), &msg, stream_id, chunk_num, chunk_data).await?;
    }

    // Send remainder chunk if needed
    if remainder > 0 {
        let start = (whole_chunks * chunk_payload_size) as usize;
        let chunk_data = &content[start..];

        send_chunk(node.clone(), &msg, stream_id, whole_chunks, chunk_data).await?;
    }

    Ok(())
}

/// Send a stream header message
async fn send_stream_header(
    node: Arc<dyn Node>,
    original_msg: &Msg,
    stream_id: u32,
    total_chunks: u32,
) -> Result<()> {
    let mut header = BytesMut::new();

    // Write stream ID and total chunks
    header.put_u32_le(stream_id);
    header.put_u32_le(total_chunks);

    let header_msg = Msg {
        name: original_msg.name.clone(),
        content: header.freeze(),
        is_chan: original_msg.is_chan,
        pubkey: original_msg.pubkey.clone(),
        chunked: true,
        stream_header: true,
    };

    trace!(
        "Sending stream header: stream_id={:08x}, total_chunks={}",
        stream_id,
        total_chunks
    );
    node.send_msg(header_msg).await
}

/// Send a single chunk
async fn send_chunk(
    node: Arc<dyn Node>,
    original_msg: &Msg,
    stream_id: u32,
    chunk_num: u32,
    data: &[u8],
) -> Result<()> {
    let mut chunk_content = BytesMut::new();

    // Write chunk header (stream ID + chunk number)
    chunk_content.put_u32_le(stream_id);
    chunk_content.put_u32_le(chunk_num);

    // Write chunk data
    chunk_content.put_slice(data);

    let chunk_msg = Msg {
        name: original_msg.name.clone(),
        content: chunk_content.freeze(),
        is_chan: original_msg.is_chan,
        pubkey: original_msg.pubkey.clone(),
        chunked: true,
        stream_header: false,
    };

    trace!(
        "Sending chunk: stream_id={:08x}, chunk_num={}, size={}",
        stream_id,
        chunk_num,
        data.len()
    );

    node.send_msg(chunk_msg).await
}

/// Handle a chunked message (either stream header or chunk)
pub async fn handle_chunked(node: Arc<dyn Node>, msg: Msg) -> Result<()> {
    if msg.stream_header {
        handle_stream_header(node, msg).await
    } else {
        handle_chunk(node, msg).await
    }
}

/// Handle a stream header message
async fn handle_stream_header(node: Arc<dyn Node>, msg: Msg) -> Result<()> {
    let content = msg.content.as_ref();

    if content.len() < 8 {
        return Err(RatNetError::Serialization(
            "Stream header too short".to_string(),
        ));
    }

    let stream_id = u32::from_le_bytes([content[0], content[1], content[2], content[3]]);
    let total_chunks = u32::from_le_bytes([content[4], content[5], content[6], content[7]]);

    let channel_name = if msg.is_chan {
        msg.name.clone()
    } else {
        String::new()
    };

    debug!(
        "Received stream header: stream_id={:08x}, total_chunks={}, channel='{}'",
        stream_id, total_chunks, channel_name
    );

    node.add_stream(stream_id, total_chunks, channel_name).await
}

/// Handle a chunk message
async fn handle_chunk(node: Arc<dyn Node>, msg: Msg) -> Result<()> {
    let content = msg.content.as_ref();

    if content.len() < 8 {
        return Err(RatNetError::Serialization(
            "Chunk message too short".to_string(),
        ));
    }

    let stream_id = u32::from_le_bytes([content[0], content[1], content[2], content[3]]);
    let chunk_num = u32::from_le_bytes([content[4], content[5], content[6], content[7]]);
    let chunk_data = Bytes::from(content[8..].to_vec());

    debug!(
        "Received chunk: stream_id={:08x}, chunk_num={}, size={}",
        stream_id,
        chunk_num,
        chunk_data.len()
    );

    node.add_chunk(stream_id, chunk_num, chunk_data).await?;

    // Check if stream is complete and reassemble if needed
    try_reassemble_stream(node, stream_id).await
}

/// Attempt to reassemble a complete stream
async fn try_reassemble_stream(node: Arc<dyn Node>, stream_id: u32) -> Result<()> {
    // This is implemented in the node itself as it needs access to stream state
    // The node will check if all chunks are available and reassemble if complete
    node.check_stream_complete(stream_id).await
}

/// Generate a random 32-bit stream ID
pub fn generate_stream_id() -> Result<u32> {
    use crate::api::crypto::random_bytes;

    let bytes = random_bytes(4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::nodes::MemoryNode;

    use std::sync::Arc;

    #[tokio::test]
    async fn test_chunk_size_calculation() {
        let node = Arc::new(MemoryNode::new());
        let chunk_size = calculate_chunk_size(node).await.unwrap();

        // Should be reasonable chunk size
        assert!(chunk_size > 0);
        assert!(chunk_size <= DEFAULT_CHUNK_SIZE);
    }

    #[test]
    fn test_generate_stream_id() {
        let id1 = generate_stream_id().unwrap();
        let id2 = generate_stream_id().unwrap();

        // IDs should be different (very high probability)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_chunk_parameters() {
        let content_size = 100000u32; // 100KB
        let chunk_payload_size = 8192u32; // 8KB chunks

        let whole_chunks = content_size / chunk_payload_size;
        let remainder = content_size % chunk_payload_size;
        let total_chunks = if remainder > 0 {
            whole_chunks + 1
        } else {
            whole_chunks
        };

        assert_eq!(whole_chunks, 12); // 100000 / 8192 = 12
        assert_eq!(remainder, 1696); // 100000 % 8192 = 1696
        assert_eq!(total_chunks, 13); // 12 + 1 = 13
    }
}
