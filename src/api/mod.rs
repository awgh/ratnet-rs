//! Core API traits and data structures for RatNet

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::error::Result;

pub mod actions;
pub mod crypto;
pub mod remoting;
pub mod chunking;

pub use actions::*;
pub use crypto::*;
pub use remoting::*;
pub use chunking::*;

/// Core message type for RatNet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Msg {
    pub name: String,
    #[serde(serialize_with = "serialize_bytes", deserialize_with = "deserialize_bytes")]
    pub content: Bytes,
    pub is_chan: bool,
    pub pubkey: PubKey,
    pub chunked: bool,
    pub stream_header: bool,
}

/// Bundle of messages for batch operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    #[serde(serialize_with = "serialize_bytes", deserialize_with = "deserialize_bytes")]
    pub data: Bytes,
    pub time: i64,
}

/// Contact information (named public key)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub pubkey: String,
}

/// Channel information (named public key)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub pubkey: String,
}

/// Channel with private key
#[derive(Debug)]
pub struct ChannelPriv {
    pub name: String,
    pub pubkey: String,
    pub privkey: KeyPair,
}

/// Profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub enabled: bool,
    pub pubkey: String,
}

/// Profile with private key
#[derive(Debug)]
pub struct ProfilePriv {
    pub name: String,
    pub enabled: bool,
    pub privkey: KeyPair,
}

/// Peer connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub name: String,
    pub enabled: bool,
    pub uri: String,
    pub group: String,
}

/// Outbox message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxMsg {
    pub channel: Option<String>,
    #[serde(serialize_with = "serialize_bytes", deserialize_with = "deserialize_bytes")]
    pub msg: Bytes,
    pub timestamp: i64,
}

/// Configuration value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    pub name: String,
    pub value: String,
}

/// Event emitted by the node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event_type: String,
    pub data: HashMap<String, String>,
}

/// Stream header for chunked transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamHeader {
    pub stream_id: u32,
    pub num_chunks: u32,
    pub channel_name: String,
}

/// Chunk data for chunked transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub stream_id: u32,
    pub chunk_num: u32,
    #[serde(serialize_with = "serialize_bytes", deserialize_with = "deserialize_bytes")]
    pub data: Bytes,
}

/// Peer information for connection tracking
#[derive(Debug)]
pub struct PeerInfo {
    pub last_poll_local: std::sync::atomic::AtomicI64,
    pub last_poll_remote: std::sync::atomic::AtomicI64,
    pub total_bytes_tx: std::sync::atomic::AtomicI64,
    pub total_bytes_rx: std::sync::atomic::AtomicI64,
    pub routing_pub: Option<crypto::PubKey>,
}

/// Routing flags
pub const STREAM_HEADER_FLAG: u8 = 0x01;
pub const CHUNKED_FLAG: u8 = 0x02;
pub const CHANNEL_FLAG: u8 = 0x04;

/// Main Node trait - equivalent to Go's Node interface
#[async_trait]
pub trait Node: Send + Sync {
    // Lifecycle
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    fn is_running(&self) -> bool;

    // Configuration
    fn get_policies(&self) -> Vec<Arc<dyn Policy>>;
    fn set_policy(&self, policies: Vec<Arc<dyn Policy>>);
    fn router(&self) -> Arc<dyn Router>;
    fn set_router(&self, router: Arc<dyn Router>);

    // Channels for async communication
    fn in_channel(&self) -> mpsc::Receiver<Msg>;
    fn out_channel(&self) -> mpsc::Receiver<Msg>;
    fn events_channel(&self) -> mpsc::Receiver<Event>;

    // Core messaging
    async fn handle(&self, msg: Msg) -> Result<bool>;
    async fn forward(&self, msg: Msg) -> Result<()>;

    // Chunking support
    async fn add_stream(&self, stream_id: u32, total_chunks: u32, channel_name: String) -> Result<()>;
    async fn add_chunk(&self, stream_id: u32, chunk_num: u32, data: Bytes) -> Result<()>;
    async fn check_stream_complete(&self, stream_id: u32) -> Result<()>;

    // Maintenance
    async fn flush_outbox(&self, max_age_seconds: i64) -> Result<()>;

    // Public API
    async fn id(&self) -> Result<PubKey>;
    async fn dropoff(&self, bundle: Bundle) -> Result<()>;
    async fn pickup(&self, routing_pub: PubKey, last_time: i64, max_bytes: i64, channel_names: Vec<String>) -> Result<Bundle>;

    // Admin API
    async fn cid(&self) -> Result<PubKey>;
    
    // Contact management
    async fn get_contact(&self, name: &str) -> Result<Contact>;
    async fn get_contacts(&self) -> Result<Vec<Contact>>;
    async fn add_contact(&self, name: String, key: String) -> Result<()>;
    async fn delete_contact(&self, name: &str) -> Result<()>;

    // Channel management
    async fn get_channel(&self, name: &str) -> Result<Channel>;
    async fn get_channels(&self) -> Result<Vec<Channel>>;
    async fn add_channel(&self, name: String, privkey: String) -> Result<()>;
    async fn delete_channel(&self, name: &str) -> Result<()>;
    async fn get_channel_privkey(&self, name: &str) -> Result<String>;

    // Profile management
    async fn get_profile(&self, name: &str) -> Result<Profile>;
    async fn get_profiles(&self) -> Result<Vec<Profile>>;
    async fn add_profile(&self, name: String, enabled: bool) -> Result<()>;
    async fn delete_profile(&self, name: &str) -> Result<()>;
    async fn load_profile(&self, name: &str) -> Result<PubKey>;

    // Peer management
    async fn get_peer(&self, name: &str) -> Result<Peer>;
    async fn get_peers(&self, group: Option<String>) -> Result<Vec<Peer>>;
    async fn add_peer(&self, name: String, enabled: bool, uri: String, group: Option<String>) -> Result<()>;
    async fn delete_peer(&self, name: &str) -> Result<()>;

    // Message sending
    async fn send_msg(&self, msg: Msg) -> Result<()>;

    // RPC handling
    async fn admin_rpc(&self, transport: Arc<dyn Transport>, call: RemoteCall) -> Result<RemoteResponse>;
    async fn public_rpc(&self, transport: Arc<dyn Transport>, call: RemoteCall) -> Result<RemoteResponse>;
}

/// Transport trait for network communication
#[async_trait]
pub trait Transport: Send + Sync {
    async fn listen(&self, listen: String, admin_mode: bool) -> Result<()>;
    fn name(&self) -> &str;
    async fn rpc(&self, host: &str, method: Action, args: Vec<serde_json::Value>) -> Result<serde_json::Value>;
    async fn stop(&self) -> Result<()>;
    
    fn byte_limit(&self) -> i64;
    fn set_byte_limit(&self, limit: i64);
    fn is_running(&self) -> bool;
}

/// Router trait for message routing
#[async_trait] 
pub trait Router: Send + Sync + std::fmt::Debug {
    async fn route(&self, node: Arc<dyn Node>, msg: Bytes) -> Result<()>;
    fn patch(&self, patch: Patch);
    fn get_patches(&self) -> Vec<Patch>;
}

/// Policy trait for connection management
#[async_trait]
pub trait Policy: Send + Sync {
    async fn run_policy(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    fn get_transport(&self) -> Arc<dyn Transport>;
}

/// Routing patch for channel mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub from: String,
    pub to: Vec<String>,
}

// Custom serialization functions for Bytes
fn serialize_bytes<S>(bytes: &Bytes, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serde_bytes::serialize(bytes.as_ref(), serializer)
}

fn deserialize_bytes<'de, D>(deserializer: D) -> std::result::Result<Bytes, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let vec: Vec<u8> = serde_bytes::deserialize(deserializer)?;
    Ok(Bytes::from(vec))
} 