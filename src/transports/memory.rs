//! Memory transport for testing purposes

use async_trait::async_trait;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

use crate::api::{Action, Transport};
use crate::error::{Result, RatNetError};

/// Memory transport for testing
pub struct MemoryTransport {
    running: AtomicBool,
    byte_limit: AtomicI64,
}

impl MemoryTransport {
    /// Create a new memory transport
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            byte_limit: AtomicI64::new(1024 * 1024), // 1MB default
        }
    }
}

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MemoryTransport {
    async fn listen(&self, _listen: String, _admin_mode: bool) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn name(&self) -> &str {
        "memory"
    }

    async fn rpc(&self, _host: &str, _method: Action, _args: Vec<Value>) -> Result<Value> {
        // For testing, just return a success response
        Ok(Value::Null)
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn byte_limit(&self) -> i64 {
        self.byte_limit.load(Ordering::SeqCst)
    }

    fn set_byte_limit(&self, limit: i64) {
        self.byte_limit.store(limit, Ordering::SeqCst);
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
} 