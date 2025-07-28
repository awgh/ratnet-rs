//! Default router implementation

use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

use crate::api::{
    chunking, Msg, Node, Patch, Router, CHANNEL_FLAG, CHUNKED_FLAG, JSON, STREAM_HEADER_FLAG,
};
use crate::error::{RatNetError, Result};

/// Default router implementation
#[derive(Debug)]
pub struct DefaultRouter {
    patches: Arc<RwLock<Vec<Patch>>>,
}

impl DefaultRouter {
    /// Create a new default router
    pub fn new() -> Self {
        Self {
            patches: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Extract channel name from message if it has channel flag
    fn extract_channel_name(data: &[u8]) -> Result<(Option<String>, &[u8])> {
        if data.is_empty() {
            return Ok((None, data));
        }

        let flags = data[0];
        let mut offset = 1;

        if flags & CHANNEL_FLAG != 0 {
            if data.len() < 3 {
                return Err(RatNetError::Serialization(
                    "Invalid channel message format".to_string(),
                ));
            }

            let name_len = u16::from_be_bytes([data[1], data[2]]) as usize;
            offset += 2;

            if data.len() < offset + name_len {
                return Err(RatNetError::Serialization(
                    "Channel name length exceeds message size".to_string(),
                ));
            }

            let channel_name = String::from_utf8(data[offset..offset + name_len].to_vec())
                .map_err(|e| {
                    RatNetError::Serialization(format!("Invalid UTF-8 in channel name: {e}"))
                })?;

            offset += name_len;

            Ok((Some(channel_name), &data[offset..]))
        } else {
            Ok((None, &data[1..]))
        }
    }

    /// Handle stream header message
    #[allow(dead_code)]
    async fn handle_stream_header(&self, node: Arc<dyn Node>, data: &[u8]) -> Result<()> {
        if data.len() < 9 {
            return Err(RatNetError::Serialization(
                "Stream header too short".to_string(),
            ));
        }

        let stream_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let num_chunks = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let channel_name_len = data[8] as usize;

        if data.len() < 9 + channel_name_len {
            return Err(RatNetError::Serialization(
                "Stream header channel name truncated".to_string(),
            ));
        }

        let channel_name =
            String::from_utf8(data[9..9 + channel_name_len].to_vec()).map_err(|e| {
                RatNetError::Serialization(format!("Invalid UTF-8 in channel name: {e}"))
            })?;

        debug!(
            "Received stream header: stream_id={}, num_chunks={}, channel={}",
            stream_id, num_chunks, channel_name
        );

        node.add_stream(stream_id, num_chunks, channel_name).await
    }

    /// Handle chunk message
    #[allow(dead_code)]
    async fn handle_chunk(&self, node: Arc<dyn Node>, data: &[u8]) -> Result<()> {
        if data.len() < 8 {
            return Err(RatNetError::Serialization(
                "Chunk message too short".to_string(),
            ));
        }

        let stream_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let chunk_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let chunk_data = Bytes::from(data[8..].to_vec());

        debug!(
            "Received chunk: stream_id={}, chunk_num={}, size={}",
            stream_id,
            chunk_num,
            chunk_data.len()
        );

        node.add_chunk(stream_id, chunk_num, chunk_data).await
    }

    /// Apply patches to route messages between channels
    async fn apply_patches(
        &self,
        channel_name: &str,
        node: Arc<dyn Node>,
        content: Bytes,
    ) -> Result<()> {
        let patches = self.patches.read().await;
        let mut routed = false;

        for patch in patches.iter() {
            if patch.from == channel_name {
                for to_channel in &patch.to {
                    debug!(
                        "Routing message from '{}' to '{}'",
                        channel_name, to_channel
                    );

                    // Create new message for target channel
                    let msg = Msg {
                        name: to_channel.clone(),
                        content: content.clone(),
                        is_chan: true,
                        pubkey: crate::api::PubKey::nil(),
                        chunked: false,
                        stream_header: false,
                    };

                    if let Err(e) = node.send_msg(msg).await {
                        error!("Failed to route message to channel '{}': {}", to_channel, e);
                    } else {
                        routed = true;
                    }
                }
            }
        }

        if routed {
            info!("Message routed from channel '{}'", channel_name);
        }

        Ok(())
    }
}

impl Default for DefaultRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Router for DefaultRouter {
    async fn route(&self, node: Arc<dyn Node>, msg: Bytes) -> Result<()> {
        if msg.is_empty() {
            return Err(RatNetError::InvalidArgument("Empty message".to_string()));
        }

        let flags = msg[0];
        trace!("Routing message with flags: 0x{:02x}", flags);

        // Handle stream header
        if flags & STREAM_HEADER_FLAG != 0 {
            let (channel_name, remaining_data) = Self::extract_channel_name(&msg)?;
            let is_chan = channel_name.is_some();
            let name = channel_name.unwrap_or_default();
            let stream_msg = Msg {
                name,
                content: Bytes::from(remaining_data.to_vec()),
                is_chan,
                pubkey: crate::api::PubKey::nil(),
                chunked: true,
                stream_header: true,
            };
            return chunking::handle_chunked(node, stream_msg).await;
        }

        // Handle chunked message
        if flags & CHUNKED_FLAG != 0 {
            let (channel_name, remaining_data) = Self::extract_channel_name(&msg)?;
            let is_chan = channel_name.is_some();
            let name = channel_name.unwrap_or_default();
            let chunk_msg = Msg {
                name,
                content: Bytes::from(remaining_data.to_vec()),
                is_chan,
                pubkey: crate::api::PubKey::nil(),
                chunked: true,
                stream_header: false,
            };
            return chunking::handle_chunked(node, chunk_msg).await;
        }

        // Handle regular channel message
        if flags & CHANNEL_FLAG != 0 {
            let (channel_name, content) = Self::extract_channel_name(&msg)?;

            if let Some(channel_name) = channel_name {
                let content_bytes = Bytes::from(content.to_vec());

                // Create message for the channel
                let msg = Msg {
                    name: channel_name.clone(),
                    content: content_bytes.clone(),
                    is_chan: true,
                    pubkey: crate::api::PubKey::nil(),
                    chunked: false,
                    stream_header: false,
                };

                // Handle the message
                match node.handle(msg).await {
                    Ok(handled) => {
                        if handled {
                            debug!("Message handled by node for channel '{}'", channel_name);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Error handling message for channel '{}': {}",
                            channel_name, e
                        );
                    }
                }

                // Apply routing patches
                self.apply_patches(&channel_name, node, content_bytes)
                    .await?;

                return Ok(());
            }
        }

        // Handle regular (non-channel) message
        let content = Bytes::from(msg[1..].to_vec());
        let msg = Msg {
            name: String::new(),
            content,
            is_chan: false,
            pubkey: crate::api::PubKey::nil(),
            chunked: false,
            stream_header: false,
        };

        match node.handle(msg).await {
            Ok(handled) => {
                if handled {
                    debug!("Message handled by node");
                } else {
                    debug!("Message not handled by node");
                }
            }
            Err(e) => {
                error!("Error handling message: {}", e);
            }
        }

        Ok(())
    }

    fn patch(&self, patch: Patch) {
        let patches = self.patches.clone();
        tokio::spawn(async move {
            let mut patches = patches.write().await;
            // Remove existing patch with same 'from' channel
            patches.retain(|p| p.from != patch.from);
            // Add new patch
            patches.push(patch);
        });
    }

    fn get_patches(&self) -> Vec<Patch> {
        // This is a synchronous function but we need async access
        // In practice, you might want to redesign this interface to be async
        // For now, we'll return empty and log a warning
        tracing::warn!("get_patches called synchronously on async router");
        Vec::new()
    }
}

impl JSON for DefaultRouter {
    fn to_json(&self) -> Result<String> {
        // For serialization, we need to get the patches
        // This is a simplified implementation
        let patches_json = serde_json::json!({
            "type": "default",
            "patches": []  // Would need async access to get actual patches
        });

        serde_json::to_string(&patches_json).map_err(Into::into)
    }

    fn from_json(json: &str) -> Result<Self> {
        // Parse configuration and create router
        let _config: serde_json::Value = serde_json::from_str(json)?;
        Ok(Self::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use tokio_test; // Unused import - may be needed for future testing

    #[test]
    fn test_extract_channel_name() {
        // Test message without channel flag
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let (channel, remaining) = DefaultRouter::extract_channel_name(&data).unwrap();
        assert_eq!(channel, None);
        assert_eq!(remaining, &[0x01, 0x02, 0x03]);

        // Test message with channel flag
        let channel_name = "test-channel";
        let mut data = vec![CHANNEL_FLAG];
        data.extend_from_slice(&(channel_name.len() as u16).to_be_bytes());
        data.extend_from_slice(channel_name.as_bytes());
        data.extend_from_slice(&[0x01, 0x02, 0x03]);

        let (channel, remaining) = DefaultRouter::extract_channel_name(&data).unwrap();
        assert_eq!(channel, Some("test-channel".to_string()));
        assert_eq!(remaining, &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_router_creation() {
        let router = DefaultRouter::new();
        // Just test that it can be created
        assert_eq!(router.get_patches().len(), 0);
    }
}
