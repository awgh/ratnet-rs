//! Message routing implementations

use std::sync::Arc;

pub mod default;

pub use default::DefaultRouter;

/// Initialize router registrations
pub fn init() {
    crate::register_router!("default", |_config| { Arc::new(DefaultRouter::new()) });
}
