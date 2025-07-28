//! Message routing implementations

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::api::{Node, Patch, Router, JSON, CHANNEL_FLAG, CHUNKED_FLAG, STREAM_HEADER_FLAG};
use crate::error::{Result, RatNetError};

pub mod default;

pub use default::DefaultRouter;

/// Initialize router registrations
pub fn init() {
    crate::register_router!("default", |config| {
        Arc::new(DefaultRouter::new())
    });
} 