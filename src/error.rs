//! Error types for RatNet

use thiserror::Error;

/// Main error type for RatNet operations
#[derive(Error, Debug)]
pub enum RatNetError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Node error: {0}")]
    Node(String),

    #[error("Router error: {0}")]
    Router(String),

    #[error("Policy error: {0}")]
    Policy(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Operation timed out")]
    Timeout,

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Service unavailable: {0}")]
    Unavailable(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Feature not enabled: {0}")]
    Feature(String),
}

/// Result type alias for RatNet operations
pub type Result<T> = std::result::Result<T, RatNetError>;

impl From<serde_json::Error> for RatNetError {
    fn from(err: serde_json::Error) -> Self {
        RatNetError::Serialization(err.to_string())
    }
}

impl From<bincode::Error> for RatNetError {
    fn from(err: bincode::Error) -> Self {
        RatNetError::Serialization(err.to_string())
    }
}

#[cfg(feature = "sqlite")]
impl From<sqlx::Error> for RatNetError {
    fn from(err: sqlx::Error) -> Self {
        RatNetError::Database(err.to_string())
    }
}
