//! Server policy implementation
//!
//! The Server policy is a simple listen-only connection policy that starts
//! a transport in server mode and keeps it running until stopped.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::api::{Policy, Transport, JSON};
use crate::error::Result;

/// Server policy - a simple listen-only connection policy
pub struct ServerPolicy {
    transport: Arc<dyn Transport>,
    listen_uri: String,
    admin_mode: bool,
    running: AtomicBool,
    transport_mutex: Mutex<()>,
}

impl ServerPolicy {
    /// Create a new server policy
    pub fn new(transport: Arc<dyn Transport>, listen_uri: String, admin_mode: bool) -> Self {
        Self {
            transport,
            listen_uri,
            admin_mode,
            running: AtomicBool::new(false),
            transport_mutex: Mutex::new(()),
        }
    }

    /// Get the listen URI
    pub fn listen_uri(&self) -> &str {
        &self.listen_uri
    }

    /// Get admin mode setting
    pub fn admin_mode(&self) -> bool {
        self.admin_mode
    }

    /// Check if the policy is currently running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl Policy for ServerPolicy {
    async fn run_policy(&self) -> Result<()> {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return Ok(()); // Already running
        }

        info!("Starting server policy on {}", self.listen_uri);

        // Start listening on the transport
        let transport = self.transport.clone();
        let listen_uri = self.listen_uri.clone();
        let admin_mode = self.admin_mode;

        tokio::spawn(async move {
            if let Err(e) = transport.listen(listen_uri, admin_mode).await {
                tracing::error!("Server policy listen error: {}", e);
            }
        });

        debug!("Server policy started successfully");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        if self
            .running
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return Ok(()); // Already stopped
        }

        info!("Stopping server policy");

        // Stop the transport
        let _lock = self.transport_mutex.lock().await;
        self.transport.stop().await?;

        debug!("Server policy stopped successfully");
        Ok(())
    }

    fn get_transport(&self) -> Arc<dyn Transport> {
        self.transport.clone()
    }
}

impl JSON for ServerPolicy {
    fn to_json(&self) -> Result<String> {
        let config = ServerPolicyConfig {
            policy: "server".to_string(),
            listen_uri: self.listen_uri.clone(),
            admin_mode: self.admin_mode,
            transport: "transport".to_string(), // Simplified for now
        };
        Ok(serde_json::to_string(&config)?)
    }

    fn from_json(_s: &str) -> Result<Self> {
        // Note: This would require transport deserialization which is complex
        // For now, use the constructor with appropriate transport
        Err(crate::error::RatNetError::NotImplemented(
            "ServerPolicy deserialization not yet implemented".to_string(),
        ))
    }
}

/// Configuration structure for JSON serialization
#[derive(Debug, Serialize, Deserialize)]
struct ServerPolicyConfig {
    policy: String,
    listen_uri: String,
    admin_mode: bool,
    transport: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transports::UdpTransport;

    #[tokio::test]
    async fn test_server_policy_creation() {
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));
        let policy = ServerPolicy::new(transport.clone(), "127.0.0.1:8080".to_string(), false);

        assert_eq!(policy.listen_uri(), "127.0.0.1:8080");
        assert!(!policy.admin_mode());
        assert!(!policy.is_running());
        assert_eq!(policy.get_transport().name(), transport.name());
    }

    #[tokio::test]
    async fn test_server_policy_start_stop() {
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));
        let policy = ServerPolicy::new(transport, "127.0.0.1:8080".to_string(), false);

        // Test starting
        assert!(policy.run_policy().await.is_ok());
        assert!(policy.is_running());

        // Test double start (should be safe)
        assert!(policy.run_policy().await.is_ok());
        assert!(policy.is_running());

        // Test stopping
        assert!(policy.stop().await.is_ok());
        assert!(!policy.is_running());

        // Test double stop (should be safe)
        assert!(policy.stop().await.is_ok());
        assert!(!policy.is_running());
    }

    #[tokio::test]
    async fn test_server_policy_json() {
        let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));
        let policy = ServerPolicy::new(transport, "127.0.0.1:8080".to_string(), true);

        let json = policy.to_json().unwrap();
        assert!(json.contains("server"));
        assert!(json.contains("127.0.0.1:8080"));
        assert!(json.contains("true")); // admin_mode
    }
}
