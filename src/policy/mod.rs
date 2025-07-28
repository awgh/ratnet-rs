//! Policy implementations for connection management

use std::sync::Arc;

pub mod common;
pub mod p2p;
pub mod poll;
pub mod server;

pub use common::PeerTable;
pub use poll::PollPolicy;
pub use server::ServerPolicy;

// Always export P2PPolicy - it will be the stub implementation when p2p feature is disabled
pub use p2p::P2PPolicy;

/// Initialize policy registrations
pub fn init() {
    crate::register_policy!("poll", |transport, node, _config| {
        Arc::new(PollPolicy::new(transport, node))
    });

    crate::register_policy!("server", |transport, node, config| {
        let listen_uri = config
            .get("listen_uri")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:8080")
            .to_string();
        let admin_mode = config
            .get("admin_mode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Arc::new(ServerPolicy::new(transport, listen_uri, admin_mode))
    });

    #[cfg(feature = "p2p")]
    crate::register_policy!("p2p", |transport, node, config| {
        let listen_uri = config
            .get("listen_uri")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:8080")
            .to_string();
        let admin_mode = config
            .get("admin_mode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let listen_interval = std::time::Duration::from_millis(
            config
                .get("listen_interval")
                .and_then(|v| v.as_u64())
                .unwrap_or(30000),
        );
        let advertise_interval = std::time::Duration::from_millis(
            config
                .get("advertise_interval")
                .and_then(|v| v.as_u64())
                .unwrap_or(10000),
        );

        Arc::new(p2p::P2PPolicy::new(
            transport,
            listen_uri,
            node,
            admin_mode,
            listen_interval,
            advertise_interval,
        ))
    });
}
