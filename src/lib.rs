//! # RatNet - Rust Implementation
//!
//! A Rust port of RatNet, an anonymity network for mesh routing and embedded scenarios.
//!
//! RatNet provides:
//! - Anonymous peer-to-peer networking
//! - Pluggable transport protocols  
//! - Message routing with store-and-forward capabilities
//! - Cryptographic privacy and authentication
//!
//! ## Architecture
//!
//! The core components are:
//! - **Node**: Main interface for sending/receiving messages
//! - **Transport**: Network layer abstraction (UDP, TLS, HTTPS)
//! - **Router**: Message routing logic
//! - **Policy**: Connection management policies
//!
//! ## Example
//!
//! ```rust,no_run
//! use ratnet::prelude::*;
//! use ratnet::nodes::MemoryNode;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//!     // Initialize component registrations
//!     ratnet::init();
//!     
//!     // Create a new node
//!     let node = Arc::new(MemoryNode::new());
//!     
//!     // Start the node
//!     node.start().await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod api;
pub mod benchmark;
pub mod crypto;
pub mod database;
pub mod error;
pub mod nodes;
pub mod policy;
pub mod registry;
pub mod router;
pub mod transports;

pub mod prelude {
    //! Convenience re-exports for common types and traits

    pub use crate::api::*;
    pub use crate::error::*;
    pub use crate::registry::Registry;
}

use crate::registry::Registry;

/// Global type registry for components
use std::sync::LazyLock;
pub static REGISTRY: LazyLock<Registry> = LazyLock::new(|| Registry::new());

/// Initialize all component registrations
///
/// Call this function once before using the library to register
/// all built-in components (routers, transports, policies).
pub fn init() {
    router::init();
    transports::init();
    policy::init();
}
