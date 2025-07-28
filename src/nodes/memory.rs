//! In-memory node implementation for testing and simple use cases

use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Arc as StdArc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::api::*;
use crate::error::{RatNetError, Result};

pub struct MemoryNode {
    // Core state
    running: AtomicBool,
    routing_key: RwLock<Option<KeyPair>>,
    content_key: RwLock<Option<KeyPair>>,

    // Components
    router: StdArc<RwLock<Option<Arc<dyn Router>>>>,
    policies: StdArc<RwLock<Vec<Arc<dyn Policy>>>>,

    // Data storage
    contacts: DashMap<String, Contact>,
    channels: DashMap<String, ChannelPriv>,
    profiles: DashMap<String, ProfilePriv>,
    peers: DashMap<String, Peer>,
    outbox: DashMap<String, Vec<OutboxMsg>>,
    bundles: DashMap<String, Bundle>, // Store bundles by routing key

    // Message channels
    in_tx: mpsc::UnboundedSender<Msg>,
    in_rx: RwLock<Option<mpsc::UnboundedReceiver<Msg>>>,
    out_tx: mpsc::UnboundedSender<Msg>,
    out_rx: RwLock<Option<mpsc::UnboundedReceiver<Msg>>>,
    events_tx: mpsc::UnboundedSender<Event>,
    events_rx: RwLock<Option<mpsc::UnboundedReceiver<Event>>>,

    // Streaming support
    streams: DashMap<u32, StreamInfo>,
    chunks: DashMap<(u32, u32), Bytes>, // (stream_id, chunk_num) -> data
}

#[derive(Debug, Clone)]
struct StreamInfo {
    total_chunks: u32,
    channel_name: String,
    received_chunks: HashMap<u32, bool>,
}

impl MemoryNode {
    /// Create a new memory node
    pub fn new() -> Self {
        let (in_tx, in_rx) = mpsc::unbounded_channel();
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        Self {
            running: AtomicBool::new(false),
            routing_key: RwLock::new(None),
            content_key: RwLock::new(None),
            router: StdArc::new(RwLock::new(None)),
            policies: StdArc::new(RwLock::new(Vec::new())),
            contacts: DashMap::new(),
            channels: DashMap::new(),
            profiles: DashMap::new(),
            peers: DashMap::new(),
            outbox: DashMap::new(),
            bundles: DashMap::new(),
            in_tx,
            in_rx: RwLock::new(Some(in_rx)),
            out_tx,
            out_rx: RwLock::new(Some(out_rx)),
            events_tx,
            events_rx: RwLock::new(Some(events_rx)),
            streams: DashMap::new(),
            chunks: DashMap::new(),
        }
    }

    /// Generate or set the routing key
    async fn ensure_routing_key(&self) -> Result<()> {
        let mut key_guard = self.routing_key.write().await;
        if key_guard.is_none() {
            let keypair = KeyPair::generate_ed25519()?;
            *key_guard = Some(keypair);
            debug!("Generated new routing key");
        }
        Ok(())
    }

    /// Generate or set the content key
    async fn ensure_content_key(&self) -> Result<()> {
        let mut key_guard = self.content_key.write().await;
        if key_guard.is_none() {
            let keypair = KeyPair::generate_ed25519()?;
            *key_guard = Some(keypair);
            debug!("Generated new content key");
        }
        Ok(())
    }

    /// Generate a hybrid key pair for routing (Ed25519 for signatures + Kyber for encryption)
    #[allow(dead_code)]
    async fn ensure_hybrid_routing_keys(&self) -> Result<()> {
        let mut routing_key_guard = self.routing_key.write().await;
        if routing_key_guard.is_none() {
            let (ed25519_keypair, _kyber_keypair) = generate_hybrid_keypair()?;
            *routing_key_guard = Some(ed25519_keypair);
            debug!("Generated new hybrid routing keys");
        }
        Ok(())
    }

    /// Generate a hybrid key pair for content (Ed25519 for signatures + Kyber for encryption)
    #[allow(dead_code)]
    async fn ensure_hybrid_content_keys(&self) -> Result<()> {
        let mut content_key_guard = self.content_key.write().await;
        if content_key_guard.is_none() {
            let (ed25519_keypair, _kyber_keypair) = generate_hybrid_keypair()?;
            *content_key_guard = Some(ed25519_keypair);
            debug!("Generated new hybrid content keys");
        }
        Ok(())
    }
}

impl Default for MemoryNode {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MemoryNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryNode")
            .field("running", &self.running)
            .field("contacts", &self.contacts.len())
            .field("channels", &self.channels.len())
            .field("profiles", &self.profiles.len())
            .field("peers", &self.peers.len())
            .field("outbox", &self.outbox.len())
            .field("streams", &self.streams.len())
            .field("chunks", &self.chunks.len())
            .finish()
    }
}

#[async_trait]
impl Node for MemoryNode {
    async fn start(&self) -> Result<()> {
        info!("Starting memory node");

        // Ensure we have keys
        self.ensure_routing_key().await?;
        self.ensure_content_key().await?;

        self.running.store(true, Ordering::Relaxed);

        // Start policies
        let policies = self.policies.read().await.clone();
        for policy in policies {
            if let Err(e) = policy.run_policy().await {
                warn!("Failed to start policy: {}", e);
            }
        }

        info!("Memory node started");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping memory node");

        self.running.store(false, Ordering::Relaxed);

        // Stop policies
        let policies = self.policies.read().await.clone();
        for policy in policies {
            if let Err(e) = policy.stop().await {
                warn!("Failed to stop policy: {}", e);
            }
        }

        info!("Memory node stopped");
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn get_policies(&self) -> Vec<Arc<dyn Policy>> {
        // This needs to be redesigned to be async-friendly
        warn!("get_policies called synchronously on async node");
        Vec::new()
    }

    fn set_policy(&self, policies: Vec<Arc<dyn Policy>>) {
        let policies_clone = policies;
        let policies_lock = self.policies.clone();
        tokio::spawn(async move {
            let mut policies_guard = policies_lock.write().await;
            *policies_guard = policies_clone;
        });
    }

    fn router(&self) -> Arc<dyn Router> {
        // This needs to be redesigned to be async-friendly
        warn!("router called synchronously on async node");
        Arc::new(crate::router::DefaultRouter::new())
    }

    fn set_router(&self, router: Arc<dyn Router>) {
        let router_lock = self.router.clone();
        tokio::spawn(async move {
            let mut router_guard = router_lock.write().await;
            *router_guard = Some(router);
        });
    }

    fn in_channel(&self) -> mpsc::Receiver<Msg> {
        // This interface needs to be redesigned for proper async usage
        // For now, create a new channel
        let (_, rx) = mpsc::channel(100);
        rx
    }

    fn out_channel(&self) -> mpsc::Receiver<Msg> {
        // This interface needs to be redesigned for proper async usage
        let (_, rx) = mpsc::channel(100);
        rx
    }

    fn events_channel(&self) -> mpsc::Receiver<Event> {
        // This interface needs to be redesigned for proper async usage
        let (_, rx) = mpsc::channel(100);
        rx
    }

    async fn handle(&self, msg: Msg) -> Result<bool> {
        debug!("Handling message: {}", msg.name);

        // Send to in channel
        if let Err(_) = self.in_tx.send(msg) {
            warn!("Failed to send message to in channel");
        }

        // In a full implementation, this would process the message
        // and return whether it was handled
        Ok(true)
    }

    async fn forward(&self, msg: Msg) -> Result<()> {
        debug!("Forwarding message: {}", msg.name);

        // Send to out channel
        if let Err(_) = self.out_tx.send(msg) {
            return Err(RatNetError::Node("Failed to forward message".to_string()));
        }

        Ok(())
    }

    async fn add_stream(
        &self,
        stream_id: u32,
        total_chunks: u32,
        channel_name: String,
    ) -> Result<()> {
        debug!(
            "Adding stream: id={}, chunks={}, channel={}",
            stream_id, total_chunks, channel_name
        );

        let stream_info = StreamInfo {
            total_chunks,
            channel_name,
            received_chunks: HashMap::new(),
        };

        self.streams.insert(stream_id, stream_info);
        Ok(())
    }

    async fn add_chunk(&self, stream_id: u32, chunk_num: u32, data: Bytes) -> Result<()> {
        debug!(
            "Adding chunk: stream={}, chunk={}, size={}",
            stream_id,
            chunk_num,
            data.len()
        );

        self.chunks.insert((stream_id, chunk_num), data);

        // Check if we have all chunks for this stream
        if let Some(mut stream_info) = self.streams.get_mut(&stream_id) {
            stream_info.received_chunks.insert(chunk_num, true);

            if stream_info.received_chunks.len() as u32 == stream_info.total_chunks {
                debug!("Stream {} complete, reassembling", stream_id);
                // Trigger reassembly check
                drop(stream_info); // Release the lock before async call
                return self.check_stream_complete(stream_id).await;
            }
        }

        Ok(())
    }

    async fn check_stream_complete(&self, stream_id: u32) -> Result<()> {
        let stream_info = match self.streams.get(&stream_id) {
            Some(info) => {
                // Extract the needed values instead of cloning the whole thing
                StreamInfo {
                    total_chunks: info.total_chunks,
                    channel_name: info.channel_name.clone(),
                    received_chunks: info.received_chunks.clone(),
                }
            }
            None => {
                warn!("Stream {} not found for completion check", stream_id);
                return Ok(());
            }
        };

        // Check if we have all chunks
        if stream_info.received_chunks.len() as u32 != stream_info.total_chunks {
            return Ok(()); // Not complete yet
        }

        // Collect all chunks in order
        let mut reassembled_data = Vec::new();
        for chunk_num in 0..stream_info.total_chunks {
            if let Some(chunk_data) = self.chunks.get(&(stream_id, chunk_num)) {
                reassembled_data.extend_from_slice(&chunk_data);
            } else {
                warn!("Missing chunk {} for stream {}", chunk_num, stream_id);
                return Ok(());
            }
        }

        // Create the reassembled message
        let content = Bytes::from(reassembled_data);
        let msg = Msg {
            name: stream_info.channel_name.clone(),
            content,
            is_chan: !stream_info.channel_name.is_empty(),
            pubkey: PubKey::nil(),
            chunked: false,
            stream_header: false,
        };

        debug!(
            "Reassembled stream {} into {} bytes",
            stream_id,
            msg.content.len()
        );

        // Clean up chunks and stream info
        for chunk_num in 0..stream_info.total_chunks {
            self.chunks.remove(&(stream_id, chunk_num));
        }
        self.streams.remove(&stream_id);

        // Handle the reassembled message
        match self.handle(msg).await {
            Ok(handled) => {
                if handled {
                    debug!("Reassembled message handled successfully");
                } else {
                    debug!("Reassembled message not handled");
                }
            }
            Err(e) => {
                warn!("Error handling reassembled message: {}", e);
            }
        }

        Ok(())
    }

    async fn admin_rpc(
        &self,
        _transport: Arc<dyn Transport>,
        call: RemoteCall,
    ) -> Result<RemoteResponse> {
        debug!("Admin RPC call: {:?}", call.action);

        match call.action {
            Action::CID => match self.cid().await {
                Ok(pubkey) => Ok(RemoteResponse::success(serde_json::json!(
                    pubkey.to_string()
                ))),
                Err(e) => Ok(RemoteResponse::error(e.to_string())),
            },

            Action::GetContact => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.get_contact(name).await {
                    Ok(contact) => Ok(RemoteResponse::success(serde_json::json!(contact))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetContacts => match self.get_contacts().await {
                Ok(contacts) => Ok(RemoteResponse::success(serde_json::json!(contacts))),
                Err(e) => Ok(RemoteResponse::error(e.to_string())),
            },

            Action::AddContact => {
                if call.args.len() < 2 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let key = match &call.args[1] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.add_contact(name, key).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::DeleteContact => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.delete_contact(name).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetChannel => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.get_channel(name).await {
                    Ok(channel) => Ok(RemoteResponse::success(serde_json::json!(channel))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetChannels => match self.get_channels().await {
                Ok(channels) => Ok(RemoteResponse::success(serde_json::json!(channels))),
                Err(e) => Ok(RemoteResponse::error(e.to_string())),
            },

            Action::AddChannel => {
                if call.args.len() < 2 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let privkey = match &call.args[1] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.add_channel(name, privkey).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::DeleteChannel => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.delete_channel(name).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetProfile => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.get_profile(name).await {
                    Ok(profile) => Ok(RemoteResponse::success(serde_json::json!(profile))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetProfiles => match self.get_profiles().await {
                Ok(profiles) => Ok(RemoteResponse::success(serde_json::json!(profiles))),
                Err(e) => Ok(RemoteResponse::error(e.to_string())),
            },

            Action::AddProfile => {
                if call.args.len() < 2 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let enabled = match &call.args[1] {
                    serde_json::Value::Bool(b) => *b,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.add_profile(name, enabled).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::DeleteProfile => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.delete_profile(name).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::LoadProfile => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.load_profile(name).await {
                    Ok(pubkey) => Ok(RemoteResponse::success(serde_json::json!(
                        pubkey.to_string()
                    ))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetPeer => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.get_peer(name).await {
                    Ok(peer) => Ok(RemoteResponse::success(serde_json::json!(peer))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetPeers => {
                let group = if !call.args.is_empty() {
                    match &call.args[0] {
                        serde_json::Value::String(s) => Some(s.clone()),
                        _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                    }
                } else {
                    None
                };

                match self.get_peers(group).await {
                    Ok(peers) => Ok(RemoteResponse::success(serde_json::json!(peers))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::AddPeer => {
                if call.args.len() < 3 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let enabled = match &call.args[1] {
                    serde_json::Value::Bool(b) => *b,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let uri = match &call.args[2] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let group = if call.args.len() > 3 {
                    match &call.args[3] {
                        serde_json::Value::String(s) => Some(s.clone()),
                        _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                    }
                } else {
                    None
                };

                match self.add_peer(name, enabled, uri, group).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::DeletePeer => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.delete_peer(name).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::SendMsg => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                // Parse the message from JSON
                let msg_json = &call.args[0];
                match serde_json::from_value::<Msg>(msg_json.clone()) {
                    Ok(msg) => match self.send_msg(msg).await {
                        Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                        Err(e) => Ok(RemoteResponse::error(e.to_string())),
                    },
                    Err(e) => Ok(RemoteResponse::error(format!(
                        "Invalid message format: {e}"
                    ))),
                }
            }

            Action::Send => {
                if call.args.len() < 2 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let contact_name = match &call.args[0] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let data = match &call.args[1] {
                    serde_json::Value::String(s) => s.as_bytes().to_vec(),
                    serde_json::Value::Array(arr) => {
                        // Try to parse as array of numbers
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect()
                    }
                    _ => return Ok(RemoteResponse::error("Invalid data format".to_string())),
                };

                // Create a message for the contact
                let msg = Msg {
                    name: contact_name,
                    content: Bytes::from(data),
                    is_chan: false,
                    pubkey: PubKey::Nil, // Will be set by the node
                    chunked: false,
                    stream_header: false,
                };

                match self.send_msg(msg).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::SendChannel => {
                if call.args.len() < 2 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let channel_name = match &call.args[0] {
                    serde_json::Value::String(s) => s.clone(),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                let data = match &call.args[1] {
                    serde_json::Value::String(s) => s.as_bytes().to_vec(),
                    serde_json::Value::Array(arr) => {
                        // Try to parse as array of numbers
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect()
                    }
                    _ => return Ok(RemoteResponse::error("Invalid data format".to_string())),
                };

                // Create a message for the channel
                let msg = Msg {
                    name: channel_name,
                    content: Bytes::from(data),
                    is_chan: true,
                    pubkey: PubKey::Nil, // Will be set by the node
                    chunked: false,
                    stream_header: false,
                };

                match self.send_msg(msg).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::GetChannelPrivKey => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                let name = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                match self.get_channel_privkey(name).await {
                    Ok(privkey) => Ok(RemoteResponse::success(serde_json::json!(privkey))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::FlushOutbox => {
                let max_age = if !call.args.is_empty() {
                    match &call.args[0] {
                        serde_json::Value::Number(n) => n.as_i64().unwrap_or(3600),
                        _ => 3600,
                    }
                } else {
                    3600
                };

                match self.flush_outbox(max_age).await {
                    Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::IsRunning => Ok(RemoteResponse::success(
                serde_json::json!(self.is_running()),
            )),

            Action::Start => match self.start().await {
                Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                Err(e) => Ok(RemoteResponse::error(e.to_string())),
            },

            Action::Stop => {
                let _ = self.stop().await;
                Ok(RemoteResponse::success(serde_json::Value::Null))
            }

            _ => Ok(RemoteResponse::error(format!(
                "Unknown admin action: {:?}",
                call.action
            ))),
        }
    }

    async fn public_rpc(
        &self,
        transport: Arc<dyn Transport>,
        call: RemoteCall,
    ) -> Result<RemoteResponse> {
        debug!("Public RPC call: {:?}", call.action);

        match call.action {
            Action::ID => match self.id().await {
                Ok(pubkey) => {
                    if pubkey.is_nil() {
                        Ok(RemoteResponse::error(
                            "Node has no routing key set".to_string(),
                        ))
                    } else {
                        Ok(RemoteResponse::success(serde_json::json!(
                            pubkey.to_string()
                        )))
                    }
                }
                Err(e) => Ok(RemoteResponse::error(e.to_string())),
            },

            Action::Pickup => {
                if call.args.len() < 4 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                // Parse routing public key
                let routing_pub_str = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument 1".to_string())),
                };

                // Parse last time
                let last_time = match &call.args[1] {
                    serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
                    _ => return Ok(RemoteResponse::error("Invalid argument 2".to_string())),
                };

                // Parse max bytes
                let max_bytes = match &call.args[2] {
                    serde_json::Value::Number(n) => n.as_i64().unwrap_or(transport.byte_limit()),
                    _ => return Ok(RemoteResponse::error("Invalid argument 3".to_string())),
                };

                // Parse channel names array
                let channel_names: Vec<String> = match &call.args[3] {
                    serde_json::Value::Array(arr) => arr
                        .iter()
                        .filter_map(|arg| {
                            if let serde_json::Value::String(s) = arg {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                    _ => {
                        return Ok(RemoteResponse::error(
                            "Invalid argument 4 (expected array)".to_string(),
                        ))
                    }
                };

                // Parse routing public key
                let routing_pub = match PubKey::from_string(routing_pub_str) {
                    Ok(pk) => pk,
                    Err(_) => {
                        return Ok(RemoteResponse::error(
                            "Invalid routing public key".to_string(),
                        ))
                    }
                };

                match self
                    .pickup(routing_pub, last_time, max_bytes, channel_names)
                    .await
                {
                    Ok(bundle) => Ok(RemoteResponse::success(serde_json::json!(bundle))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::Dropoff => {
                if call.args.is_empty() {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                // Parse the bundle from JSON
                let bundle_json = &call.args[0];
                match serde_json::from_value::<Bundle>(bundle_json.clone()) {
                    Ok(bundle) => match self.dropoff(bundle).await {
                        Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                        Err(e) => Ok(RemoteResponse::error(e.to_string())),
                    },
                    Err(e) => Ok(RemoteResponse::error(format!("Invalid bundle format: {e}"))),
                }
            }

            _ => Ok(RemoteResponse::error(format!(
                "Unknown public action: {:?}",
                call.action
            ))),
        }
    }

    async fn flush_outbox(&self, max_age_seconds: i64) -> Result<()> {
        debug!("Flushing outbox (max age: {} seconds)", max_age_seconds);

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Remove old messages from outbox
        for mut entry in self.outbox.iter_mut() {
            entry
                .value_mut()
                .retain(|msg| current_time - msg.timestamp < max_age_seconds);
        }

        Ok(())
    }

    async fn id(&self) -> Result<PubKey> {
        let key_guard = self.routing_key.read().await;
        match key_guard.as_ref() {
            Some(keypair) => Ok(keypair.public_key()),
            None => Err(RatNetError::Node("No routing key set".to_string())),
        }
    }

    async fn dropoff(&self, bundle: Bundle) -> Result<()> {
        debug!("Dropoff bundle: {} bytes", bundle.data.len());

        // Deserialize bundle data into individual messages
        let messages = match self.deserialize_bundle(&bundle.data).await {
            Ok(msgs) => msgs,
            Err(e) => {
                warn!("Failed to deserialize bundle: {}", e);
                return Ok(()); // Don't fail dropoff, just log the error
            }
        };

        // Process each message in the bundle
        for msg in messages {
            let msg_name = msg.name.clone();
            match self.handle(msg).await {
                Ok(_) => {
                    debug!("Processed message from bundle: {}", msg_name);
                }
                Err(e) => {
                    warn!("Failed to process message from bundle: {}", e);
                    // Continue processing other messages
                }
            }
        }

        Ok(())
    }

    async fn pickup(
        &self,
        routing_pub: PubKey,
        last_time: i64,
        max_bytes: i64,
        channel_names: Vec<String>,
    ) -> Result<Bundle> {
        debug!(
            "Pickup for key: {}, channels: {:?}, max_bytes: {}",
            routing_pub, channel_names, max_bytes
        );

        // Gather messages from outbox that match the criteria
        let mut messages = Vec::new();
        let mut total_bytes = 0;

        // Get messages from outbox for the specified channels
        for channel_name in &channel_names {
            if let Some(outbox_msgs) = self.outbox.get(channel_name) {
                for outbox_msg in outbox_msgs.iter() {
                    // Check if message is within time range and size limit
                    if outbox_msg.timestamp >= last_time
                        && total_bytes + outbox_msg.msg.len() as i64 <= max_bytes
                    {
                        // Deserialize the outbox message into a Msg
                        match self.deserialize_outbox_msg(&outbox_msg.msg).await {
                            Ok(msg) => {
                                messages.push(msg);
                                total_bytes += outbox_msg.msg.len() as i64;
                            }
                            Err(e) => {
                                warn!("Failed to deserialize outbox message: {}", e);
                                // Continue with other messages
                            }
                        }
                    }
                }
            }
        }

        // Serialize messages into bundle
        let bundle_data = match self.serialize_messages_to_bundle(&messages).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Failed to serialize messages to bundle: {}", e);
                return Ok(Bundle {
                    data: Bytes::new(),
                    time: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                });
            }
        };

        debug!(
            "Pickup returning bundle with {} messages, {} bytes",
            messages.len(),
            bundle_data.len()
        );

        Ok(Bundle {
            data: bundle_data,
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        })
    }

    async fn cid(&self) -> Result<PubKey> {
        let key_guard = self.content_key.read().await;
        match key_guard.as_ref() {
            Some(keypair) => Ok(keypair.public_key()),
            None => Err(RatNetError::Node("No content key set".to_string())),
        }
    }

    async fn get_contact(&self, name: &str) -> Result<Contact> {
        self.contacts
            .get(name)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| RatNetError::NotFound(format!("Contact '{name}' not found")))
    }

    async fn get_contacts(&self) -> Result<Vec<Contact>> {
        Ok(self
            .contacts
            .iter()
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn add_contact(&self, name: String, key: String) -> Result<()> {
        let contact = Contact {
            name: name.clone(),
            pubkey: key,
        };
        self.contacts.insert(name, contact);
        Ok(())
    }

    async fn delete_contact(&self, name: &str) -> Result<()> {
        self.contacts
            .remove(name)
            .ok_or_else(|| RatNetError::NotFound(format!("Contact '{name}' not found")))?;
        Ok(())
    }

    async fn get_channel(&self, name: &str) -> Result<Channel> {
        self.channels
            .get(name)
            .map(|entry| Channel {
                name: entry.name.clone(),
                pubkey: entry.pubkey.clone(),
            })
            .ok_or_else(|| RatNetError::NotFound(format!("Channel '{name}' not found")))
    }

    async fn get_channels(&self) -> Result<Vec<Channel>> {
        Ok(self
            .channels
            .iter()
            .map(|entry| Channel {
                name: entry.name.clone(),
                pubkey: entry.pubkey.clone(),
            })
            .collect())
    }

    async fn add_channel(&self, name: String, privkey: String) -> Result<()> {
        // Parse the private key string and derive the public key
        let keypair = KeyPair::from_string(&privkey)
            .map_err(|e| RatNetError::Crypto(format!("Invalid private key: {e}")))?;

        let pubkey = keypair.public_key();

        let channel = ChannelPriv {
            name: name.clone(),
            pubkey: pubkey.to_string(),
            privkey: keypair,
        };
        self.channels.insert(name, channel);
        Ok(())
    }

    async fn delete_channel(&self, name: &str) -> Result<()> {
        self.channels
            .remove(name)
            .ok_or_else(|| RatNetError::NotFound(format!("Channel '{name}' not found")))?;
        Ok(())
    }

    async fn get_channel_privkey(&self, name: &str) -> Result<String> {
        if let Some(entry) = self.channels.get(name) {
            entry.privkey.to_string()
        } else {
            Err(RatNetError::NotFound(format!("Channel '{name}' not found")))
        }
    }

    async fn get_profile(&self, name: &str) -> Result<Profile> {
        self.profiles
            .get(name)
            .map(|entry| Profile {
                name: entry.name.clone(),
                enabled: entry.enabled,
                pubkey: entry.privkey.public_key().to_string(),
            })
            .ok_or_else(|| RatNetError::NotFound(format!("Profile '{name}' not found")))
    }

    async fn get_profiles(&self) -> Result<Vec<Profile>> {
        Ok(self
            .profiles
            .iter()
            .map(|entry| Profile {
                name: entry.name.clone(),
                enabled: entry.enabled,
                pubkey: entry.privkey.public_key().to_string(),
            })
            .collect())
    }

    async fn add_profile(&self, name: String, enabled: bool) -> Result<()> {
        let profile = ProfilePriv {
            name: name.clone(),
            enabled,
            privkey: KeyPair::generate_ed25519()?,
        };
        self.profiles.insert(name, profile);
        Ok(())
    }

    async fn delete_profile(&self, name: &str) -> Result<()> {
        self.profiles
            .remove(name)
            .ok_or_else(|| RatNetError::NotFound(format!("Profile '{name}' not found")))?;
        Ok(())
    }

    async fn load_profile(&self, name: &str) -> Result<PubKey> {
        self.profiles
            .get(name)
            .map(|entry| entry.privkey.public_key())
            .ok_or_else(|| RatNetError::NotFound(format!("Profile '{name}' not found")))
    }

    async fn get_peer(&self, name: &str) -> Result<Peer> {
        self.peers
            .get(name)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| RatNetError::NotFound(format!("Peer '{name}' not found")))
    }

    async fn get_peers(&self, group: Option<String>) -> Result<Vec<Peer>> {
        let peers: Vec<Peer> = self
            .peers
            .iter()
            .filter(|entry| {
                if let Some(ref group) = group {
                    &entry.group == group
                } else {
                    true
                }
            })
            .map(|entry| entry.value().clone())
            .collect();
        Ok(peers)
    }

    async fn add_peer(
        &self,
        name: String,
        enabled: bool,
        uri: String,
        group: Option<String>,
    ) -> Result<()> {
        let peer = Peer {
            name: name.clone(),
            enabled,
            uri,
            group: group.unwrap_or_default(),
        };
        self.peers.insert(name, peer);
        Ok(())
    }

    async fn delete_peer(&self, name: &str) -> Result<()> {
        self.peers
            .remove(name)
            .ok_or_else(|| RatNetError::NotFound(format!("Peer '{name}' not found")))?;
        Ok(())
    }

    async fn send_msg(&self, msg: Msg) -> Result<()> {
        debug!(
            "Sending message: name='{}', size={}, chunked={}",
            msg.name,
            msg.content.len(),
            msg.chunked
        );

        // If message is already chunked, send as-is
        if msg.chunked {
            return self.send_msg_internal(msg);
        }

        // For automatic chunking, we'll rely on the transport layer to handle it
        // since calculating chunk size requires the policy setup which may not be complete yet
        // The actual chunking will be handled by the transport when needed
        self.send_msg_internal(msg)
    }
}

impl MemoryNode {
    /// Send a message directly without chunking (internal method)
    fn send_msg_internal(&self, msg: Msg) -> Result<()> {
        // Serialize the message for outbox storage
        let msg_data = match serde_json::to_vec(&msg) {
            Ok(data) => Bytes::from(data),
            Err(e) => {
                error!("Failed to serialize message for outbox: {}", e);
                return Err(RatNetError::Serialization(format!(
                    "Failed to serialize message: {e}"
                )));
            }
        };

        let outbox_msg = OutboxMsg {
            channel: if msg.is_chan {
                Some(msg.name.clone())
            } else {
                None
            },
            msg: msg_data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };

        if let Some(channel) = &outbox_msg.channel {
            self.outbox
                .entry(channel.clone())
                .or_default()
                .push(outbox_msg);
        } else {
            self.outbox
                .entry(String::new())
                .or_default()
                .push(outbox_msg);
        }

        Ok(())
    }

    /// Deserialize bundle data into individual messages
    async fn deserialize_bundle(&self, bundle_data: &[u8]) -> Result<Vec<Msg>> {
        if bundle_data.is_empty() {
            return Ok(Vec::new());
        }

        // Try to deserialize as JSON array of messages
        match serde_json::from_slice::<Vec<Msg>>(bundle_data) {
            Ok(messages) => Ok(messages),
            Err(_) => {
                // Fallback: try to deserialize as single message
                match serde_json::from_slice::<Msg>(bundle_data) {
                    Ok(message) => Ok(vec![message]),
                    Err(e) => Err(RatNetError::Serialization(format!(
                        "Failed to deserialize bundle: {e}"
                    ))),
                }
            }
        }
    }

    /// Serialize messages into bundle data
    async fn serialize_messages_to_bundle(&self, messages: &[Msg]) -> Result<Bytes> {
        if messages.is_empty() {
            return Ok(Bytes::new());
        }

        // Serialize messages as JSON array
        let json_data = serde_json::to_vec(&messages).map_err(|e| {
            RatNetError::Serialization(format!("Failed to serialize messages: {e}"))
        })?;

        Ok(Bytes::from(json_data))
    }

    /// Deserialize outbox message data into a Msg
    async fn deserialize_outbox_msg(&self, msg_data: &[u8]) -> Result<Msg> {
        serde_json::from_slice(msg_data).map_err(|e| {
            RatNetError::Serialization(format!("Failed to deserialize outbox message: {e}"))
        })
    }

    /// Serialize a Msg into outbox message data
    #[allow(dead_code)]
    async fn serialize_msg_to_outbox(&self, msg: &Msg) -> Result<Bytes> {
        let json_data = serde_json::to_vec(msg)
            .map_err(|e| RatNetError::Serialization(format!("Failed to serialize message: {e}")))?;

        Ok(Bytes::from(json_data))
    }
}

impl Clone for MemoryNode {
    fn clone(&self) -> Self {
        // For cloning, create a new node instance
        // This is used primarily for Arc<dyn Node> operations
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transports::UdpTransport;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_admin_rpc_cid() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        // Generate keys first
        node.ensure_content_key().await.unwrap();

        let call = RemoteCall {
            action: Action::CID,
            args: vec![],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());
        assert!(!response.is_nil());

        let value = response.value.unwrap();
        assert!(value.is_string());
    }

    #[tokio::test]
    async fn test_admin_rpc_add_contact() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        let call = RemoteCall {
            action: Action::AddContact,
            args: vec![
                serde_json::json!("test_contact"),
                serde_json::json!("ed25519:test_key"),
            ],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());

        // Verify contact was added
        let call = RemoteCall {
            action: Action::GetContact,
            args: vec![serde_json::json!("test_contact")],
        };

        let response = node.admin_rpc(transport, call).await.unwrap();
        assert!(!response.is_err());

        let contact: Contact = serde_json::from_value(response.value.unwrap()).unwrap();
        assert_eq!(contact.name, "test_contact");
        assert_eq!(contact.pubkey, "ed25519:test_key");
    }

    #[tokio::test]
    async fn test_admin_rpc_add_channel() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        // Generate a real keypair for the test
        let keypair = KeyPair::generate_ed25519().unwrap();
        let privkey_str = keypair.to_string().unwrap();

        let call = RemoteCall {
            action: Action::AddChannel,
            args: vec![
                serde_json::json!("test_channel"),
                serde_json::json!(privkey_str),
            ],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());

        // Verify channel was added
        let call = RemoteCall {
            action: Action::GetChannel,
            args: vec![serde_json::json!("test_channel")],
        };

        let response = node.admin_rpc(transport, call).await.unwrap();
        assert!(!response.is_err());

        let channel: Channel = serde_json::from_value(response.value.unwrap()).unwrap();
        assert_eq!(channel.name, "test_channel");
    }

    #[tokio::test]
    async fn test_admin_rpc_add_peer() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        let call = RemoteCall {
            action: Action::AddPeer,
            args: vec![
                serde_json::json!("test_peer"),
                serde_json::json!(true),
                serde_json::json!("127.0.0.1:8080"),
                serde_json::json!("test_group"),
            ],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());

        // Verify peer was added
        let call = RemoteCall {
            action: Action::GetPeer,
            args: vec![serde_json::json!("test_peer")],
        };

        let response = node.admin_rpc(transport, call).await.unwrap();
        assert!(!response.is_err());

        let peer: Peer = serde_json::from_value(response.value.unwrap()).unwrap();
        assert_eq!(peer.name, "test_peer");
        assert_eq!(peer.uri, "127.0.0.1:8080");
        assert_eq!(peer.group, "test_group");
    }

    #[tokio::test]
    async fn test_public_rpc_id() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        // Generate keys first
        node.ensure_routing_key().await.unwrap();

        let call = RemoteCall {
            action: Action::ID,
            args: vec![],
        };

        let response = node.public_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());
        assert!(!response.is_nil());

        let value = response.value.unwrap();
        assert!(value.is_string());
    }

    #[tokio::test]
    async fn test_public_rpc_pickup() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        // Generate keys first
        node.ensure_routing_key().await.unwrap();
        let routing_key = node.id().await.unwrap();

        let call = RemoteCall {
            action: Action::Pickup,
            args: vec![
                serde_json::json!(routing_key.to_string()),
                serde_json::json!(0),
                serde_json::json!(1024), // max_bytes
                serde_json::json!(vec![serde_json::json!("test_channel")]), // channel_names
            ],
        };

        let response = node.public_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());

        let bundle: Bundle = serde_json::from_value(response.value.unwrap()).unwrap();
        assert_eq!(bundle.data.len(), 0); // Should be empty for new node
    }

    #[tokio::test]
    async fn test_public_rpc_dropoff() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        let bundle = Bundle {
            data: Bytes::from("test data"),
            time: 1234567890,
        };

        let call = RemoteCall {
            action: Action::Dropoff,
            args: vec![serde_json::json!(bundle)],
        };

        let response = node.public_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());
    }

    #[tokio::test]
    async fn test_admin_rpc_send_msg() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        let msg = Msg {
            name: "test_contact".to_string(),
            content: Bytes::from("Hello, world!"),
            is_chan: false,
            pubkey: PubKey::Nil,
            chunked: false,
            stream_header: false,
        };

        let call = RemoteCall {
            action: Action::SendMsg,
            args: vec![serde_json::json!(msg)],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());
    }

    #[tokio::test]
    async fn test_admin_rpc_send() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        let call = RemoteCall {
            action: Action::Send,
            args: vec![
                serde_json::json!("test_contact"),
                serde_json::json!("Hello, world!"),
            ],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());
    }

    #[tokio::test]
    async fn test_admin_rpc_send_channel() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        let call = RemoteCall {
            action: Action::SendChannel,
            args: vec![
                serde_json::json!("test_channel"),
                serde_json::json!("Hello, channel!"),
            ],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(!response.is_err());
    }

    #[tokio::test]
    async fn test_admin_rpc_error_handling() {
        let node = Arc::new(MemoryNode::new());
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

        // Test invalid argument count
        let call = RemoteCall {
            action: Action::AddContact,
            args: vec![serde_json::json!("test_contact")], // Missing second argument
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(response.is_err());

        // Test invalid argument type
        let call = RemoteCall {
            action: Action::AddContact,
            args: vec![
                serde_json::json!(123), // Should be string
                serde_json::json!("test_key"),
            ],
        };

        let response = node.admin_rpc(transport.clone(), call).await.unwrap();
        assert!(response.is_err());
    }
}
