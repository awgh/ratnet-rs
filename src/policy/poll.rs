//! Basic polling policy implementation

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

use crate::api::{Node, Policy, Transport};
use crate::error::Result;

/// Simple polling policy
pub struct PollPolicy {
    transport: Arc<dyn Transport>,
    node: Arc<dyn Node>,
    running: Arc<AtomicBool>,
}

impl PollPolicy {
    /// Create a new polling policy
    pub fn new(transport: Arc<dyn Transport>, node: Arc<dyn Node>) -> Self {
        Self {
            transport,
            node,
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Policy for PollPolicy {
    async fn run_policy(&self) -> Result<()> {
        if self.running.swap(true, Ordering::Relaxed) {
            return Ok(()); // Already running
        }
        
        info!("Starting poll policy");
        
        let transport = self.transport.clone();
        let node = self.node.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(30)); // Poll every 30 seconds
            
            while running.load(Ordering::Relaxed) {
                ticker.tick().await;
                
                debug!("Poll policy tick");
                
                // In a full implementation, this would:
                // 1. Get list of peers from the node
                // 2. Contact each peer to pickup messages
                // 3. Process any received messages
                // 4. Send any pending messages
                
                if let Ok(peers) = node.get_peers(None).await {
                    for peer in peers.iter().filter(|p| p.enabled) {
                        debug!("Would poll peer: {}", peer.name);
                        // Actual polling logic would go here
                    }
                }
            }
            
            info!("Poll policy stopped");
        });
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        info!("Stopping poll policy");
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }
    
    fn get_transport(&self) -> Arc<dyn Transport> {
        self.transport.clone()
    }
} 