//! Filesystem-based node implementation for persistent storage

use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Arc as StdArc;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::api::*;
use crate::error::{RatNetError, Result};

/// Filesystem node implementation that stores data on disk
pub struct FilesystemNode {
    // Core state
    running: AtomicBool,
    routing_key: RwLock<Option<KeyPair>>,
    content_key: RwLock<Option<KeyPair>>,

    // Components
    router: StdArc<RwLock<Option<Arc<dyn Router>>>>,
    policies: StdArc<RwLock<Vec<Arc<dyn Policy>>>>,

    // Data storage (in-memory for fast access)
    contacts: DashMap<String, Contact>,
    channels: DashMap<String, ChannelPriv>,
    profiles: DashMap<String, ProfilePriv>,
    peers: DashMap<String, Peer>,

    // Message channels
    in_tx: mpsc::UnboundedSender<Msg>,
    in_rx: RwLock<Option<mpsc::UnboundedReceiver<Msg>>>,
    out_tx: mpsc::UnboundedSender<Msg>,
    out_rx: RwLock<Option<mpsc::UnboundedReceiver<Msg>>>,
    events_tx: mpsc::UnboundedSender<Event>,
    events_rx: RwLock<Option<mpsc::UnboundedReceiver<Event>>>,

    // Filesystem storage
    base_path: PathBuf,

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

impl FilesystemNode {
    /// Create a new filesystem node
    pub fn new(base_path: PathBuf) -> Self {
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
            in_tx,
            in_rx: RwLock::new(Some(in_rx)),
            out_tx,
            out_rx: RwLock::new(Some(out_rx)),
            events_tx,
            events_rx: RwLock::new(Some(events_rx)),
            base_path,
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

    /// Ensure the base directory exists
    async fn ensure_base_dir(&self) -> Result<()> {
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path).await?;
        }
        Ok(())
    }

    /// Create channel directory if it doesn't exist
    async fn ensure_channel_dir(&self, channel_name: &str) -> Result<PathBuf> {
        let channel_path = self.base_path.join(channel_name);
        if !channel_path.exists() {
            fs::create_dir(&channel_path).await?;
        }
        Ok(channel_path)
    }

    /// Convert timestamp to hex filename
    fn timestamp_to_hex(timestamp: i64) -> String {
        format!("{:016x}", timestamp)
    }

    /// Convert hex filename to timestamp
    fn hex_to_timestamp(hex: &str) -> Result<i64> {
        i64::from_str_radix(hex, 16)
            .map_err(|e| RatNetError::InvalidArgument(format!("Invalid hex timestamp: {}", e)))
    }

    /// Save data to filesystem
    async fn save_message(&self, channel_name: Option<&str>, data: &[u8]) -> Result<()> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;

        let filename = Self::timestamp_to_hex(timestamp);

        let file_path = if let Some(channel) = channel_name {
            let channel_path = self.ensure_channel_dir(channel).await?;
            channel_path.join(&filename)
        } else {
            self.base_path.join(&filename)
        };

        let mut file = File::create(&file_path).await?;
        file.write_all(data).await?;

        Ok(())
    }

    /// Load data from filesystem
    async fn load_messages(&self, last_time: i64, max_bytes: i64) -> Result<Vec<(i64, Vec<u8>)>> {
        let mut messages = Vec::new();
        let mut bytes_read = 0i64;

        // Read from base directory
        let mut entries = fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Some(filename) = entry.file_name().to_str() {
                    if let Ok(timestamp) = Self::hex_to_timestamp(filename) {
                        if timestamp > last_time {
                            let data = fs::read(entry.path()).await?;

                            let proposed_size = data.len() as i64 + bytes_read;
                            if max_bytes > 0 && proposed_size > max_bytes {
                                if bytes_read == 0 {
                                    return Err(RatNetError::InvalidArgument(
                                        "Message too large for transport".to_string(),
                                    ));
                                }
                                break;
                            }

                            bytes_read += data.len() as i64;
                            messages.push((timestamp, data));
                        }
                    }
                }
            }
        }

        // Sort by timestamp
        messages.sort_by_key(|(timestamp, _)| *timestamp);

        Ok(messages)
    }

    /// Load messages from a specific channel
    async fn load_channel_messages(
        &self,
        channel_name: &str,
        last_time: i64,
        max_bytes: i64,
    ) -> Result<Vec<(i64, Vec<u8>)>> {
        let mut messages = Vec::new();
        let mut bytes_read = 0i64;

        let channel_path = self.base_path.join(channel_name);
        if !channel_path.exists() {
            return Ok(messages);
        }

        let mut entries = fs::read_dir(&channel_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Some(filename) = entry.file_name().to_str() {
                    if let Ok(timestamp) = Self::hex_to_timestamp(filename) {
                        if timestamp > last_time {
                            let data = fs::read(entry.path()).await?;

                            let proposed_size = data.len() as i64 + bytes_read;
                            if max_bytes > 0 && proposed_size > max_bytes {
                                if bytes_read == 0 {
                                    return Err(RatNetError::InvalidArgument(
                                        "Message too large for transport".to_string(),
                                    ));
                                }
                                break;
                            }

                            bytes_read += data.len() as i64;
                            messages.push((timestamp, data));
                        }
                    }
                }
            }
        }

        // Sort by timestamp
        messages.sort_by_key(|(timestamp, _)| *timestamp);

        Ok(messages)
    }

    async fn reassemble_stream(&self, stream_id: u32) -> Result<Vec<u8>> {
        if let Some(stream_info) = self.streams.get(&stream_id) {
            let mut complete_data = Vec::new();
            for i in 0..stream_info.total_chunks {
                if let Some(chunk) = self.chunks.get(&(stream_id, i)) {
                    complete_data.extend_from_slice(&chunk);
                } else {
                    return Err(RatNetError::InvalidArgument(format!(
                        "Missing chunk {} in stream {}",
                        i, stream_id
                    )));
                }
            }
            Ok(complete_data)
        } else {
            Err(RatNetError::InvalidArgument(format!(
                "Stream {} not found",
                stream_id
            )))
        }
    }

    async fn process_complete_message(&self, data: Vec<u8>) -> Result<()> {
        // For now, just log the message and send to events channel
        // In a full implementation, this would parse the message and take appropriate actions
        info!("Processing complete message of {} bytes", data.len());

        // Send to events channel if available
        let event = Event {
            event_type: "message_received".to_string(),
            data: std::collections::HashMap::new(),
        };
        if let Err(e) = self.events_tx.send(event) {
            warn!("Failed to send message to events channel: {}", e);
        }

        Ok(())
    }

    async fn process_complete_stream(&self, stream_id: u32) -> Result<()> {
        if let Some(stream_info) = self.streams.get(&stream_id) {
            // Reconstruct the complete message
            let mut complete_data = Vec::new();
            for chunk_num in 0..stream_info.total_chunks {
                if let Some(chunk_data) = self.chunks.get(&(stream_id, chunk_num)) {
                    complete_data.extend_from_slice(&chunk_data);
                } else {
                    return Err(RatNetError::InvalidArgument(format!(
                        "Missing chunk {} in stream {}",
                        chunk_num, stream_id
                    )));
                }
            }

            // Process the complete message
            self.process_complete_message(complete_data).await?;

            // Clean up stream data
            self.streams.remove(&stream_id);
            for chunk_num in 0..stream_info.total_chunks {
                self.chunks.remove(&(stream_id, chunk_num));
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for FilesystemNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilesystemNode")
            .field("running", &self.running.load(Ordering::Relaxed))
            .field("base_path", &self.base_path)
            .field("contacts", &self.contacts.len())
            .field("channels", &self.channels.len())
            .field("profiles", &self.profiles.len())
            .field("peers", &self.peers.len())
            .finish()
    }
}

#[async_trait]
impl Node for FilesystemNode {
    async fn start(&self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Err(RatNetError::InvalidArgument(
                "Node is already running".to_string(),
            ));
        }

        // Ensure base directory exists
        self.ensure_base_dir().await?;

        // Generate keys if needed
        self.ensure_routing_key().await?;
        self.ensure_content_key().await?;

        self.running.store(true, Ordering::Relaxed);
        info!("Filesystem node started at {:?}", self.base_path);

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(RatNetError::InvalidArgument(
                "Node is not running".to_string(),
            ));
        }

        self.running.store(false, Ordering::Relaxed);
        info!("Filesystem node stopped");

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn get_policies(&self) -> Vec<Arc<dyn Policy>> {
        // Use tokio::task::block_in_place to safely access the async RwLock from sync context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let policies_guard = self.policies.read().await;
                policies_guard.clone()
            })
        })
    }

    fn set_policy(&self, policies: Vec<Arc<dyn Policy>>) {
        // Use tokio::task::block_in_place to safely access the async RwLock from sync context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut policies_guard = self.policies.write().await;
                *policies_guard = policies;
                debug!("Updated policies: {} policies set", policies_guard.len());
            })
        })
    }

    fn router(&self) -> Arc<dyn Router> {
        // Use tokio::task::block_in_place to safely access the async RwLock from sync context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let router_guard = self.router.read().await;
                match router_guard.as_ref() {
                    Some(router) => router.clone(),
                    None => {
                        // Return a default router if none is set
                        Arc::new(crate::router::DefaultRouter::new())
                    }
                }
            })
        })
    }

    fn set_router(&self, router: Arc<dyn Router>) {
        let router_lock = self.router.clone();
        tokio::spawn(async move {
            let mut router_guard = router_lock.write().await;
            *router_guard = Some(router);
            debug!("Updated router");
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
        use crate::api::crypto;
        use serde_json;

        // Get the content key from the RwLock and use it within the scope
        let decrypted_content = {
            let content_key_guard = self.content_key.read().await;
            let content_key = content_key_guard.as_ref().ok_or_else(|| {
                RatNetError::InvalidArgument("Content key not initialized".to_string())
            })?;

            // Decrypt the message content using our content key
            crypto::decrypt(content_key, &msg.content.to_vec())?
        };

        // Deserialize the message
        let message_data: serde_json::Value = serde_json::from_slice(&decrypted_content)?;

        // Check if this is a chunked message
        if let Some(stream_id_str) = message_data.get("stream_id").and_then(|v| v.as_str()) {
            // Handle chunked message
            let stream_id = stream_id_str.parse::<u32>().map_err(|_| {
                RatNetError::InvalidArgument("Invalid stream_id format".to_string())
            })?;

            let chunk_data = message_data
                .get("chunk_data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    RatNetError::InvalidArgument("Missing chunk_data in stream message".to_string())
                })?;
            let chunk_index = message_data
                .get("chunk_index")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    RatNetError::InvalidArgument(
                        "Missing chunk_index in stream message".to_string(),
                    )
                })?;

            // Add chunk to stream
            self.add_chunk(
                stream_id,
                chunk_index as u32,
                chunk_data.as_bytes().to_vec().into(),
            )
            .await?;

            // Check if stream is complete
            self.check_stream_complete(stream_id).await?;

            // For now, return true to indicate message was handled
            Ok(true)
        } else {
            // Regular message - process directly
            self.process_complete_message(decrypted_content).await?;
            Ok(true)
        }
    }

    async fn forward(&self, msg: Msg) -> Result<()> {
        // Determine the channel name (if any)
        let channel_name = if msg.is_chan {
            Some(msg.name.as_str())
        } else {
            None
        };
        // Serialize the message (for now, just the content)
        let data = msg.content.as_ref();
        // Save the message to disk
        self.save_message(channel_name, data).await?;
        // Optionally, send to out channel for immediate delivery
        if let Err(e) = self.out_tx.send(msg) {
            warn!("Failed to send message to out channel: {}", e);
        }
        Ok(())
    }

    async fn add_stream(
        &self,
        stream_id: u32,
        total_chunks: u32,
        channel_name: String,
    ) -> Result<()> {
        let stream_info = StreamInfo {
            total_chunks,
            channel_name,
            received_chunks: HashMap::new(),
        };
        self.streams.insert(stream_id, stream_info);
        Ok(())
    }

    async fn add_chunk(&self, stream_id: u32, chunk_num: u32, data: Bytes) -> Result<()> {
        if let Some(mut stream_info) = self.streams.get_mut(&stream_id) {
            stream_info.received_chunks.insert(chunk_num, true);
            self.chunks.insert((stream_id, chunk_num), data);

            // Check if stream is complete
            self.check_stream_complete(stream_id).await?;
        } else {
            return Err(RatNetError::InvalidArgument(format!(
                "Stream {} not found",
                stream_id
            )));
        }
        Ok(())
    }

    async fn check_stream_complete(&self, stream_id: u32) -> Result<()> {
        if let Some(stream_info) = self.streams.get(&stream_id) {
            if stream_info.received_chunks.len() == stream_info.total_chunks as usize {
                // Stream is complete, process it
                self.process_complete_stream(stream_id).await?;
            }
        } else {
            return Err(RatNetError::InvalidArgument(format!(
                "Stream {} not found",
                stream_id
            )));
        }
        Ok(())
    }

    async fn flush_outbox(&self, max_age_seconds: i64) -> Result<()> {
        use std::time::{SystemTime, UNIX_EPOCH};
        use tokio::fs;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff = now - max_age_seconds;

        // Helper to remove old files in a directory
        async fn remove_old_files_in_dir(dir: &PathBuf, cutoff: i64) -> Result<()> {
            let mut entries = fs::read_dir(dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                        if let Ok(timestamp) = FilesystemNode::hex_to_timestamp(filename) {
                            if timestamp < cutoff {
                                fs::remove_file(&path).await?;
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        // Flush base path
        remove_old_files_in_dir(&self.base_path, cutoff).await?;

        // Flush all channel subdirectories
        let mut entries = fs::read_dir(&self.base_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                remove_old_files_in_dir(&path, cutoff).await?;
            }
        }
        Ok(())
    }

    async fn id(&self) -> Result<PubKey> {
        let routing_key = self.routing_key.read().await;
        routing_key
            .as_ref()
            .map(|k| k.public_key())
            .ok_or_else(|| RatNetError::InvalidArgument("Routing key not initialized".to_string()))
    }

    async fn dropoff(&self, bundle: Bundle) -> Result<()> {
        // For now, just save the bundle data to disk
        // In a full implementation, this would deserialize the bundle and process individual messages
        let data = bundle.data.to_vec();
        self.save_message(None, &data).await?;
        Ok(())
    }

    async fn pickup(
        &self,
        routing_pub: PubKey,
        last_time: i64,
        max_bytes: i64,
        channel_names: Vec<String>,
    ) -> Result<Bundle> {
        use std::collections::BTreeMap;
        use tokio::fs;

        let mut file_times: BTreeMap<i64, (PathBuf, Vec<u8>)> = BTreeMap::new();

        // Helper to collect messages from a directory
        async fn collect_messages_from_dir(
            dir: &PathBuf,
            file_times: &mut BTreeMap<i64, (PathBuf, Vec<u8>)>,
        ) -> Result<()> {
            let mut entries = fs::read_dir(dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                        if let Ok(timestamp) = FilesystemNode::hex_to_timestamp(filename) {
                            let data = fs::read(&path).await?;
                            file_times.insert(timestamp, (path, data));
                        }
                    }
                }
            }
            Ok(())
        }

        // Collect messages from base path
        collect_messages_from_dir(&self.base_path, &mut file_times).await?;

        // Collect messages from channel subdirectories
        let mut entries = fs::read_dir(&self.base_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                collect_messages_from_dir(&path, &mut file_times).await?;
            }
        }

        // Filter messages by time and size
        let mut bundle_data = Vec::new();
        let mut current_size = 0;

        for (timestamp, (_, data)) in file_times {
            if timestamp > last_time && current_size + data.len() <= max_bytes as usize {
                bundle_data.extend_from_slice(&data);
                current_size += data.len();
            }
        }

        Ok(Bundle {
            data: bundle_data.into(),
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        })
    }

    async fn cid(&self) -> Result<PubKey> {
        let content_key = self.content_key.read().await;
        content_key
            .as_ref()
            .map(|k| k.public_key())
            .ok_or_else(|| RatNetError::InvalidArgument("Content key not initialized".to_string()))
    }

    async fn get_contact(&self, name: &str) -> Result<Contact> {
        self.contacts
            .get(name)
            .map(|c| c.clone())
            .ok_or_else(|| RatNetError::NotFound(format!("Contact '{}' not found", name)))
    }

    async fn get_contacts(&self) -> Result<Vec<Contact>> {
        Ok(self.contacts.iter().map(|c| c.clone()).collect())
    }

    async fn add_contact(&self, name: String, key: String) -> Result<()> {
        // Validate the public key
        let _pubkey = PubKey::from_string(&key)?;

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
            .ok_or_else(|| RatNetError::NotFound(format!("Contact '{}' not found", name)))?;
        Ok(())
    }

    async fn get_channel(&self, name: &str) -> Result<Channel> {
        self.channels
            .get(name)
            .map(|c| Channel {
                name: c.name.clone(),
                pubkey: c.pubkey.clone(),
            })
            .ok_or_else(|| RatNetError::NotFound(format!("Channel '{}' not found", name)))
    }

    async fn get_channels(&self) -> Result<Vec<Channel>> {
        Ok(self
            .channels
            .iter()
            .map(|c| Channel {
                name: c.name.clone(),
                pubkey: c.pubkey.clone(),
            })
            .collect())
    }

    async fn add_channel(&self, name: String, privkey: String) -> Result<()> {
        // Parse the private key string into a KeyPair
        let keypair = KeyPair::from_string(&privkey)?;
        let pubkey = keypair.public_key().to_string();

        let channel_priv = ChannelPriv {
            name: name.clone(),
            pubkey,
            privkey: keypair,
        };

        self.channels.insert(name.clone(), channel_priv);

        // Ensure the channel directory exists
        self.ensure_channel_dir(&name).await?;

        info!("Added channel: {}", name);
        Ok(())
    }

    async fn delete_channel(&self, name: &str) -> Result<()> {
        self.channels
            .remove(name)
            .ok_or_else(|| RatNetError::NotFound(format!("Channel '{}' not found", name)))?;
        Ok(())
    }

    async fn get_channel_privkey(&self, name: &str) -> Result<String> {
        if let Some(channel) = self.channels.get(name) {
            channel.privkey.to_string()
        } else {
            Err(RatNetError::NotFound(format!(
                "Channel '{}' not found",
                name
            )))
        }
    }

    async fn get_profile(&self, name: &str) -> Result<Profile> {
        self.profiles
            .get(name)
            .map(|p| Profile {
                name: p.name.clone(),
                enabled: p.enabled,
                pubkey: p.privkey.public_key().to_string(),
            })
            .ok_or_else(|| RatNetError::NotFound(format!("Profile '{}' not found", name)))
    }

    async fn get_profiles(&self) -> Result<Vec<Profile>> {
        Ok(self
            .profiles
            .iter()
            .map(|p| Profile {
                name: p.name.clone(),
                enabled: p.enabled,
                pubkey: p.privkey.public_key().to_string(),
            })
            .collect())
    }

    async fn add_profile(&self, name: String, enabled: bool) -> Result<()> {
        let keypair = KeyPair::generate_ed25519()?;

        let profile = ProfilePriv {
            name: name.clone(),
            enabled,
            privkey: keypair,
        };

        self.profiles.insert(name, profile);
        Ok(())
    }

    async fn delete_profile(&self, name: &str) -> Result<()> {
        self.profiles
            .remove(name)
            .ok_or_else(|| RatNetError::NotFound(format!("Profile '{}' not found", name)))?;
        Ok(())
    }

    async fn load_profile(&self, name: &str) -> Result<PubKey> {
        self.profiles
            .get(name)
            .map(|p| p.privkey.public_key())
            .ok_or_else(|| RatNetError::NotFound(format!("Profile '{}' not found", name)))
    }

    async fn get_peer(&self, name: &str) -> Result<Peer> {
        self.peers
            .get(name)
            .map(|p| p.clone())
            .ok_or_else(|| RatNetError::NotFound(format!("Peer '{}' not found", name)))
    }

    async fn get_peers(&self, group: Option<String>) -> Result<Vec<Peer>> {
        let peers: Vec<Peer> = self
            .peers
            .iter()
            .filter(|p| {
                if let Some(ref group_filter) = group {
                    p.group == *group_filter
                } else {
                    true
                }
            })
            .map(|p| p.clone())
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
            group: group.unwrap_or_else(|| "default".to_string()),
        };

        self.peers.insert(name, peer);
        Ok(())
    }

    async fn delete_peer(&self, name: &str) -> Result<()> {
        self.peers
            .remove(name)
            .ok_or_else(|| RatNetError::NotFound(format!("Peer '{}' not found", name)))?;
        Ok(())
    }

    async fn send_msg(&self, msg: Msg) -> Result<()> {
        // Forward the message to be persisted
        self.forward(msg).await?;
        Ok(())
    }

    async fn admin_rpc(
        &self,
        _transport: Arc<dyn Transport>,
        call: RemoteCall,
    ) -> Result<RemoteResponse> {
        use crate::api::Action;
        use serde_json;

        debug!("Admin RPC call: {:?}", call.action);

        match call.action {
            Action::CID => match self.cid().await {
                Ok(pubkey) => Ok(RemoteResponse::success(serde_json::json!(
                    pubkey.to_string()
                ))),
                Err(e) => Ok(RemoteResponse::error(e.to_string())),
            },

            Action::GetContact => {
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
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
                let group = if call.args.len() > 0 {
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
                if call.args.len() < 1 {
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
                if call.args.len() < 1 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                // For now, we'll just return success since send_msg takes a Msg object
                // In a real implementation, we'd need to construct the Msg from the args
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
        use crate::api::Action;
        use serde_json;

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
                if call.args.len() < 2 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                // Parse routing public key
                let routing_pub_str = match &call.args[0] {
                    serde_json::Value::String(s) => s,
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                // Parse last time
                let last_time = match &call.args[1] {
                    serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
                    _ => return Ok(RemoteResponse::error("Invalid argument type".to_string())),
                };

                // Parse channel names (optional)
                let mut channel_names = Vec::new();
                for arg in call.args.iter().skip(2) {
                    if let serde_json::Value::String(s) = arg {
                        channel_names.push(s.clone());
                    } else {
                        return Ok(RemoteResponse::error("Invalid argument type".to_string()));
                    }
                }

                // For now, we'll use a placeholder PubKey since we need to parse the string
                // In a real implementation, we'd parse the routing_pub_str into a PubKey
                let routing_pub = PubKey::nil(); // Placeholder
                let max_bytes = transport.byte_limit();

                match self
                    .pickup(routing_pub, last_time, max_bytes, channel_names)
                    .await
                {
                    Ok(bundle) => Ok(RemoteResponse::success(serde_json::json!(bundle))),
                    Err(e) => Ok(RemoteResponse::error(e.to_string())),
                }
            }

            Action::Dropoff => {
                if call.args.len() < 1 {
                    return Ok(RemoteResponse::error("Invalid argument count".to_string()));
                }

                // Parse bundle
                let bundle_json = &call.args[0];
                match serde_json::from_value::<Bundle>(bundle_json.clone()) {
                    Ok(bundle) => match self.dropoff(bundle).await {
                        Ok(()) => Ok(RemoteResponse::success(serde_json::Value::Null)),
                        Err(e) => Ok(RemoteResponse::error(e.to_string())),
                    },
                    Err(e) => Ok(RemoteResponse::error(format!(
                        "Invalid bundle format: {}",
                        e
                    ))),
                }
            }

            _ => Ok(RemoteResponse::error(format!(
                "Unknown public action: {:?}",
                call.action
            ))),
        }
    }
}
