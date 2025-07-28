//! Database-backed node implementation

use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

use crate::api::*;
use crate::database::Database;
use crate::error::{Result, RatNetError};
use crate::nodes::MemoryNode;

/// Database-backed node that wraps a memory node with persistence
pub struct DatabaseNode {
    memory_node: Arc<MemoryNode>,
    database: Arc<dyn Database>,
}

impl DatabaseNode {
    /// Create a new database node
    pub async fn new(database: Arc<dyn Database>) -> Result<Self> {
        // Initialize the database schema
        database.bootstrap().await?;
        
        let memory_node = Arc::new(MemoryNode::new());
        
        let node = DatabaseNode {
            memory_node,
            database,
        };
        
        // Load existing keys from database if they exist
        node.load_keys().await?;
        
        Ok(node)
    }
    
    /// Load existing keys from database
    async fn load_keys(&self) -> Result<()> {
        // Check if we have content and routing keys stored
        if let Some(content_key) = self.database.get_config("contentkey").await? {
            debug!("Loaded content key from database");
            // TODO: When crypto is fully implemented, load the actual key
        } else {
            // Generate new keys and store them
            let content_key = KeyPair::generate_ed25519()?;
            let routing_key = KeyPair::generate_ed25519()?;
            
            // Serialize and store the keys
            let content_key_str = content_key.to_string()?;
            let routing_key_str = routing_key.to_string()?;
            
            self.database.set_config("contentkey".to_string(), content_key_str).await?;
            self.database.set_config("routingkey".to_string(), routing_key_str).await?;
            
            info!("Generated and stored new keys in database");
        }
        
        Ok(())
    }
    
    /// Save outbox message to database
    async fn save_outbox_msg(&self, msg: &Msg) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
            
        let channel = if msg.is_chan && !msg.name.is_empty() {
            Some(msg.name.clone())
        } else {
            None
        };
        
        self.database.add_outbox_msg(channel, &msg.content, timestamp).await?;
        Ok(())
    }
    
    /// Load data from database into memory node
    async fn sync_from_database(&self) -> Result<()> {
        // Load contacts
        let contacts = self.database.get_contacts().await?;
        for contact in contacts {
            self.memory_node.add_contact(contact.name, contact.pubkey).await?;
        }
        
        // Load peers
        let peers = self.database.get_peers(None).await?;
        for peer in peers {
            self.memory_node.add_peer(peer.name, peer.enabled, peer.uri, Some(peer.group)).await?;
        }
        
        debug!("Synchronized data from database to memory");
        Ok(())
    }
}

#[async_trait]
impl Node for DatabaseNode {
    async fn start(&self) -> Result<()> {
        info!("Starting database node");
        
        // Sync data from database to memory
        self.sync_from_database().await?;
        
        // Start the underlying memory node
        self.memory_node.start().await?;
        
        info!("Database node started");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        info!("Stopping database node");
        
        // Stop the underlying memory node
        self.memory_node.stop().await?;
        
        // Close database connection
        self.database.close().await?;
        
        info!("Database node stopped");
        Ok(())
    }
    
    fn is_running(&self) -> bool {
        self.memory_node.is_running()
    }
    
    fn get_policies(&self) -> Vec<Arc<dyn Policy>> {
        self.memory_node.get_policies()
    }
    
    fn set_policy(&self, policies: Vec<Arc<dyn Policy>>) {
        self.memory_node.set_policy(policies)
    }
    
    fn router(&self) -> Arc<dyn Router> {
        self.memory_node.router()
    }
    
    fn set_router(&self, router: Arc<dyn Router>) {
        self.memory_node.set_router(router)
    }
    
    fn in_channel(&self) -> tokio::sync::mpsc::Receiver<Msg> {
        self.memory_node.in_channel()
    }
    
    fn out_channel(&self) -> tokio::sync::mpsc::Receiver<Msg> {
        self.memory_node.out_channel()
    }
    
    fn events_channel(&self) -> tokio::sync::mpsc::Receiver<Event> {
        self.memory_node.events_channel()
    }
    
    async fn handle(&self, msg: Msg) -> Result<bool> {
        self.memory_node.handle(msg).await
    }
    
    async fn forward(&self, msg: Msg) -> Result<()> {
        self.memory_node.forward(msg).await
    }
    
    async fn add_stream(&self, stream_id: u32, total_chunks: u32, channel_name: String) -> Result<()> {
        // Save to database
        self.database.add_stream(stream_id, total_chunks, channel_name.clone()).await?;
        
        // Forward to memory node
        self.memory_node.add_stream(stream_id, total_chunks, channel_name).await
    }
    
    async fn add_chunk(&self, stream_id: u32, chunk_num: u32, data: Bytes) -> Result<()> {
        // Persist to database
        self.database.add_chunk(stream_id, chunk_num, &data).await?;
        
        // Add to memory node for active processing
        self.memory_node.add_chunk(stream_id, chunk_num, data).await
    }
    
    async fn check_stream_complete(&self, stream_id: u32) -> Result<()> {
        self.memory_node.check_stream_complete(stream_id).await
    }
    
    async fn flush_outbox(&self, max_age_seconds: i64) -> Result<()> {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - max_age_seconds;
            
        self.database.delete_outbox_msgs(cutoff_time).await?;
        self.memory_node.flush_outbox(max_age_seconds).await
    }
    
    async fn id(&self) -> Result<PubKey> {
        self.memory_node.id().await
    }
    
    async fn dropoff(&self, bundle: Bundle) -> Result<()> {
        self.memory_node.dropoff(bundle).await
    }
    
    async fn pickup(&self, routing_pub: PubKey, last_time: i64, max_bytes: i64, channel_names: Vec<String>) -> Result<Bundle> {
        // Get messages from database
        let outbox_msgs = self.database.get_outbox_msgs(last_time).await?;
        
        // Filter by channels if specified
        let filtered_msgs: Vec<_> = if channel_names.is_empty() {
            outbox_msgs
        } else {
            outbox_msgs.into_iter()
                .filter(|msg| {
                    if let Some(ref channel) = msg.channel.as_ref() {
                        channel_names.contains(channel)
                    } else {
                        true // Include messages without channel
                    }
                })
                .collect()
        };
        
        // TODO: Serialize messages into bundle format respecting max_bytes limit
        // For now, return empty bundle like memory node
        self.memory_node.pickup(routing_pub, last_time, max_bytes, channel_names).await
    }
    
    async fn cid(&self) -> Result<PubKey> {
        self.memory_node.cid().await
    }
    
    // Contact operations - delegate to database
    async fn get_contact(&self, name: &str) -> Result<Contact> {
        if let Some(contact) = self.database.get_contact(name).await? {
            Ok(contact)
        } else {
            Err(RatNetError::NotFound(format!("Contact '{}' not found", name)))
        }
    }
    
    async fn get_contacts(&self) -> Result<Vec<Contact>> {
        self.database.get_contacts().await
    }
    
    async fn add_contact(&self, name: String, key: String) -> Result<()> {
        self.database.add_contact(name, key).await
    }
    
    async fn delete_contact(&self, name: &str) -> Result<()> {
        self.database.delete_contact(name).await
    }
    
    // Channel operations - delegate to database
    async fn get_channel(&self, name: &str) -> Result<Channel> {
        if let Some(channel) = self.database.get_channel(name).await? {
            Ok(channel)
        } else {
            Err(RatNetError::NotFound(format!("Channel '{}' not found", name)))
        }
    }
    
    async fn get_channels(&self) -> Result<Vec<Channel>> {
        self.database.get_channels().await
    }
    
    async fn add_channel(&self, name: String, privkey: String) -> Result<()> {
        self.database.add_channel(name, privkey).await
    }
    
    async fn delete_channel(&self, name: &str) -> Result<()> {
        self.database.delete_channel(name).await
    }
    
    async fn get_channel_privkey(&self, name: &str) -> Result<String> {
        if let Some(privkey) = self.database.get_channel_privkey(name).await? {
            Ok(privkey)
        } else {
            Err(RatNetError::NotFound(format!("Channel '{}' not found", name)))
        }
    }
    
    // Profile operations - delegate to database
    async fn get_profile(&self, name: &str) -> Result<Profile> {
        if let Some(profile) = self.database.get_profile(name).await? {
            Ok(profile)
        } else {
            Err(RatNetError::NotFound(format!("Profile '{}' not found", name)))
        }
    }
    
    async fn get_profiles(&self) -> Result<Vec<Profile>> {
        self.database.get_profiles().await
    }
    
    async fn add_profile(&self, name: String, enabled: bool) -> Result<()> {
        // Generate a new key pair for the profile
        let keypair = KeyPair::generate_ed25519()?;
        let privkey_str = keypair.to_string()?;
        
        self.database.add_profile(name, enabled, privkey_str).await?;
        Ok(())
    }
    
    async fn delete_profile(&self, name: &str) -> Result<()> {
        self.database.delete_profile(name).await
    }
    
    async fn load_profile(&self, name: &str) -> Result<PubKey> {
        if let Some(privkey) = self.database.get_profile_privkey(name).await? {
            // Parse the private key and return the public key
            let keypair = KeyPair::from_string(&privkey)
                .map_err(|e| RatNetError::Crypto(format!("Invalid private key: {}", e)))?;
            Ok(keypair.public_key())
        } else {
            Err(RatNetError::NotFound(format!("Profile '{}' not found", name)))
        }
    }
    
    // Peer operations - delegate to database
    async fn get_peer(&self, name: &str) -> Result<Peer> {
        if let Some(peer) = self.database.get_peer(name).await? {
            Ok(peer)
        } else {
            Err(RatNetError::NotFound(format!("Peer '{}' not found", name)))
        }
    }
    
    async fn get_peers(&self, group: Option<String>) -> Result<Vec<Peer>> {
        self.database.get_peers(group.as_deref()).await
    }
    
    async fn add_peer(&self, name: String, enabled: bool, uri: String, group: Option<String>) -> Result<()> {
        let group = group.unwrap_or_default();
        self.database.add_peer(name, enabled, uri, group).await
    }
    
    async fn delete_peer(&self, name: &str) -> Result<()> {
        self.database.delete_peer(name).await
    }
    
    async fn send_msg(&self, msg: Msg) -> Result<()> {
        // Save to outbox in database
        self.save_outbox_msg(&msg).await?;
        
        // Forward to memory node for immediate processing
        self.memory_node.send_msg(msg).await
    }
    
    async fn admin_rpc(&self, transport: Arc<dyn Transport>, call: RemoteCall) -> Result<RemoteResponse> {
        self.memory_node.admin_rpc(transport, call).await
    }
    
    async fn public_rpc(&self, transport: Arc<dyn Transport>, call: RemoteCall) -> Result<RemoteResponse> {
        self.memory_node.public_rpc(transport, call).await
    }
} 