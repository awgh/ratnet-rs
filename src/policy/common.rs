//! Common policy utilities and peer management
//!
//! This module provides shared functionality used by multiple policy implementations,
//! including peer table management and polling operations.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

use crate::api::crypto::PubKey;
use crate::api::{Action, Bundle, Node, PeerInfo, RemoteCall, RemoteResponse, Transport};
use crate::error::{RatNetError, Result};

/// PeerTable manages information about known peers and their connection state
#[derive(Debug)]
pub struct PeerTable {
    peer_table: RwLock<HashMap<String, Arc<PeerInfo>>>,
}

impl PeerTable {
    /// Create a new empty peer table
    pub fn new() -> Self {
        Self {
            peer_table: RwLock::new(HashMap::new()),
        }
    }

    /// Read peer information for a given host
    async fn read_peer_table(&self, key: &str) -> Option<Arc<PeerInfo>> {
        let table = self.peer_table.read().await;
        table.get(key).cloned()
    }

    /// Write peer information for a given host  
    async fn write_peer_table(&self, key: String, val: Arc<PeerInfo>) {
        let mut table = self.peer_table.write().await;
        table.insert(key, val);
    }

    /// Perform a push/pull operation between local and remote nodes
    ///
    /// This is the core peer synchronization mechanism that:
    /// 1. Gets the remote node's routing public key (if not cached)
    /// 2. Picks up local messages for the remote peer
    /// 3. Picks up remote messages for the local node
    /// 4. Exchanges the messages via RPC calls
    /// 5. Updates transfer statistics and timestamps
    pub async fn poll_server(
        &self,
        transport: Arc<dyn Transport>,
        node: Arc<dyn Node>,
        host: &str,
        pubsrv: PubKey,
    ) -> Result<bool> {
        // PollServer should be non-reentrant - use host-specific locking if needed

        // Make PeerInfo for this host if it doesn't exist
        if self.read_peer_table(host).await.is_none() {
            self.write_peer_table(
                host.to_string(),
                Arc::new(PeerInfo {
                    last_poll_local: AtomicI64::new(0),
                    last_poll_remote: AtomicI64::new(0),
                    total_bytes_tx: AtomicI64::new(0),
                    total_bytes_rx: AtomicI64::new(0),
                    routing_pub: None,
                }),
            )
            .await;
        }

        let peer = self.read_peer_table(host).await.unwrap();

        // Get remote routing public key if we don't have it cached
        if peer.routing_pub.is_none() {
            debug!("Getting routing key for peer: {}", host);
            let rpubkey_result = transport.rpc(host, Action::ID, vec![]).await;
            match rpubkey_result {
                Ok(response) => {
                    if let Ok(pubkey) = serde_json::from_value::<PubKey>(response) {
                        // Create new PeerInfo with updated routing_pub
                        let updated_peer = Arc::new(PeerInfo {
                            last_poll_local: AtomicI64::new(
                                peer.last_poll_local.load(Ordering::Relaxed),
                            ),
                            last_poll_remote: AtomicI64::new(
                                peer.last_poll_remote.load(Ordering::Relaxed),
                            ),
                            total_bytes_tx: AtomicI64::new(
                                peer.total_bytes_tx.load(Ordering::Relaxed),
                            ),
                            total_bytes_rx: AtomicI64::new(
                                peer.total_bytes_rx.load(Ordering::Relaxed),
                            ),
                            routing_pub: Some(pubkey),
                        });
                        self.write_peer_table(host.to_string(), updated_peer.clone())
                            .await;
                        let peer = updated_peer;
                    } else {
                        error!("Failed to deserialize remote routing key");
                        return Err(RatNetError::Serialization(
                            "Invalid routing key format".to_string(),
                        ));
                    }
                }
                Err(e) => {
                    error!("Failed to get remote routing key: {}", e);
                    return Err(e);
                }
            }
        }

        let routing_pub = peer.routing_pub.as_ref().unwrap().clone();
        let last_poll_local = peer.last_poll_local.load(Ordering::Relaxed);
        let last_poll_remote = peer.last_poll_remote.load(Ordering::Relaxed);

        // Pickup Local - get messages to send to remote peer
        let to_remote = node
            .pickup(
                routing_pub.clone(),
                last_poll_local,
                transport.byte_limit(),
                vec![], // empty channel filter for now
            )
            .await?;

        debug!("Local pickup result length: {}", to_remote.data.len());

        // Pickup Remote - get messages from remote peer
        let to_local_raw = transport
            .rpc(
                host,
                Action::Pickup,
                vec![
                    serde_json::to_value(pubsrv)?,
                    serde_json::to_value(last_poll_remote)?,
                ],
            )
            .await;

        let mut to_local = None;
        match to_local_raw {
            Ok(response) => {
                if let Ok(bundle) = serde_json::from_value::<Bundle>(response) {
                    debug!("Remote pickup result length: {}", bundle.data.len());
                    peer.total_bytes_rx
                        .fetch_add(bundle.data.len() as i64, Ordering::Relaxed);
                    to_local = Some(bundle);
                } else {
                    debug!("Remote pickup returned no data or invalid format");
                }
            }
            Err(e) => {
                error!("Remote pickup error: {}", e);
                return Err(e);
            }
        }

        // Dropoff Remote - send local messages to remote peer
        if !to_remote.data.is_empty() {
            match transport
                .rpc(
                    host,
                    Action::Dropoff,
                    vec![serde_json::to_value(&to_remote)?],
                )
                .await
            {
                Ok(_) => {
                    // Only start tracking time once we start receiving data
                    if peer.total_bytes_tx.load(Ordering::Relaxed) > 0 {
                        peer.last_poll_local
                            .store(to_remote.time, Ordering::Relaxed);
                    }
                    peer.total_bytes_tx
                        .fetch_add(to_remote.data.len() as i64, Ordering::Relaxed);
                }
                Err(e) => {
                    error!("Remote dropoff error: {}", e);
                    return Err(e);
                }
            }
        }

        // Dropoff Local - process messages received from remote peer
        if let Some(bundle) = to_local {
            if !bundle.data.is_empty() {
                if let Err(e) = node.dropoff(bundle.clone()).await {
                    error!("Local dropoff error: {}", e);
                    return Err(e);
                }
                if peer.total_bytes_rx.load(Ordering::Relaxed) > 0 {
                    peer.last_poll_remote.store(bundle.time, Ordering::Relaxed);
                }
            }
        }

        Ok(true)
    }

    /// Get all known peers
    pub async fn get_peers(&self) -> Vec<String> {
        let table = self.peer_table.read().await;
        table.keys().cloned().collect()
    }

    /// Get peer statistics
    pub async fn get_peer_stats(&self, host: &str) -> Option<(i64, i64, i64, i64)> {
        if let Some(peer) = self.read_peer_table(host).await {
            Some((
                peer.last_poll_local.load(Ordering::Relaxed),
                peer.last_poll_remote.load(Ordering::Relaxed),
                peer.total_bytes_tx.load(Ordering::Relaxed),
                peer.total_bytes_rx.load(Ordering::Relaxed),
            ))
        } else {
            None
        }
    }

    /// Remove a peer from the table
    pub async fn remove_peer(&self, host: &str) -> bool {
        let mut table = self.peer_table.write().await;
        table.remove(host).is_some()
    }

    /// Clear all peers
    pub async fn clear(&self) {
        let mut table = self.peer_table.write().await;
        table.clear();
    }

    /// Add a discovered peer to the table
    pub async fn add_peer(&self, key: String, address: String, port: u16) {
        let peer = Arc::new(PeerInfo {
            last_poll_local: AtomicI64::new(0),
            last_poll_remote: AtomicI64::new(0),
            total_bytes_tx: AtomicI64::new(0),
            total_bytes_rx: AtomicI64::new(0),
            routing_pub: None, // Will be discovered on first poll
        });

        self.write_peer_table(key, peer).await;
    }
}

impl Default for PeerTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicI64;

    #[tokio::test]
    async fn test_peer_table_basic_operations() {
        let table = PeerTable::new();

        // Test empty table
        assert!(table.read_peer_table("test").await.is_none());
        assert_eq!(table.get_peers().await.len(), 0);

        // Add a peer
        let peer = Arc::new(PeerInfo {
            last_poll_local: AtomicI64::new(100),
            last_poll_remote: AtomicI64::new(200),
            total_bytes_tx: AtomicI64::new(1024),
            total_bytes_rx: AtomicI64::new(2048),
            routing_pub: Some(PubKey::nil()),
        });

        table.write_peer_table("test".to_string(), peer).await;

        // Verify peer was added
        assert!(table.read_peer_table("test").await.is_some());
        assert_eq!(table.get_peers().await.len(), 1);

        // Check stats
        let stats = table.get_peer_stats("test").await.unwrap();
        assert_eq!(stats, (100, 200, 1024, 2048));

        // Remove peer
        assert!(table.remove_peer("test").await);
        assert!(table.read_peer_table("test").await.is_none());
        assert_eq!(table.get_peers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_peer_table_clear() {
        let table = PeerTable::new();

        // Add multiple peers
        for i in 0..5 {
            let peer = Arc::new(PeerInfo {
                last_poll_local: AtomicI64::new(i * 100),
                last_poll_remote: AtomicI64::new(i * 200),
                total_bytes_tx: AtomicI64::new(i * 1024),
                total_bytes_rx: AtomicI64::new(i * 2048),
                routing_pub: Some(PubKey::nil()),
            });
            table.write_peer_table(format!("peer{}", i), peer).await;
        }

        assert_eq!(table.get_peers().await.len(), 5);

        table.clear().await;
        assert_eq!(table.get_peers().await.len(), 0);
    }
}
