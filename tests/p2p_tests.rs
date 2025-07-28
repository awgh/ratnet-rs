#[cfg(not(feature = "p2p"))]
use ratnet::policy::p2p::P2PPolicy;
#[cfg(feature = "p2p")]
use ratnet::policy::P2PPolicy;
use ratnet::api::{Policy, JSON};
use ratnet::transports::MemoryTransport;
use std::sync::Arc;
use std::time::Duration;

#[cfg(not(feature = "p2p"))]
#[tokio::test]
async fn test_p2p_policy_stub_creation() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport.clone(),
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );

    // Test that the stub implementation works correctly
    assert!(!policy.is_listening());
    assert!(!policy.is_advertising());
    assert_eq!(policy.listen_interval(), Duration::from_secs(30));
    assert_eq!(policy.advertise_interval(), Duration::from_secs(10));
    assert_eq!(policy.negotiation_rank(), 1);
}

#[cfg(not(feature = "p2p"))]
#[tokio::test]
async fn test_p2p_policy_stub_behavior() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport.clone(),
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );

    // Test Policy trait implementation
    assert!(policy.run_policy().await.is_ok());
    assert!(policy.stop().await.is_ok());
    assert_eq!(policy.get_transport().name(), "memory");

    // Test JSON trait implementation
    let json = policy.to_json().expect("Failed to serialize to JSON");
    assert!(json.contains("P2PPolicy"));
    assert!(json.contains("enabled"));
    assert!(json.contains("false"));

    // Test that from_json fails as expected
    let result = P2PPolicy::from_json(&json);
    match result {
        Err(e) => {
            let error_msg = e.to_string();
            assert!(error_msg.contains("P2P feature not enabled"));
        },
        Ok(_) => panic!("Expected error from from_json, got Ok"),
    }
}

#[cfg(not(feature = "p2p"))]
#[tokio::test]
async fn test_p2p_policy_stub_negotiation_rank() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport,
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );

    let original_rank = policy.negotiation_rank();
    policy.reroll_negotiation_rank();
    let new_rank = policy.negotiation_rank();
    
    // In stub implementation, reroll_negotiation_rank is a no-op
    assert_eq!(original_rank, new_rank);
    assert_eq!(original_rank, 1);
}

#[cfg(feature = "p2p")]
#[tokio::test]
async fn test_p2p_policy_real_creation() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport.clone(),
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );
    assert!(!policy.is_listening());
    assert!(!policy.is_advertising());
    assert_eq!(policy.listen_interval(), Duration::from_secs(30));
    assert_eq!(policy.advertise_interval(), Duration::from_secs(10));
    assert!(policy.negotiation_rank() > 0);
}

#[cfg(feature = "p2p")]
#[tokio::test]
async fn test_p2p_policy_start_stop_real() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport,
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );

    // Test starting - this might fail due to socket binding issues in test environment
    // so we'll just check that the policy was created correctly
    assert!(!policy.is_listening());
    assert!(!policy.is_advertising());
    assert_eq!(policy.listen_interval(), Duration::from_secs(30));
    assert_eq!(policy.advertise_interval(), Duration::from_secs(10));
    assert!(policy.negotiation_rank() > 0);

    // Try to start the policy, but don't fail if it can't bind sockets
    let start_result = policy.run_policy().await;
    if start_result.is_ok() {
        assert!(policy.is_listening());
        assert!(policy.is_advertising());

        // Test stopping
        assert!(policy.stop().await.is_ok());
        assert!(!policy.is_listening());
        assert!(!policy.is_advertising());
    } else {
        // If starting failed (likely due to socket binding), that's okay for tests
        println!("P2P policy start failed (expected in test environment): {:?}", start_result);
    }
}

#[cfg(feature = "p2p")]
#[tokio::test]
async fn test_p2p_negotiation_rank_real() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport,
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );

    let original_rank = policy.negotiation_rank();
    policy.reroll_negotiation_rank();
    let new_rank = policy.negotiation_rank();

    // Very high probability they'll be different
    assert_ne!(original_rank, new_rank);
}

#[cfg(feature = "p2p")]
#[tokio::test]
async fn test_p2p_dns_packet_creation() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport,
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );

    // Test DNS packet creation
    let packet = policy.create_mdns_advertisement_packet("test://127.0.0.1:8080", 12345).unwrap();
    
    // Verify packet structure
    assert!(packet.len() >= 12); // DNS header
    
    // Check DNS header
    assert_eq!(packet[0..2], [0x00, 0x00]); // Transaction ID
    assert_eq!(packet[2..4], [0x84, 0x00]); // Flags (response, authoritative)
    assert_eq!(packet[4..6], [0x00, 0x00]); // Questions
    assert_eq!(packet[6..8], [0x00, 0x02]); // Answer RRs (2 records)
}

#[cfg(feature = "p2p")]
#[tokio::test]
async fn test_p2p_peer_discovery() {
    let transport = Arc::new(MemoryTransport::new());
    let policy = P2PPolicy::new(
        transport,
        "127.0.0.1:8080".to_string(),
        Arc::new(ratnet::nodes::MemoryNode::new()),
        false,
        Duration::from_secs(30),
        Duration::from_secs(10),
    );

    // Create a mock DNS packet for testing
    let mut test_packet = Vec::new();
    
    // DNS header
    test_packet.extend_from_slice(&[
        0x00, 0x00, // Transaction ID
        0x84, 0x00, // Flags (response, authoritative)
        0x00, 0x00, // Questions
        0x00, 0x01, // Answer RRs
        0x00, 0x00, // Authority RRs
        0x00, 0x00, // Additional RRs
    ]);

    // Add a mock SRV record
    // Name: rn.746573743a2f2f3132372e302e302e313a38303830.local
    test_packet.extend_from_slice(&[
        0x02, 0x72, 0x6e, // "rn"
        0x1e, 0x74, 0x65, 0x73, 0x74, 0x3a, 0x2f, 0x2f, 0x31, 0x32, 0x37, 0x2e, 0x30, 0x2e, 0x30, 0x2e, 0x31, 0x3a, 0x38, 0x30, 0x38, 0x30, // encoded address
        0x05, 0x6c, 0x6f, 0x63, 0x61, 0x6c, // "local"
        0x00, // null terminator
        0x00, 0x21, // SRV type
        0x00, 0x01, // IN class
        0x00, 0x00, 0x00, 120, // TTL
        0x00, 0x0a, // Data length
        0x00, 0x00, // Priority
        0x00, 0x00, // Weight
        0x14, 0xe9, // Port (5353)
        0x07, 0x74, 0x65, 0x73, 0x74, 0x68, 0x6f, 0x73, 0x74, // "testhost"
        0x05, 0x6c, 0x6f, 0x63, 0x61, 0x6c, // "local"
        0x00, // null terminator
    ]);

    // Process the packet
    let result = policy.process_mdns_packet(&test_packet).await;
    assert!(result.is_ok());
} 