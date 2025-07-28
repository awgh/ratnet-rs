//! Database abstraction and implementations for RatNet

use async_trait::async_trait;

use crate::api::*;
use crate::error::Result;

pub mod migrations;
pub mod sqlite;

pub use migrations::{default_migrations, Migration, MigrationManager};
pub use sqlite::SqliteDatabase;

/// Database abstraction trait for RatNet persistence
#[async_trait]
pub trait Database: Send + Sync {
    /// Initialize the database with required schema
    async fn bootstrap(&self) -> Result<()>;

    /// Close the database connection
    async fn close(&self) -> Result<()>;

    // Contact operations
    async fn get_contact(&self, name: &str) -> Result<Option<Contact>>;
    async fn get_contacts(&self) -> Result<Vec<Contact>>;
    async fn add_contact(&self, name: String, pubkey: String) -> Result<()>;
    async fn delete_contact(&self, name: &str) -> Result<()>;

    // Channel operations
    async fn get_channel(&self, name: &str) -> Result<Option<Channel>>;
    async fn get_channels(&self) -> Result<Vec<Channel>>;
    async fn get_channel_privkey(&self, name: &str) -> Result<Option<String>>;
    async fn add_channel(&self, name: String, privkey: String) -> Result<()>;
    async fn delete_channel(&self, name: &str) -> Result<()>;

    // Profile operations
    async fn get_profile(&self, name: &str) -> Result<Option<Profile>>;
    async fn get_profiles(&self) -> Result<Vec<Profile>>;
    async fn get_profile_privkey(&self, name: &str) -> Result<Option<String>>;
    async fn add_profile(&self, name: String, enabled: bool, privkey: String) -> Result<()>;
    async fn delete_profile(&self, name: &str) -> Result<()>;

    // Peer operations
    async fn get_peer(&self, name: &str) -> Result<Option<Peer>>;
    async fn get_peers(&self, group: Option<&str>) -> Result<Vec<Peer>>;
    async fn add_peer(&self, name: String, enabled: bool, uri: String, group: String)
        -> Result<()>;
    async fn delete_peer(&self, name: &str) -> Result<()>;

    // Configuration operations
    async fn get_config(&self, name: &str) -> Result<Option<String>>;
    async fn set_config(&self, name: String, value: String) -> Result<()>;
    async fn delete_config(&self, name: &str) -> Result<()>;

    // Outbox operations
    async fn add_outbox_msg(
        &self,
        channel: Option<String>,
        msg: &[u8],
        timestamp: i64,
    ) -> Result<()>;
    async fn get_outbox_msgs(&self, since: i64) -> Result<Vec<OutboxMsg>>;
    async fn delete_outbox_msgs(&self, before: i64) -> Result<()>;

    // Streaming operations
    async fn add_stream(&self, stream_id: u32, parts: u32, channel: String) -> Result<()>;
    async fn get_stream(&self, stream_id: u32) -> Result<Option<StreamHeader>>;
    async fn delete_stream(&self, stream_id: u32) -> Result<()>;

    async fn add_chunk(&self, stream_id: u32, chunk_num: u32, data: &[u8]) -> Result<()>;
    async fn get_chunk(&self, stream_id: u32, chunk_num: u32) -> Result<Option<Chunk>>;
    async fn get_stream_chunks(&self, stream_id: u32) -> Result<Vec<Chunk>>;
    async fn delete_stream_chunks(&self, stream_id: u32) -> Result<()>;
}
