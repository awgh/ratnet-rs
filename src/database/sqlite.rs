//! SQLite database implementation for RatNet

#[cfg(feature = "sqlite")]
use async_trait::async_trait;
#[cfg(feature = "sqlite")]
use bytes::Bytes;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
#[cfg(feature = "sqlite")]
use sqlx::{Row, Sqlite, Transaction};
#[cfg(feature = "sqlite")]
use std::path::Path;
#[cfg(feature = "sqlite")]
use tracing::{debug, info};

#[cfg(feature = "sqlite")]
use crate::api::*;
#[cfg(feature = "sqlite")]
use crate::crypto::KeyPair;
#[cfg(feature = "sqlite")]
use crate::database::{default_migrations, Database, MigrationManager};
#[cfg(feature = "sqlite")]
use crate::error::{RatNetError, Result};

/// SQLite database implementation
#[cfg(feature = "sqlite")]
pub struct SqliteDatabase {
    pool: SqlitePool,
}

#[cfg(feature = "sqlite")]
impl SqliteDatabase {
    /// Create a new SQLite database instance
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to SQLite database: {}", database_url);

        // Create database file if it doesn't exist
        if database_url.starts_with("sqlite://") {
            let path = database_url
                .strip_prefix("sqlite://")
                .unwrap_or(database_url);
            if let Some(parent) = Path::new(path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| RatNetError::Database(format!("Failed to connect to database: {}", e)))?;

        Ok(SqliteDatabase { pool })
    }

    /// Create a new in-memory SQLite database for testing
    pub async fn new_memory() -> Result<Self> {
        Self::new("sqlite::memory:").await
    }
}

#[cfg(feature = "sqlite")]
#[async_trait]
impl Database for SqliteDatabase {
    async fn bootstrap(&self) -> Result<()> {
        info!("Bootstrapping database schema using migrations");

        let migration_manager = default_migrations();
        migration_manager.migrate(&self.pool).await?;

        info!("Database schema bootstrapped successfully");
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.pool.close().await;
        Ok(())
    }

    // Contact operations
    async fn get_contact(&self, name: &str) -> Result<Option<Contact>> {
        let row = sqlx::query("SELECT name, pubkey FROM contacts WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            Ok(Some(Contact {
                name: row.get("name"),
                pubkey: row.get("pubkey"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_contacts(&self) -> Result<Vec<Contact>> {
        let rows = sqlx::query("SELECT name, pubkey FROM contacts ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        let contacts = rows
            .into_iter()
            .map(|row| Contact {
                name: row.get("name"),
                pubkey: row.get("pubkey"),
            })
            .collect();

        Ok(contacts)
    }

    async fn add_contact(&self, name: String, pubkey: String) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO contacts (name, pubkey) VALUES (?, ?)")
            .bind(&name)
            .bind(&pubkey)
            .execute(&self.pool)
            .await?;

        debug!("Added contact: {}", name);
        Ok(())
    }

    async fn delete_contact(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM contacts WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        debug!("Deleted contact: {}", name);
        Ok(())
    }

    // Channel operations
    async fn get_channel(&self, name: &str) -> Result<Option<Channel>> {
        let row = sqlx::query("SELECT name, privkey FROM channels WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let privkey: String = row.get("privkey");
            // Derive pubkey from privkey
            let keypair = KeyPair::from_string(&privkey)
                .map_err(|e| RatNetError::Crypto(format!("Invalid private key: {}", e)))?;
            let pubkey = keypair.public_key();

            Ok(Some(Channel {
                name: row.get("name"),
                pubkey: pubkey.to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_channels(&self) -> Result<Vec<Channel>> {
        let rows = sqlx::query("SELECT name, privkey FROM channels ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        let channels = rows
            .into_iter()
            .map(|row| {
                let privkey: String = row.get("privkey");
                // Derive pubkey from privkey
                let keypair = KeyPair::from_string(&privkey)
                    .map_err(|e| RatNetError::Crypto(format!("Invalid private key: {}", e)))?;
                let pubkey = keypair.public_key();

                Ok(Channel {
                    name: row.get("name"),
                    pubkey: pubkey.to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(channels)
    }

    async fn get_channel_privkey(&self, name: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT privkey FROM channels WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("privkey")))
    }

    async fn add_channel(&self, name: String, privkey: String) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO channels (name, privkey) VALUES (?, ?)")
            .bind(&name)
            .bind(&privkey)
            .execute(&self.pool)
            .await?;

        debug!("Added channel: {}", name);
        Ok(())
    }

    async fn delete_channel(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM channels WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        debug!("Deleted channel: {}", name);
        Ok(())
    }

    // Profile operations
    async fn get_profile(&self, name: &str) -> Result<Option<Profile>> {
        let row = sqlx::query("SELECT name, privkey, enabled FROM profiles WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let privkey: String = row.get("privkey");
            // Derive pubkey from privkey
            let keypair = KeyPair::from_string(&privkey)
                .map_err(|e| RatNetError::Crypto(format!("Invalid private key: {}", e)))?;
            let pubkey = keypair.public_key();

            Ok(Some(Profile {
                name: row.get("name"),
                enabled: row.get("enabled"),
                pubkey: pubkey.to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_profiles(&self) -> Result<Vec<Profile>> {
        let rows = sqlx::query("SELECT name, privkey, enabled FROM profiles ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        let profiles = rows
            .into_iter()
            .map(|row| {
                let privkey: String = row.get("privkey");
                // Derive pubkey from privkey
                let keypair = KeyPair::from_string(&privkey)
                    .map_err(|e| RatNetError::Crypto(format!("Invalid private key: {}", e)))?;
                let pubkey = keypair.public_key();

                Ok(Profile {
                    name: row.get("name"),
                    enabled: row.get("enabled"),
                    pubkey: pubkey.to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(profiles)
    }

    async fn get_profile_privkey(&self, name: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT privkey FROM profiles WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("privkey")))
    }

    async fn add_profile(&self, name: String, enabled: bool, privkey: String) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO profiles (name, privkey, enabled) VALUES (?, ?, ?)")
            .bind(&name)
            .bind(&privkey)
            .bind(enabled)
            .execute(&self.pool)
            .await?;

        debug!("Added profile: {}", name);
        Ok(())
    }

    async fn delete_profile(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM profiles WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        debug!("Deleted profile: {}", name);
        Ok(())
    }

    // Peer operations
    async fn get_peer(&self, name: &str) -> Result<Option<Peer>> {
        let row = sqlx::query("SELECT name, uri, enabled, peergroup FROM peers WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            Ok(Some(Peer {
                name: row.get("name"),
                enabled: row.get("enabled"),
                uri: row.get("uri"),
                group: row.get("peergroup"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_peers(&self, group: Option<&str>) -> Result<Vec<Peer>> {
        let rows = if let Some(group) = group {
            sqlx::query(
                "SELECT name, uri, enabled, peergroup FROM peers WHERE peergroup = ? ORDER BY name",
            )
            .bind(group)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query("SELECT name, uri, enabled, peergroup FROM peers ORDER BY name")
                .fetch_all(&self.pool)
                .await?
        };

        let peers = rows
            .into_iter()
            .map(|row| Peer {
                name: row.get("name"),
                enabled: row.get("enabled"),
                uri: row.get("uri"),
                group: row.get("peergroup"),
            })
            .collect();

        Ok(peers)
    }

    async fn add_peer(
        &self,
        name: String,
        enabled: bool,
        uri: String,
        group: String,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO peers (name, uri, enabled, peergroup) VALUES (?, ?, ?, ?)",
        )
        .bind(&name)
        .bind(&uri)
        .bind(enabled)
        .bind(&group)
        .execute(&self.pool)
        .await?;

        debug!("Added peer: {}", name);
        Ok(())
    }

    async fn delete_peer(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM peers WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        debug!("Deleted peer: {}", name);
        Ok(())
    }

    // Configuration operations
    async fn get_config(&self, name: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM config WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("value")))
    }

    async fn set_config(&self, name: String, value: String) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO config (name, value) VALUES (?, ?)")
            .bind(&name)
            .bind(&value)
            .execute(&self.pool)
            .await?;

        debug!("Set config: {} = {}", name, value);
        Ok(())
    }

    async fn delete_config(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM config WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        debug!("Deleted config: {}", name);
        Ok(())
    }

    // Outbox operations
    async fn add_outbox_msg(
        &self,
        channel: Option<String>,
        msg: &[u8],
        timestamp: i64,
    ) -> Result<()> {
        sqlx::query("INSERT INTO outbox (channel, msg, timestamp) VALUES (?, ?, ?)")
            .bind(channel)
            .bind(msg)
            .bind(timestamp)
            .execute(&self.pool)
            .await?;

        debug!("Added outbox message with timestamp: {}", timestamp);
        Ok(())
    }

    async fn get_outbox_msgs(&self, since: i64) -> Result<Vec<OutboxMsg>> {
        let rows = sqlx::query(
            "SELECT channel, msg, timestamp FROM outbox WHERE timestamp > ? ORDER BY timestamp",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        let msgs = rows
            .into_iter()
            .map(|row| OutboxMsg {
                channel: row.get("channel"),
                msg: Bytes::from(row.get::<Vec<u8>, _>("msg")),
                timestamp: row.get("timestamp"),
            })
            .collect();

        Ok(msgs)
    }

    async fn delete_outbox_msgs(&self, before: i64) -> Result<()> {
        let result = sqlx::query("DELETE FROM outbox WHERE timestamp < ?")
            .bind(before)
            .execute(&self.pool)
            .await?;

        debug!(
            "Deleted {} outbox messages before timestamp {}",
            result.rows_affected(),
            before
        );
        Ok(())
    }

    // Streaming operations
    async fn add_stream(&self, stream_id: u32, parts: u32, channel: String) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO streams (streamid, parts, channel) VALUES (?, ?, ?)")
            .bind(stream_id as i64)
            .bind(parts as i64)
            .bind(&channel)
            .execute(&self.pool)
            .await?;

        debug!(
            "Added stream: id={}, parts={}, channel={}",
            stream_id, parts, channel
        );
        Ok(())
    }

    async fn get_stream(&self, stream_id: u32) -> Result<Option<StreamHeader>> {
        let row = sqlx::query("SELECT streamid, parts, channel FROM streams WHERE streamid = ?")
            .bind(stream_id as i64)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            Ok(Some(StreamHeader {
                stream_id,
                num_chunks: row.get::<i64, _>("parts") as u32,
                channel_name: row.get("channel"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_stream(&self, stream_id: u32) -> Result<()> {
        sqlx::query("DELETE FROM streams WHERE streamid = ?")
            .bind(stream_id as i64)
            .execute(&self.pool)
            .await?;

        debug!("Deleted stream: {}", stream_id);
        Ok(())
    }

    async fn add_chunk(&self, stream_id: u32, chunk_num: u32, data: &[u8]) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO chunks (streamid, chunknum, data) VALUES (?, ?, ?)")
            .bind(stream_id as i64)
            .bind(chunk_num as i64)
            .bind(data)
            .execute(&self.pool)
            .await?;

        debug!(
            "Added chunk: stream={}, chunk={}, size={}",
            stream_id,
            chunk_num,
            data.len()
        );
        Ok(())
    }

    async fn get_chunk(&self, stream_id: u32, chunk_num: u32) -> Result<Option<Chunk>> {
        let row = sqlx::query(
            "SELECT streamid, chunknum, data FROM chunks WHERE streamid = ? AND chunknum = ?",
        )
        .bind(stream_id as i64)
        .bind(chunk_num as i64)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Chunk {
                stream_id,
                chunk_num,
                data: Bytes::from(row.get::<Vec<u8>, _>("data")),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_stream_chunks(&self, stream_id: u32) -> Result<Vec<Chunk>> {
        let rows = sqlx::query(
            "SELECT streamid, chunknum, data FROM chunks WHERE streamid = ? ORDER BY chunknum",
        )
        .bind(stream_id as i64)
        .fetch_all(&self.pool)
        .await?;

        let chunks = rows
            .into_iter()
            .map(|row| Chunk {
                stream_id,
                chunk_num: row.get::<i64, _>("chunknum") as u32,
                data: Bytes::from(row.get::<Vec<u8>, _>("data")),
            })
            .collect();

        Ok(chunks)
    }

    async fn delete_stream_chunks(&self, stream_id: u32) -> Result<()> {
        sqlx::query("DELETE FROM chunks WHERE streamid = ?")
            .bind(stream_id as i64)
            .execute(&self.pool)
            .await?;

        debug!("Deleted chunks for stream: {}", stream_id);
        Ok(())
    }
}

#[cfg(not(feature = "sqlite"))]
pub struct SqliteDatabase;

#[cfg(not(feature = "sqlite"))]
impl SqliteDatabase {
    pub async fn new(_database_url: &str) -> crate::error::Result<Self> {
        Err(crate::error::RatNetError::Feature("sqlite feature not enabled".to_string()))
    }

    pub async fn new_memory() -> crate::error::Result<Self> {
        Err(crate::error::RatNetError::Feature("sqlite feature not enabled".to_string()))
    }
}
