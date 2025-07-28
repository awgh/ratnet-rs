//! Transport layer implementations

use std::sync::Arc;

pub mod memory;
pub mod udp;

#[cfg(feature = "tls")]
pub mod tls;

#[cfg(feature = "https")]
pub mod https;

pub use memory::MemoryTransport;
pub use udp::UdpTransport;

#[cfg(feature = "tls")]
pub use tls::TlsTransport;

#[cfg(feature = "https")]
pub use https::HttpsTransport;

/// Initialize transport registrations
pub fn init() {
    crate::register_transport!("udp", |_node, config| {
        let listen_addr = config
            .get("listen")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:0");

        Arc::new(UdpTransport::new(listen_addr.to_string()))
    });

    #[cfg(feature = "tls")]
    crate::register_transport!("tls", |node, config| {
        let listen_addr = config
            .get("listen")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:8443");

        Arc::new(TlsTransport::new(listen_addr.to_string(), node))
    });

    #[cfg(feature = "https")]
    crate::register_transport!("https", |node, config| {
        let listen_addr = config
            .get("listen")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:8080");

        Arc::new(HttpsTransport::new(listen_addr.to_string(), node))
    });
}
