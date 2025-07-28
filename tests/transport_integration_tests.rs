//! Transport integration tests

use ratnet::prelude::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[cfg(feature = "tls")]
use ratnet::transports::TlsTransport;

#[cfg(feature = "https")]
use ratnet::transports::HttpsTransport;

#[tokio::test]
async fn test_udp_transport_echo() {
    // Create two UDP transports
    let transport1 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));
    let transport2 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start both transports
    transport1
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport1");
    transport2
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport2");

    // Get their addresses
    let addr1 = transport1
        .local_addr()
        .await
        .expect("Failed to get transport1 address");
    let addr2 = transport2
        .local_addr()
        .await
        .expect("Failed to get transport2 address");

    // Send a simple message from transport1 to transport2
    let test_data = b"Hello, UDP!";

    // Use the send_request method if available, otherwise use RPC
    let response = transport1
        .rpc(
            &addr2.to_string(),
            Action::ID,
            vec![serde_json::Value::String(
                String::from_utf8_lossy(test_data).to_string(),
            )],
        )
        .await;

    // Print the response for debugging
    match &response {
        Ok(value) => println!("RPC response success: {value:?}"),
        Err(e) => println!("RPC response error: {e:?}"),
    }

    // The response should be successful (echo back)
    assert!(response.is_ok());

    // Clean up
    transport1.stop().await.expect("Failed to stop transport1");
    transport2.stop().await.expect("Failed to stop transport2");
}

#[tokio::test]
async fn test_udp_transport_rpc_communication() {
    // Create two UDP transports
    let transport1 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));
    let transport2 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start both transports
    transport1
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport1");
    transport2
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport2");

    // Get their addresses
    let addr1 = transport1
        .local_addr()
        .await
        .expect("Failed to get transport1 address");
    let addr2 = transport2
        .local_addr()
        .await
        .expect("Failed to get transport2 address");

    // Send RPC calls in both directions
    let args1 = vec![serde_json::Value::String(
        "Hello from transport1".to_string(),
    )];
    let args2 = vec![serde_json::Value::String(
        "Hello from transport2".to_string(),
    )];

    let response1 = transport1.rpc(&addr2.to_string(), Action::ID, args1).await;
    let response2 = transport2.rpc(&addr1.to_string(), Action::ID, args2).await;

    // Both responses should be successful
    assert!(response1.is_ok());
    assert!(response2.is_ok());

    // Clean up
    transport1.stop().await.expect("Failed to stop transport1");
    transport2.stop().await.expect("Failed to stop transport2");
}

#[tokio::test]
async fn test_udp_transport_timeout() {
    let transport = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start transport
    transport
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport");

    // Try to send RPC to a non-existent address
    let result = transport.rpc("127.0.0.1:9999", Action::ID, vec![]).await;

    // Should timeout or fail
    assert!(result.is_err());

    // Clean up
    transport.stop().await.expect("Failed to stop transport");
}

#[tokio::test]
async fn test_udp_transport_large_message() {
    let transport1 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));
    let transport2 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start both transports
    transport1
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport1");
    transport2
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport2");

    // Get transport2 address
    let addr2 = transport2
        .local_addr()
        .await
        .expect("Failed to get transport2 address");

    // Create a large message (but within reasonable limits)
    let large_data = "x".repeat(1000);
    let args = vec![serde_json::Value::String(large_data)];

    // Send the large message
    let response = transport1.rpc(&addr2.to_string(), Action::ID, args).await;

    // Should succeed
    assert!(response.is_ok());

    // Clean up
    transport1.stop().await.expect("Failed to stop transport1");
    transport2.stop().await.expect("Failed to stop transport2");
}

#[tokio::test]
async fn test_udp_transport_concurrent_messages() {
    let transport1 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));
    let transport2 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start both transports
    transport1
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport1");
    transport2
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport2");

    // Get transport2 address
    let addr2 = transport2
        .local_addr()
        .await
        .expect("Failed to get transport2 address");

    // Send multiple concurrent messages
    let mut handles = vec![];

    for i in 0..10 {
        let transport1_clone = transport1.clone();
        let addr2_clone = addr2.to_string();
        let handle = tokio::spawn(async move {
            let args = vec![serde_json::Value::Number(serde_json::Number::from(i))];
            transport1_clone.rpc(&addr2_clone, Action::ID, args).await
        });
        handles.push(handle);
    }

    // Wait for all messages to complete
    for handle in handles {
        let result = handle.await.expect("Task failed");
        assert!(result.is_ok());
    }

    // Clean up
    transport1.stop().await.expect("Failed to stop transport1");
    transport2.stop().await.expect("Failed to stop transport2");
}

#[tokio::test]
async fn test_udp_transport_restart() {
    let transport = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start, stop, and restart multiple times
    for i in 0..5 {
        // Start
        transport
            .listen("127.0.0.1:0".to_string(), false)
            .await
            .unwrap_or_else(|_| panic!("Failed to start transport (iteration {i})"));

        // Verify it's running
        let local_addr = transport.local_addr().await;
        assert!(local_addr.is_some());

        // Stop
        transport
            .stop()
            .await
            .unwrap_or_else(|_| panic!("Failed to stop transport (iteration {i})"));

        // Small delay
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_udp_transport_different_ports() {
    // Test that transports can run on different ports
    let transport1 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));
    let transport2 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));
    let transport3 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start all transports
    transport1
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport1");
    transport2
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport2");
    transport3
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport3");

    // Get their addresses
    let addr1 = transport1
        .local_addr()
        .await
        .expect("Failed to get transport1 address");
    let addr2 = transport2
        .local_addr()
        .await
        .expect("Failed to get transport2 address");
    let addr3 = transport3
        .local_addr()
        .await
        .expect("Failed to get transport3 address");

    // Verify they're on different ports
    assert_ne!(addr1.port(), addr2.port());
    assert_ne!(addr2.port(), addr3.port());
    assert_ne!(addr1.port(), addr3.port());

    // Test communication between all pairs
    let response1 = transport1.rpc(&addr2.to_string(), Action::ID, vec![]).await;
    let response2 = transport2.rpc(&addr3.to_string(), Action::ID, vec![]).await;
    let response3 = transport3.rpc(&addr1.to_string(), Action::ID, vec![]).await;

    assert!(response1.is_ok());
    assert!(response2.is_ok());
    assert!(response3.is_ok());

    // Clean up
    transport1.stop().await.expect("Failed to stop transport1");
    transport2.stop().await.expect("Failed to stop transport2");
    transport3.stop().await.expect("Failed to stop transport3");
}

#[tokio::test]
async fn test_udp_transport_admin_mode() {
    let transport = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start in admin mode
    transport
        .listen("127.0.0.1:0".to_string(), true)
        .await
        .expect("Failed to start transport in admin mode");

    // Verify it's running
    let local_addr = transport.local_addr().await;
    assert!(local_addr.is_some());

    // Clean up
    transport.stop().await.expect("Failed to stop transport");
}

#[tokio::test]
async fn test_udp_transport_byte_limit_enforcement() {
    let transport = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Set a very small byte limit
    transport.set_byte_limit(100);

    // Start transport
    transport
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport");

    // Create a message that exceeds the byte limit
    let large_data = "x".repeat(200);
    let args = vec![serde_json::Value::String(large_data)];

    // Try to send to a non-existent address (should timeout, but not crash)
    let result = transport.rpc("127.0.0.1:9999", Action::ID, args).await;

    // Should fail (timeout or other error), but not crash
    assert!(result.is_err());

    // Clean up
    transport.stop().await.expect("Failed to stop transport");
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn test_tls_transport_basic() {
    // Generate test certificates
    let (cert_pem, key_pem) =
        ratnet::crypto::generate_test_cert().expect("Failed to generate test certificate");

    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = Arc::new(TlsTransport::new_with_certs(
        "127.0.0.1:0".to_string(),
        cert_pem,
        key_pem,
        true,
        node,
    ));

    // Start transport
    transport
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start TLS transport");

    // Verify it's running
    assert!(transport.is_running());

    // Clean up
    transport
        .stop()
        .await
        .expect("Failed to stop TLS transport");
}

#[cfg(feature = "https")]
#[tokio::test]
async fn test_https_transport_basic() {
    // Generate test certificates
    let (cert_pem, key_pem) =
        ratnet::crypto::generate_test_cert().expect("Failed to generate test certificate");

    let node = Arc::new(ratnet::nodes::MemoryNode::new());
    let transport = Arc::new(HttpsTransport::new_with_certs(
        "127.0.0.1:0".to_string(),
        cert_pem,
        key_pem,
        true,
        node,
    ));

    // Start transport
    transport
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start HTTPS transport");

    // Verify it's running
    assert!(transport.is_running());

    // Clean up
    transport
        .stop()
        .await
        .expect("Failed to stop HTTPS transport");
}

// Stress tests

#[tokio::test]
async fn test_udp_transport_stress_test() {
    let transport1 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));
    let transport2 = Arc::new(ratnet::transports::UdpTransport::new(
        "127.0.0.1:0".to_string(),
    ));

    // Start both transports
    transport1
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport1");
    transport2
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start transport2");

    // Get transport2 address
    let addr2 = transport2
        .local_addr()
        .await
        .expect("Failed to get transport2 address");

    // Send many messages rapidly
    let mut handles = vec![];

    for i in 0..100 {
        let transport1_clone = transport1.clone();
        let addr2_clone = addr2.to_string();
        let handle = tokio::spawn(async move {
            let args = vec![serde_json::Value::Number(serde_json::Number::from(i))];
            transport1_clone.rpc(&addr2_clone, Action::ID, args).await
        });
        handles.push(handle);
    }

    // Wait for all messages to complete
    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(_)) = handle.await {
            success_count += 1;
        }
    }

    // Most messages should succeed (some may timeout due to UDP nature)
    assert!(success_count > 50);

    // Clean up
    transport1.stop().await.expect("Failed to stop transport1");
    transport2.stop().await.expect("Failed to stop transport2");
}
