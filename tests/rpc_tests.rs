//! Comprehensive RPC tests for the RatNet implementation

use bytes::Bytes;
use ratnet::api::Node;
use ratnet::api::{
    Action, Bundle, Channel, Contact, KeyPair, Msg, Peer, PubKey, RemoteCall, RemoteResponse,
};
use ratnet::nodes::MemoryNode;
use ratnet::prelude::{
    args_from_bytes, args_to_bytes, remote_call_from_bytes, remote_call_to_bytes,
    remote_response_from_bytes, remote_response_to_bytes,
};
use ratnet::transports::MemoryTransport;
use serde_json::Value;
use std::sync::Arc;

#[tokio::test]
async fn test_binary_serialization_roundtrip() {
    // Test basic types
    let test_cases = vec![
        (Value::Null, "null"),
        (Value::Number(serde_json::Number::from(42i64)), "int64"),
        (Value::Number(serde_json::Number::from(123u64)), "uint64"),
        (Value::String("hello world".to_string()), "string"),
        (
            Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ]),
            "array",
        ),
    ];

    for (value, description) in test_cases {
        let args = vec![value.clone()];
        let serialized = args_to_bytes(&args).expect("Failed to serialize");
        let deserialized = args_from_bytes(&serialized).expect("Failed to deserialize");

        assert_eq!(args, deserialized, "Roundtrip failed for {description}");
    }
}

#[tokio::test]
async fn test_remote_call_serialization() {
    let call = RemoteCall {
        action: Action::ID,
        args: vec![Value::String("test".to_string())],
    };

    let serialized = remote_call_to_bytes(&call).expect("Failed to serialize call");
    let deserialized = remote_call_from_bytes(&serialized).expect("Failed to deserialize call");

    assert_eq!(call.action, deserialized.action);
    assert_eq!(call.args, deserialized.args);
}

#[tokio::test]
async fn test_remote_response_serialization() {
    let response = RemoteResponse::success(Value::String("test result".to_string()));

    let serialized = remote_response_to_bytes(&response).expect("Failed to serialize response");
    let deserialized =
        remote_response_from_bytes(&serialized).expect("Failed to deserialize response");

    assert_eq!(response.error, deserialized.error);
    assert_eq!(response.value, deserialized.value);
}

#[tokio::test]
async fn test_error_response_serialization() {
    let response = RemoteResponse::error("test error".to_string());

    let serialized = remote_response_to_bytes(&response).expect("Failed to serialize response");
    let deserialized =
        remote_response_from_bytes(&serialized).expect("Failed to deserialize response");

    assert_eq!(response.error, deserialized.error);
    assert_eq!(response.value, deserialized.value);
}

#[tokio::test]
async fn test_public_rpc_id() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Start the node
    node.start().await.expect("Failed to start node");

    let call = RemoteCall {
        action: Action::ID,
        args: vec![],
    };

    let response = node
        .public_rpc(transport, call)
        .await
        .expect("RPC call failed");

    // Should succeed and return a public key
    assert!(!response.is_err());
    assert!(response.value.is_some());

    let binding = response.value.unwrap();
    let pubkey_str = binding.as_str().expect("Expected string");
    assert!(!pubkey_str.is_empty());
}

#[tokio::test]
async fn test_admin_rpc_cid() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Start the node
    node.start().await.expect("Failed to start node");

    let call = RemoteCall {
        action: Action::CID,
        args: vec![],
    };

    let response = node
        .admin_rpc(transport, call)
        .await
        .expect("RPC call failed");

    // Should succeed and return a content key
    assert!(!response.is_err());
    assert!(response.value.is_some());

    let binding = response.value.unwrap();
    let pubkey_str = binding.as_str().expect("Expected string");
    assert!(!pubkey_str.is_empty());
}

#[tokio::test]
async fn test_admin_rpc_contact_management() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Start the node
    node.start().await.expect("Failed to start node");

    // Add a contact
    let add_call = RemoteCall {
        action: Action::AddContact,
        args: vec![
            Value::String("test_contact".to_string()),
            Value::String("test_pubkey".to_string()),
        ],
    };

    let add_response = node
        .admin_rpc(transport.clone(), add_call)
        .await
        .expect("RPC call failed");
    assert!(!add_response.is_err());

    // Get the contact
    let get_call = RemoteCall {
        action: Action::GetContact,
        args: vec![Value::String("test_contact".to_string())],
    };

    let get_response = node
        .admin_rpc(transport.clone(), get_call)
        .await
        .expect("RPC call failed");
    assert!(!get_response.is_err());

    let contact_value = get_response.value.expect("Expected value");
    let contact: Contact =
        serde_json::from_value(contact_value).expect("Failed to deserialize contact");

    assert_eq!(contact.name, "test_contact");
    assert_eq!(contact.pubkey, "test_pubkey");

    // Get all contacts
    let get_all_call = RemoteCall {
        action: Action::GetContacts,
        args: vec![],
    };

    let get_all_response = node
        .admin_rpc(transport.clone(), get_all_call)
        .await
        .expect("RPC call failed");
    assert!(!get_all_response.is_err());

    let contacts_value = get_all_response.value.expect("Expected value");
    let contacts: Vec<Contact> =
        serde_json::from_value(contacts_value).expect("Failed to deserialize contacts");

    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].name, "test_contact");

    // Delete the contact
    let delete_call = RemoteCall {
        action: Action::DeleteContact,
        args: vec![Value::String("test_contact".to_string())],
    };

    let delete_response = node
        .admin_rpc(transport, delete_call)
        .await
        .expect("RPC call failed");
    assert!(!delete_response.is_err());
}

#[tokio::test]
async fn test_admin_rpc_channel_management() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Start the node
    node.start().await.expect("Failed to start node");

    // Add a channel
    let keypair = KeyPair::generate_ed25519().expect("Failed to generate keypair");
    let privkey_str = keypair.to_string().expect("Failed to serialize keypair");

    let add_call = RemoteCall {
        action: Action::AddChannel,
        args: vec![
            Value::String("test_channel".to_string()),
            Value::String(privkey_str),
        ],
    };

    let add_response = node
        .admin_rpc(transport.clone(), add_call)
        .await
        .expect("RPC call failed");
    assert!(!add_response.is_err());

    // Get the channel
    let get_call = RemoteCall {
        action: Action::GetChannel,
        args: vec![Value::String("test_channel".to_string())],
    };

    let get_response = node
        .admin_rpc(transport.clone(), get_call)
        .await
        .expect("RPC call failed");
    assert!(!get_response.is_err());

    let channel_value = get_response.value.expect("Expected value");
    let channel: Channel =
        serde_json::from_value(channel_value).expect("Failed to deserialize channel");

    assert_eq!(channel.name, "test_channel");
    assert!(!channel.pubkey.is_empty());

    // Get all channels
    let get_all_call = RemoteCall {
        action: Action::GetChannels,
        args: vec![],
    };

    let get_all_response = node
        .admin_rpc(transport.clone(), get_all_call)
        .await
        .expect("RPC call failed");
    assert!(!get_all_response.is_err());

    let channels_value = get_all_response.value.expect("Expected value");
    let channels: Vec<Channel> =
        serde_json::from_value(channels_value).expect("Failed to deserialize channels");

    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].name, "test_channel");

    // Delete the channel
    let delete_call = RemoteCall {
        action: Action::DeleteChannel,
        args: vec![Value::String("test_channel".to_string())],
    };

    let delete_response = node
        .admin_rpc(transport, delete_call)
        .await
        .expect("RPC call failed");
    assert!(!delete_response.is_err());
}

#[tokio::test]
async fn test_admin_rpc_peer_management() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Start the node
    node.start().await.expect("Failed to start node");

    // Add a peer
    let add_call = RemoteCall {
        action: Action::AddPeer,
        args: vec![
            Value::String("test_peer".to_string()),
            Value::Bool(true),
            Value::String("test://uri".to_string()),
            Value::String("test_group".to_string()),
        ],
    };

    let add_response = node
        .admin_rpc(transport.clone(), add_call)
        .await
        .expect("RPC call failed");
    assert!(!add_response.is_err());

    // Get the peer
    let get_call = RemoteCall {
        action: Action::GetPeer,
        args: vec![Value::String("test_peer".to_string())],
    };

    let get_response = node
        .admin_rpc(transport.clone(), get_call)
        .await
        .expect("RPC call failed");
    assert!(!get_response.is_err());

    let peer_value = get_response.value.expect("Expected value");
    let peer: Peer = serde_json::from_value(peer_value).expect("Failed to deserialize peer");

    assert_eq!(peer.name, "test_peer");
    assert_eq!(peer.uri, "test://uri");
    assert_eq!(peer.group, "test_group");
    assert!(peer.enabled);

    // Get all peers
    let get_all_call = RemoteCall {
        action: Action::GetPeers,
        args: vec![],
    };

    let get_all_response = node
        .admin_rpc(transport.clone(), get_all_call)
        .await
        .expect("RPC call failed");
    assert!(!get_all_response.is_err());

    let peers_value = get_all_response.value.expect("Expected value");
    let peers: Vec<Peer> =
        serde_json::from_value(peers_value).expect("Failed to deserialize peers");

    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].name, "test_peer");

    // Delete the peer
    let delete_call = RemoteCall {
        action: Action::DeletePeer,
        args: vec![Value::String("test_peer".to_string())],
    };

    let delete_response = node
        .admin_rpc(transport, delete_call)
        .await
        .expect("RPC call failed");
    assert!(!delete_response.is_err());
}

#[tokio::test]
async fn test_admin_rpc_send_msg() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Start the node
    node.start().await.expect("Failed to start node");

    // Create a test message
    let msg = Msg {
        name: "test_message".to_string(),
        content: Bytes::from("Hello, World!"),
        is_chan: false,
        pubkey: PubKey::Nil,
        chunked: false,
        stream_header: false,
    };

    let msg_value = serde_json::to_value(&msg).expect("Failed to serialize message");

    let call = RemoteCall {
        action: Action::SendMsg,
        args: vec![msg_value],
    };

    let response = node
        .admin_rpc(transport, call)
        .await
        .expect("RPC call failed");

    // Should succeed
    assert!(!response.is_err());
}

#[tokio::test]
async fn test_public_rpc_pickup_dropoff() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Start the node
    node.start().await.expect("Failed to start node");

    // Create a test message
    let test_msg = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("test data"),
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: false,
        stream_header: false,
    };

    // Create a bundle with the test message
    let messages = vec![test_msg];
    let bundle_data = serde_json::to_vec(&messages).expect("Failed to serialize messages");
    let bundle = Bundle {
        data: Bytes::from(bundle_data),
        time: 1234567890,
    };

    let bundle_value = serde_json::to_value(&bundle).expect("Failed to serialize bundle");

    // Dropoff the bundle
    let dropoff_call = RemoteCall {
        action: Action::Dropoff,
        args: vec![bundle_value],
    };

    let dropoff_response = node
        .public_rpc(transport.clone(), dropoff_call)
        .await
        .expect("RPC call failed");
    assert!(!dropoff_response.is_err());

    // Send a message to the outbox
    let outbox_msg = Msg {
        name: "test_channel".to_string(),
        content: Bytes::from("outbox data"),
        is_chan: true,
        pubkey: PubKey::nil(),
        chunked: false,
        stream_header: false,
    };

    node.send_msg(outbox_msg)
        .await
        .expect("Failed to send message");

    // Get the node's public key for pickup
    let id_call = RemoteCall {
        action: Action::ID,
        args: vec![],
    };
    let id_response = node
        .public_rpc(transport.clone(), id_call)
        .await
        .expect("RPC call failed");
    assert!(!id_response.is_err());
    let node_pubkey = id_response
        .value
        .expect("Expected value")
        .as_str()
        .expect("Expected string")
        .to_string();

    // Pickup messages from the test channel
    let pickup_call = RemoteCall {
        action: Action::Pickup,
        args: vec![
            Value::String(node_pubkey),
            Value::Number(serde_json::Number::from(0i64)),
            Value::Number(serde_json::Number::from(10000i64)), // max_bytes
            Value::Array(vec![Value::String("test_channel".to_string())]), // channel_names
        ],
    };

    let pickup_response = node
        .public_rpc(transport, pickup_call)
        .await
        .expect("RPC call failed");
    assert!(!pickup_response.is_err());

    let pickup_bundle_value = pickup_response.value.expect("Expected value");
    let pickup_bundle: Bundle =
        serde_json::from_value(pickup_bundle_value).expect("Failed to deserialize bundle");

    // The bundle should contain the message from the outbox
    assert!(!pickup_bundle.data.is_empty());

    // Deserialize the bundle data to verify it contains our message
    let messages: Vec<Msg> =
        serde_json::from_slice(&pickup_bundle.data).expect("Failed to deserialize bundle messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].name, "test_channel");
    assert_eq!(messages[0].content, Bytes::from("outbox data"));
}

#[tokio::test]
async fn test_error_handling() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Test invalid action
    let call = RemoteCall {
        action: Action::Null,
        args: vec![],
    };

    let response = node
        .admin_rpc(transport, call)
        .await
        .expect("RPC call failed");
    assert!(response.is_err());
    assert!(response.error.unwrap().contains("Unknown admin action"));
}

#[tokio::test]
async fn test_invalid_arguments() {
    let node = MemoryNode::new();
    let transport = Arc::new(MemoryTransport::new());

    // Test missing arguments
    let call = RemoteCall {
        action: Action::GetContact,
        args: vec![], // Missing required argument
    };

    let response = node
        .admin_rpc(transport, call)
        .await
        .expect("RPC call failed");
    assert!(response.is_err());
    assert!(response.error.unwrap().contains("Invalid argument count"));
}
