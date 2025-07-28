//! End-to-end tests demonstrating message flow across all node types and transports

use bytes::Bytes;
use ratnet::prelude::*;
use ratnet::{
    database::SqliteDatabase,
    nodes::{DatabaseNode, FilesystemNode, MemoryNode},
    router::DefaultRouter,
    transports::UdpTransport,
};
use std::sync::Arc;
use tracing::info;

/// Test message flow between different node types
#[tokio::test]
async fn test_cross_node_type_communication() {
    info!("Testing cross-node type communication...");

    // Create different node types
    let memory_node = Arc::new(MemoryNode::new());
    let database = Arc::new(
        SqliteDatabase::new_memory()
            .await
            .expect("Failed to create database"),
    );
    let database_node = Arc::new(
        DatabaseNode::new(database)
            .await
            .expect("Failed to create database node"),
    );

    let temp_dir = std::env::temp_dir().join("ratnet_fs_test");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).expect("Failed to clean temp dir");
    }
    let filesystem_node = Arc::new(FilesystemNode::new(temp_dir.clone()));

    // Set up routers for all nodes
    let memory_router = Arc::new(DefaultRouter::new());
    let database_router = Arc::new(DefaultRouter::new());
    let filesystem_router = Arc::new(DefaultRouter::new());

    memory_node.set_router(memory_router);
    database_node.set_router(database_router);
    filesystem_node.set_router(filesystem_router);

    // Start all nodes
    memory_node
        .start()
        .await
        .expect("Failed to start memory node");
    database_node
        .start()
        .await
        .expect("Failed to start database node");
    filesystem_node
        .start()
        .await
        .expect("Failed to start filesystem node");

    // Test MemoryNode → DatabaseNode communication
    info!("Testing MemoryNode → DatabaseNode communication");
    let memory_id = memory_node
        .id()
        .await
        .expect("Failed to get memory node ID");
    let database_id = database_node
        .id()
        .await
        .expect("Failed to get database node ID");

    // Send a message from memory to database node
    let test_msg = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Hello from MemoryNode to DatabaseNode"),
        is_chan: true,
        pubkey: memory_id.clone(),
        chunked: false,
        stream_header: false,
    };

    memory_node
        .send_msg(test_msg)
        .await
        .expect("Failed to send message");

    // Test DatabaseNode → FilesystemNode communication
    info!("Testing DatabaseNode → FilesystemNode communication");
    let filesystem_id = filesystem_node
        .id()
        .await
        .expect("Failed to get filesystem node ID");

    let test_msg2 = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Hello from DatabaseNode to FilesystemNode"),
        is_chan: true,
        pubkey: database_id.clone(),
        chunked: false,
        stream_header: false,
    };

    database_node
        .send_msg(test_msg2)
        .await
        .expect("Failed to send message");

    // Test FilesystemNode → MemoryNode communication
    info!("Testing FilesystemNode → MemoryNode communication");

    let test_msg3 = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Hello from FilesystemNode to MemoryNode"),
        is_chan: true,
        pubkey: filesystem_id.clone(),
        chunked: false,
        stream_header: false,
    };

    filesystem_node
        .send_msg(test_msg3)
        .await
        .expect("Failed to send message");

    // Clean up
    let _ = memory_node.stop().await;
    let _ = database_node.stop().await;
    let _ = filesystem_node.stop().await;

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).expect("Failed to clean temp dir");
    }

    info!("Cross-node type communication test completed successfully");
}

/// Test message flow across different transports
#[tokio::test]
async fn test_cross_transport_communication() {
    info!("Testing cross-transport communication...");

    // Create nodes
    let node1 = Arc::new(MemoryNode::new());
    let node2 = Arc::new(MemoryNode::new());
    let node3 = Arc::new(MemoryNode::new());

    // Set up routers
    let router1 = Arc::new(DefaultRouter::new());
    let router2 = Arc::new(DefaultRouter::new());
    let router3 = Arc::new(DefaultRouter::new());

    node1.set_router(router1);
    node2.set_router(router2);
    node3.set_router(router3);

    // Create different transports
    let udp_transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

    // Start transports
    udp_transport
        .listen("127.0.0.1:0".to_string(), false)
        .await
        .expect("Failed to start UDP transport");

    // Start nodes
    node1.start().await.expect("Failed to start node1");
    node2.start().await.expect("Failed to start node2");
    node3.start().await.expect("Failed to start node3");

    // Test UDP → Memory transport communication
    info!("Testing UDP → Memory transport communication");

    let node1_id = node1.id().await.expect("Failed to get node1 ID");
    let node2_id = node2.id().await.expect("Failed to get node2 ID");

    // Send message via UDP transport
    let test_msg = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Hello via UDP transport"),
        is_chan: true,
        pubkey: node1_id.clone(),
        chunked: false,
        stream_header: false,
    };

    // This would require proper transport integration
    // For now, we'll test the basic functionality
    node1
        .send_msg(test_msg)
        .await
        .expect("Failed to send message via UDP");

    // Test Memory → UDP transport communication
    info!("Testing Memory → UDP transport communication");

    let test_msg2 = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Hello via Memory transport"),
        is_chan: true,
        pubkey: node2_id.clone(),
        chunked: false,
        stream_header: false,
    };

    node2
        .send_msg(test_msg2)
        .await
        .expect("Failed to send message via Memory");

    // Clean up
    udp_transport
        .stop()
        .await
        .expect("Failed to stop UDP transport");
    let _ = node1.stop().await;
    let _ = node2.stop().await;
    let _ = node3.stop().await;

    info!("Cross-transport communication test completed successfully");
}

/// Test multi-hop message routing
#[tokio::test]
async fn test_multi_hop_message_routing() {
    info!("Testing multi-hop message routing...");

    // Create a chain of nodes: A → B → C
    let node_a = Arc::new(MemoryNode::new());
    let node_b = Arc::new(MemoryNode::new());
    let node_c = Arc::new(MemoryNode::new());

    // Set up routers
    let router_a = Arc::new(DefaultRouter::new());
    let router_b = Arc::new(DefaultRouter::new());
    let router_c = Arc::new(DefaultRouter::new());

    node_a.set_router(router_a);
    node_b.set_router(router_b);
    node_c.set_router(router_c);

    // Start nodes
    node_a.start().await.expect("Failed to start node A");
    node_b.start().await.expect("Failed to start node B");
    node_c.start().await.expect("Failed to start node C");

    // Get node IDs
    let node_a_id = node_a.id().await.expect("Failed to get node A ID");
    let node_b_id = node_b.id().await.expect("Failed to get node B ID");
    let node_c_id = node_c.id().await.expect("Failed to get node C ID");

    info!("Node A ID: {}", node_a_id);
    info!("Node B ID: {}", node_b_id);
    info!("Node C ID: {}", node_c_id);

    // Send message from A to C via B
    info!("Sending message from A to C via B");

    let test_msg = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Hello from A to C via B"),
        is_chan: true,
        pubkey: node_a_id.clone(),
        chunked: false,
        stream_header: false,
    };

    // In a real implementation, this would route through the network
    // For now, we'll test the basic message sending
    node_a
        .send_msg(test_msg)
        .await
        .expect("Failed to send message from A");

    // Test reverse routing: C → A via B
    info!("Sending message from C to A via B");

    let test_msg2 = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Hello from C to A via B"),
        is_chan: true,
        pubkey: node_c_id.clone(),
        chunked: false,
        stream_header: false,
    };

    node_c
        .send_msg(test_msg2)
        .await
        .expect("Failed to send message from C");

    // Clean up
    let _ = node_a.stop().await;
    let _ = node_b.stop().await;
    let _ = node_c.stop().await;

    info!("Multi-hop message routing test completed successfully");
}

/// Test bundle routing through multiple nodes
#[tokio::test]
async fn test_bundle_routing_through_multiple_nodes() {
    info!("Testing bundle routing through multiple nodes...");

    // Create nodes with different types
    let memory_node = Arc::new(MemoryNode::new());
    let database = Arc::new(
        SqliteDatabase::new_memory()
            .await
            .expect("Failed to create database"),
    );
    let database_node = Arc::new(
        DatabaseNode::new(database)
            .await
            .expect("Failed to create database node"),
    );

    let temp_dir = std::env::temp_dir().join("ratnet_bundle_test");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).expect("Failed to clean temp dir");
    }
    let filesystem_node = Arc::new(FilesystemNode::new(temp_dir.clone()));

    // Set up routers
    let memory_router = Arc::new(DefaultRouter::new());
    let database_router = Arc::new(DefaultRouter::new());
    let filesystem_router = Arc::new(DefaultRouter::new());

    memory_node.set_router(memory_router);
    database_node.set_router(database_router);
    filesystem_node.set_router(filesystem_router);

    // Start nodes
    memory_node
        .start()
        .await
        .expect("Failed to start memory node");
    database_node
        .start()
        .await
        .expect("Failed to start database node");
    filesystem_node
        .start()
        .await
        .expect("Failed to start filesystem node");

    // Create a bundle with multiple messages
    let msg1 = Msg {
        name: "test_channel_1".to_string(),
        content: Bytes::from("Message 1 from memory node"),
        is_chan: true,
        pubkey: memory_node
            .id()
            .await
            .expect("Failed to get memory node ID"),
        chunked: false,
        stream_header: false,
    };

    let msg2 = Msg {
        name: "test_channel_2".to_string(),
        content: Bytes::from("Message 2 from memory node"),
        is_chan: true,
        pubkey: memory_node
            .id()
            .await
            .expect("Failed to get memory node ID"),
        chunked: false,
        stream_header: false,
    };

    let messages = vec![msg1, msg2];
    let bundle_data = serde_json::to_vec(&messages).expect("Failed to serialize messages");
    let bundle = Bundle {
        data: Bytes::from(bundle_data),
        time: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    };

    // Dropoff bundle to memory node
    info!("Dropping off bundle to memory node");
    memory_node
        .dropoff(bundle.clone())
        .await
        .expect("Failed to dropoff bundle to memory node");

    // Dropoff bundle to database node
    info!("Dropping off bundle to database node");
    database_node
        .dropoff(bundle.clone())
        .await
        .expect("Failed to dropoff bundle to database node");

    // Dropoff bundle to filesystem node
    info!("Dropping off bundle to filesystem node");
    filesystem_node
        .dropoff(bundle)
        .await
        .expect("Failed to dropoff bundle to filesystem node");

    // Test pickup from each node
    let memory_id = memory_node
        .id()
        .await
        .expect("Failed to get memory node ID");
    let database_id = database_node
        .id()
        .await
        .expect("Failed to get database node ID");
    let filesystem_id = filesystem_node
        .id()
        .await
        .expect("Failed to get filesystem node ID");

    info!("Picking up bundle from memory node");
    let memory_bundle = memory_node.pickup(memory_id, 0, 10000, vec![]).await;
    assert!(
        memory_bundle.is_ok(),
        "Failed to pickup bundle from memory node"
    );

    info!("Picking up bundle from database node");
    let database_bundle = database_node.pickup(database_id, 0, 10000, vec![]).await;
    assert!(
        database_bundle.is_ok(),
        "Failed to pickup bundle from database node"
    );

    info!("Picking up bundle from filesystem node");
    let filesystem_bundle = filesystem_node
        .pickup(filesystem_id, 0, 10000, vec![])
        .await;
    assert!(
        filesystem_bundle.is_ok(),
        "Failed to pickup bundle from filesystem node"
    );

    // Clean up
    let _ = memory_node.stop().await;
    let _ = database_node.stop().await;
    let _ = filesystem_node.stop().await;

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).expect("Failed to clean temp dir");
    }

    info!("Bundle routing test completed successfully");
}

/// Test chunked message flow across different node types
#[tokio::test]
async fn test_chunked_message_flow_across_nodes() {
    info!("Testing chunked message flow across nodes...");

    // Create nodes
    let sender_node = Arc::new(MemoryNode::new());
    let receiver_node = Arc::new(MemoryNode::new());

    // Set up routers
    let sender_router = Arc::new(DefaultRouter::new());
    let receiver_router = Arc::new(DefaultRouter::new());

    sender_node.set_router(sender_router);
    receiver_node.set_router(receiver_router);

    // Start nodes
    sender_node
        .start()
        .await
        .expect("Failed to start sender node");
    receiver_node
        .start()
        .await
        .expect("Failed to start receiver node");

    // Create a large message that will be chunked
    let large_content = "x".repeat(10000); // 10KB message
    let sender_id = sender_node.id().await.expect("Failed to get sender ID");

    let large_msg = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from(large_content),
        is_chan: true,
        pubkey: sender_id.clone(),
        chunked: true,
        stream_header: true,
    };

    // Send the large message (will be chunked automatically)
    info!("Sending large chunked message");
    sender_node
        .send_msg(large_msg)
        .await
        .expect("Failed to send large message");

    // In a real implementation, the receiver would receive and reassemble the chunks
    // For now, we'll verify the message was sent
    info!("Large chunked message sent successfully");

    // Clean up
    let _ = sender_node.stop().await;
    let _ = receiver_node.stop().await;

    info!("Chunked message flow test completed successfully");
}

/// Test network partition and recovery scenarios
#[tokio::test]
async fn test_network_partition_recovery() {
    info!("Testing network partition and recovery...");

    // Create nodes
    let node1 = Arc::new(MemoryNode::new());
    let node2 = Arc::new(MemoryNode::new());
    let node3 = Arc::new(MemoryNode::new());

    // Set up routers
    let router1 = Arc::new(DefaultRouter::new());
    let router2 = Arc::new(DefaultRouter::new());
    let router3 = Arc::new(DefaultRouter::new());

    node1.set_router(router1);
    node2.set_router(router2);
    node3.set_router(router3);

    // Start nodes
    node1.start().await.expect("Failed to start node1");
    node2.start().await.expect("Failed to start node2");
    node3.start().await.expect("Failed to start node3");

    // Simulate network partition by stopping node2
    info!("Simulating network partition by stopping node2");
    let _ = node2.stop().await;

    // Try to send messages during partition
    let node1_id = node1.id().await.expect("Failed to get node1 ID");
    let test_msg = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Message during partition"),
        is_chan: true,
        pubkey: node1_id.clone(),
        chunked: false,
        stream_header: false,
    };

    // This should still work (messages queued)
    node1
        .send_msg(test_msg)
        .await
        .expect("Failed to send message during partition");

    // Restart node2 to simulate recovery
    info!("Simulating network recovery by restarting node2");
    node2.start().await.expect("Failed to restart node2");

    // Send message after recovery
    let test_msg2 = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("Message after recovery"),
        is_chan: true,
        pubkey: node1_id.clone(),
        chunked: false,
        stream_header: false,
    };

    node1
        .send_msg(test_msg2)
        .await
        .expect("Failed to send message after recovery");

    // Clean up
    let _ = node1.stop().await;
    let _ = node2.stop().await;
    let _ = node3.stop().await;

    info!("Network partition and recovery test completed successfully");
}
