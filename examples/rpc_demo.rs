use ratnet::api::{Action, Bundle, Node, PubKey, RemoteCall, RemoteResponse, Transport};
use ratnet::nodes::MemoryNode;
use ratnet::transports::UdpTransport;
use std::sync::Arc;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting RPC Demo...");

    // Create a memory node
    let node = Arc::new(MemoryNode::new());
    let transport = Arc::new(UdpTransport::new("127.0.0.1:0".to_string()));

    // Start the node
    node.start().await?;
    info!("Node started successfully");

    // Demo 1: Basic Public RPC (ID)
    info!("\n=== Demo 1: Public RPC - ID ===");
    demo_public_rpc_id(&node, &transport).await?;

    // Demo 2: Admin RPC - Contact Management
    info!("\n=== Demo 2: Admin RPC - Contact Management ===");
    demo_contact_management(&node, &transport).await?;

    // Demo 3: Admin RPC - Channel Management
    info!("\n=== Demo 3: Admin RPC - Channel Management ===");
    demo_channel_management(&node, &transport).await?;

    // Demo 4: Admin RPC - Peer Management
    info!("\n=== Demo 4: Admin RPC - Peer Management ===");
    demo_peer_management(&node, &transport).await?;

    // Demo 5: Admin RPC - Profile Management
    info!("\n=== Demo 5: Admin RPC - Profile Management ===");
    demo_profile_management(&node, &transport).await?;

    // Demo 6: Admin RPC - Message Sending
    info!("\n=== Demo 6: Admin RPC - Message Sending ===");
    demo_message_sending(&node, &transport).await?;

    // Demo 7: Public RPC - Pickup/Dropoff
    info!("\n=== Demo 7: Public RPC - Pickup/Dropoff ===");
    demo_pickup_dropoff(&node, &transport).await?;

    // Demo 8: Error Handling
    info!("\n=== Demo 8: Error Handling ===");
    demo_error_handling(&node, &transport).await?;

    // Stop the node
    node.stop().await;
    info!("Node stopped successfully");

    info!("RPC Demo completed successfully!");
    Ok(())
}

async fn demo_public_rpc_id(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    let call = RemoteCall {
        action: Action::ID,
        args: vec![],
    };

    let response = node.public_rpc(transport.clone(), call).await?;

    if response.is_err() {
        warn!("ID RPC failed: {:?}", response.error);
    } else {
        let id = response.value.unwrap();
        info!("Node ID: {}", id);
    }

    Ok(())
}

async fn demo_contact_management(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add a contact
    let add_call = RemoteCall {
        action: Action::AddContact,
        args: vec![
            serde_json::json!("alice"),
            serde_json::json!("ed25519:alice_key_123"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), add_call).await?;
    if response.is_err() {
        warn!("Failed to add contact: {:?}", response.error);
    } else {
        info!("Contact 'alice' added successfully");
    }

    // Add another contact
    let add_call2 = RemoteCall {
        action: Action::AddContact,
        args: vec![
            serde_json::json!("bob"),
            serde_json::json!("ed25519:bob_key_456"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), add_call2).await?;
    if response.is_err() {
        warn!("Failed to add contact: {:?}", response.error);
    } else {
        info!("Contact 'bob' added successfully");
    }

    // Get all contacts
    let get_all_call = RemoteCall {
        action: Action::GetContacts,
        args: vec![],
    };

    let response = node.admin_rpc(transport.clone(), get_all_call).await?;
    if response.is_err() {
        warn!("Failed to get contacts: {:?}", response.error);
    } else {
        let contacts = response.value.unwrap();
        info!("All contacts: {}", contacts);
    }

    // Get specific contact
    let get_call = RemoteCall {
        action: Action::GetContact,
        args: vec![serde_json::json!("alice")],
    };

    let response = node.admin_rpc(transport.clone(), get_call).await?;
    if response.is_err() {
        warn!("Failed to get contact: {:?}", response.error);
    } else {
        let contact = response.value.unwrap();
        info!("Contact 'alice': {}", contact);
    }

    Ok(())
}

async fn demo_channel_management(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add a channel
    let add_call = RemoteCall {
        action: Action::AddChannel,
        args: vec![
            serde_json::json!("general"),
            serde_json::json!("channel_privkey_123"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), add_call).await?;
    if response.is_err() {
        warn!("Failed to add channel: {:?}", response.error);
    } else {
        info!("Channel 'general' added successfully");
    }

    // Add another channel
    let add_call2 = RemoteCall {
        action: Action::AddChannel,
        args: vec![
            serde_json::json!("private"),
            serde_json::json!("channel_privkey_456"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), add_call2).await?;
    if response.is_err() {
        warn!("Failed to add channel: {:?}", response.error);
    } else {
        info!("Channel 'private' added successfully");
    }

    // Get all channels
    let get_all_call = RemoteCall {
        action: Action::GetChannels,
        args: vec![],
    };

    let response = node.admin_rpc(transport.clone(), get_all_call).await?;
    if response.is_err() {
        warn!("Failed to get channels: {:?}", response.error);
    } else {
        let channels = response.value.unwrap();
        info!("All channels: {}", channels);
    }

    // Get specific channel
    let get_call = RemoteCall {
        action: Action::GetChannel,
        args: vec![serde_json::json!("general")],
    };

    let response = node.admin_rpc(transport.clone(), get_call).await?;
    if response.is_err() {
        warn!("Failed to get channel: {:?}", response.error);
    } else {
        let channel = response.value.unwrap();
        info!("Channel 'general': {}", channel);
    }

    Ok(())
}

async fn demo_peer_management(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add a peer
    let add_call = RemoteCall {
        action: Action::AddPeer,
        args: vec![
            serde_json::json!("peer1"),
            serde_json::json!(true),
            serde_json::json!("127.0.0.1:8080"),
            serde_json::json!("group1"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), add_call).await?;
    if response.is_err() {
        warn!("Failed to add peer: {:?}", response.error);
    } else {
        info!("Peer 'peer1' added successfully");
    }

    // Add another peer
    let add_call2 = RemoteCall {
        action: Action::AddPeer,
        args: vec![
            serde_json::json!("peer2"),
            serde_json::json!(false),
            serde_json::json!("127.0.0.1:8081"),
            serde_json::json!("group2"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), add_call2).await?;
    if response.is_err() {
        warn!("Failed to add peer: {:?}", response.error);
    } else {
        info!("Peer 'peer2' added successfully");
    }

    // Get all peers
    let get_all_call = RemoteCall {
        action: Action::GetPeers,
        args: vec![],
    };

    let response = node.admin_rpc(transport.clone(), get_all_call).await?;
    if response.is_err() {
        warn!("Failed to get peers: {:?}", response.error);
    } else {
        let peers = response.value.unwrap();
        info!("All peers: {}", peers);
    }

    // Get peers by group
    let get_group_call = RemoteCall {
        action: Action::GetPeers,
        args: vec![serde_json::json!("group1")],
    };

    let response = node.admin_rpc(transport.clone(), get_group_call).await?;
    if response.is_err() {
        warn!("Failed to get peers by group: {:?}", response.error);
    } else {
        let peers = response.value.unwrap();
        info!("Peers in group1: {}", peers);
    }

    Ok(())
}

async fn demo_profile_management(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add a profile
    let add_call = RemoteCall {
        action: Action::AddProfile,
        args: vec![serde_json::json!("work"), serde_json::json!(true)],
    };

    let response = node.admin_rpc(transport.clone(), add_call).await?;
    if response.is_err() {
        warn!("Failed to add profile: {:?}", response.error);
    } else {
        info!("Profile 'work' added successfully");
    }

    // Add another profile
    let add_call2 = RemoteCall {
        action: Action::AddProfile,
        args: vec![serde_json::json!("personal"), serde_json::json!(false)],
    };

    let response = node.admin_rpc(transport.clone(), add_call2).await?;
    if response.is_err() {
        warn!("Failed to add profile: {:?}", response.error);
    } else {
        info!("Profile 'personal' added successfully");
    }

    // Get all profiles
    let get_all_call = RemoteCall {
        action: Action::GetProfiles,
        args: vec![],
    };

    let response = node.admin_rpc(transport.clone(), get_all_call).await?;
    if response.is_err() {
        warn!("Failed to get profiles: {:?}", response.error);
    } else {
        let profiles = response.value.unwrap();
        info!("All profiles: {}", profiles);
    }

    // Get specific profile
    let get_call = RemoteCall {
        action: Action::GetProfile,
        args: vec![serde_json::json!("work")],
    };

    let response = node.admin_rpc(transport.clone(), get_call).await?;
    if response.is_err() {
        warn!("Failed to get profile: {:?}", response.error);
    } else {
        let profile = response.value.unwrap();
        info!("Profile 'work': {}", profile);
    }

    Ok(())
}

async fn demo_message_sending(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Send message to contact
    let send_call = RemoteCall {
        action: Action::Send,
        args: vec![
            serde_json::json!("alice"),
            serde_json::json!("Hello, Alice!"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), send_call).await?;
    if response.is_err() {
        warn!("Failed to send message to contact: {:?}", response.error);
    } else {
        info!("Message sent to contact 'alice' successfully");
    }

    // Send message to channel
    let send_channel_call = RemoteCall {
        action: Action::SendChannel,
        args: vec![
            serde_json::json!("general"),
            serde_json::json!("Hello, everyone in general channel!"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), send_channel_call).await?;
    if response.is_err() {
        warn!("Failed to send message to channel: {:?}", response.error);
    } else {
        info!("Message sent to channel 'general' successfully");
    }

    Ok(())
}

async fn demo_pickup_dropoff(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get routing key
    let routing_key = node.id().await?;

    // Create a test bundle
    let bundle = Bundle {
        data: bytes::Bytes::from("Test bundle data"),
        time: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    };

    // Dropoff bundle
    let dropoff_call = RemoteCall {
        action: Action::Dropoff,
        args: vec![serde_json::json!(bundle)],
    };

    let response = node.public_rpc(transport.clone(), dropoff_call).await?;
    if response.is_err() {
        warn!("Failed to dropoff bundle: {:?}", response.error);
    } else {
        info!("Bundle dropoff successful");
    }

    // Pickup bundle
    let pickup_call = RemoteCall {
        action: Action::Pickup,
        args: vec![
            serde_json::json!(routing_key.to_string()),
            serde_json::json!(0),
            serde_json::json!("general"),
        ],
    };

    let response = node.public_rpc(transport.clone(), pickup_call).await?;
    if response.is_err() {
        warn!("Failed to pickup bundle: {:?}", response.error);
    } else {
        let bundle = response.value.unwrap();
        info!("Bundle pickup successful: {}", bundle);
    }

    Ok(())
}

async fn demo_error_handling(
    node: &Arc<MemoryNode>,
    transport: &Arc<UdpTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Test invalid argument count
    let invalid_call = RemoteCall {
        action: Action::AddContact,
        args: vec![serde_json::json!("test")], // Missing second argument
    };

    let response = node.admin_rpc(transport.clone(), invalid_call).await?;
    if response.is_err() {
        info!(
            "Expected error for invalid argument count: {:?}",
            response.error
        );
    } else {
        warn!("Unexpected success for invalid argument count");
    }

    // Test invalid argument type
    let invalid_type_call = RemoteCall {
        action: Action::AddContact,
        args: vec![
            serde_json::json!(123), // Should be string
            serde_json::json!("test_key"),
        ],
    };

    let response = node.admin_rpc(transport.clone(), invalid_type_call).await?;
    if response.is_err() {
        info!(
            "Expected error for invalid argument type: {:?}",
            response.error
        );
    } else {
        warn!("Unexpected success for invalid argument type");
    }

    // Test getting non-existent contact
    let get_nonexistent_call = RemoteCall {
        action: Action::GetContact,
        args: vec![serde_json::json!("nonexistent")],
    };

    let response = node
        .admin_rpc(transport.clone(), get_nonexistent_call)
        .await?;
    if response.is_err() {
        info!(
            "Expected error for non-existent contact: {:?}",
            response.error
        );
    } else {
        warn!("Unexpected success for non-existent contact");
    }

    // Test unknown action
    let unknown_call = RemoteCall {
        action: Action::ID, // Using public action in admin context
        args: vec![],
    };

    let response = node.admin_rpc(transport.clone(), unknown_call).await?;
    if response.is_err() {
        info!(
            "Expected error for unknown admin action: {:?}",
            response.error
        );
    } else {
        warn!("Unexpected success for unknown admin action");
    }

    Ok(())
}
