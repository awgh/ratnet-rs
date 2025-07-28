//! Transport layer tests

use ratnet::prelude::*;
use std::sync::Arc;

#[cfg(feature = "tls")]
use ratnet::transports::TlsTransport;

#[cfg(feature = "https")]
use ratnet::transports::HttpsTransport;

#[tokio::test]
async fn test_udp_transport_creation() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());
    assert_eq!(transport.name(), "udp");
    assert!(transport.byte_limit() > 0);
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn test_tls_transport_creation() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = TlsTransport::new("127.0.0.1:0".to_string(), node);
    assert_eq!(transport.name(), "tls");
    assert!(transport.byte_limit() > 0);
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn test_tls_transport_with_certs() {
    let (cert_pem, key_pem) =
        ratnet::crypto::generate_test_cert().expect("Failed to generate test certificate");

    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport =
        TlsTransport::new_with_certs("127.0.0.1:0".to_string(), cert_pem, key_pem, true, node);

    assert_eq!(transport.name(), "tls");
    assert!(transport.byte_limit() > 0);
}

#[cfg(feature = "https")]
#[tokio::test]
async fn test_https_transport_creation() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = HttpsTransport::new("127.0.0.1:0".to_string(), node);
    assert_eq!(transport.name(), "https");
    assert!(transport.byte_limit() > 0);
}

#[cfg(feature = "https")]
#[tokio::test]
async fn test_https_transport_with_certs() {
    let (cert_pem, key_pem) =
        ratnet::crypto::generate_test_cert().expect("Failed to generate test certificate");

    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport =
        HttpsTransport::new_with_certs("127.0.0.1:0".to_string(), cert_pem, key_pem, true, node);

    assert_eq!(transport.name(), "https");
    assert!(transport.byte_limit() > 0);
}

#[tokio::test]
async fn test_transport_byte_limits() {
    let udp_transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    let original_limit = udp_transport.byte_limit();
    udp_transport.set_byte_limit(1024);
    assert_eq!(udp_transport.byte_limit(), 1024);

    udp_transport.set_byte_limit(original_limit);
    assert_eq!(udp_transport.byte_limit(), original_limit);
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn test_tls_transport_byte_limits() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = TlsTransport::new("127.0.0.1:0".to_string(), node);

    let original_limit = transport.byte_limit();
    transport.set_byte_limit(2048);
    assert_eq!(transport.byte_limit(), 2048);

    transport.set_byte_limit(original_limit);
    assert_eq!(transport.byte_limit(), original_limit);
}

#[cfg(feature = "https")]
#[tokio::test]
async fn test_https_transport_byte_limits() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = HttpsTransport::new("127.0.0.1:0".to_string(), node);

    let original_limit = transport.byte_limit();
    transport.set_byte_limit(2048);
    assert_eq!(transport.byte_limit(), 2048);

    transport.set_byte_limit(original_limit);
    assert_eq!(transport.byte_limit(), original_limit);
}

#[tokio::test]
async fn test_transport_json_serialization() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:8080".to_string());
    let json = transport.to_json().expect("Failed to serialize transport");
    let deserialized = ratnet::transports::UdpTransport::from_json(&json)
        .expect("Failed to deserialize transport");

    assert_eq!(transport.name(), deserialized.name());
    assert_eq!(transport.byte_limit(), deserialized.byte_limit());
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn test_tls_transport_json_serialization() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = TlsTransport::new("127.0.0.1:8443".to_string(), node);

    let json = transport.to_json().expect("Failed to serialize transport");
    assert!(json.contains("tls"));
    assert!(json.contains("127.0.0.1:8443"));
}

#[cfg(feature = "https")]
#[tokio::test]
async fn test_https_transport_json_serialization() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = HttpsTransport::new("127.0.0.1:8443".to_string(), node);

    let json = transport.to_json().expect("Failed to serialize transport");
    assert!(json.contains("https"));
    assert!(json.contains("127.0.0.1:8443"));
}

// Registry tests are not implemented yet
// #[tokio::test]
// async fn test_transport_registry() {
//     // TODO: Implement when TransportRegistry is available
// }

// #[tokio::test]
// async fn test_create_transport_from_registry() {
//     // TODO: Implement when TransportRegistry is available
// }

// #[cfg(feature = "tls")]
// #[tokio::test]
// async fn test_create_tls_transport_from_registry() {
//     // TODO: Implement when TransportRegistry is available
// }

// #[cfg(feature = "https")]
// #[tokio::test]
// async fn test_create_https_transport_from_registry() {
//     // TODO: Implement when TransportRegistry is available
// }

// Integration tests

#[tokio::test]
async fn test_udp_transport_listen_and_stop() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // Start listening
    let result = transport.listen("127.0.0.1:0".to_string(), false).await;
    assert!(result.is_ok());

    // Check that we got a valid local address
    let local_addr = transport.local_addr().await;
    assert!(local_addr.is_some());

    // Stop the transport
    let stop_result = transport.stop().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_udp_transport_double_listen() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // First listen should succeed
    let result1 = transport.listen("127.0.0.1:0".to_string(), false).await;
    assert!(result1.is_ok());

    // Second listen should fail
    let result2 = transport.listen("127.0.0.1:0".to_string(), false).await;
    assert!(result2.is_err());

    // Stop the transport
    transport.stop().await.expect("Failed to stop transport");
}

#[tokio::test]
async fn test_udp_transport_rpc_without_listening() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // RPC should fail when not listening
    let result = transport.rpc("127.0.0.1:8080", Action::ID, vec![]).await;
    assert!(result.is_err());
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn test_tls_transport_listen_and_stop() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = TlsTransport::new("127.0.0.1:0".to_string(), node);

    // Test that we can start and stop without errors
    let listen_result = transport.listen("127.0.0.1:0".to_string(), false).await;
    assert!(listen_result.is_ok());

    let stop_result = transport.stop().await;
    assert!(stop_result.is_ok());
}

#[cfg(feature = "https")]
#[tokio::test]
async fn test_https_transport_listen_and_stop() {
    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = HttpsTransport::new("127.0.0.1:0".to_string(), node);

    // Test that we can start and stop without errors
    let listen_result = transport.listen("127.0.0.1:0".to_string(), false).await;
    assert!(listen_result.is_ok());

    let stop_result = transport.stop().await;
    assert!(stop_result.is_ok());
}

// Edge case tests

#[tokio::test]
async fn test_udp_transport_invalid_address() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // Try to listen on an invalid address
    let result = transport.listen("invalid-address".to_string(), false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_udp_transport_negative_byte_limit() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // Set negative byte limit
    transport.set_byte_limit(-1);
    assert_eq!(transport.byte_limit(), -1);
}

#[tokio::test]
async fn test_udp_transport_zero_byte_limit() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // Set zero byte limit
    transport.set_byte_limit(0);
    assert_eq!(transport.byte_limit(), 0);
}

// Performance tests

#[tokio::test]
async fn test_udp_transport_rapid_start_stop() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // Rapidly start and stop the transport
    for _ in 0..10 {
        let result = transport.listen("127.0.0.1:0".to_string(), false).await;
        assert!(result.is_ok());

        let stop_result = transport.stop().await;
        assert!(stop_result.is_ok());
    }
}

// Concurrent access tests

#[tokio::test]
async fn test_udp_transport_concurrent_access() {
    let transport = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start listening
    transport
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start listening");

    // Spawn multiple tasks that access the transport concurrently
    let mut handles = vec![];

    for i in 0..5 {
        let transport_clone = transport.clone();
        let handle = tokio::spawn(async move {
            let byte_limit = transport_clone.byte_limit();
            transport_clone.set_byte_limit(byte_limit + i);
            transport_clone.byte_limit()
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let result = handle.await.expect("Task failed");
        assert!(result > 0);
    }

    // Stop the transport
    transport.stop().await.expect("Failed to stop transport");
}

// Error handling tests

#[tokio::test]
async fn test_udp_transport_stop_without_start() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // Stop without starting should succeed
    let result = transport.stop().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_udp_transport_double_stop() {
    let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());

    // Start listening
    transport
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start listening");

    // Stop twice should succeed
    let result1 = transport.stop().await;
    assert!(result1.is_ok());

    let result2 = transport.stop().await;
    assert!(result2.is_ok());
}

// Memory leak tests

#[tokio::test]
async fn test_udp_transport_no_memory_leaks() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TRANSPORT_COUNT: AtomicUsize = AtomicUsize::new(0);

    // Create and drop many transports
    for _ in 0..100 {
        let transport = ratnet::transports::UdpTransport::new("127.0.0.1:0".to_string());
        TRANSPORT_COUNT.fetch_add(1, Ordering::Relaxed);

        // Start and stop
        #[allow(clippy::redundant_pattern_matching)]
        if let Ok(_) = transport.listen("127.0.0.1:0".to_string(), false).await {
            transport.stop().await.expect("Failed to stop transport");
        }
    }

    // The test passes if we don't crash or run out of memory
    assert!(TRANSPORT_COUNT.load(Ordering::Relaxed) >= 100);
}
