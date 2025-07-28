//! Example demonstrating the filesystem node implementation

use ratnet::api::*;
use ratnet::nodes::FilesystemNode;
use ratnet::router::DefaultRouter;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting filesystem node example...");

    // Create a temporary directory for the filesystem node
    let temp_dir = std::env::temp_dir().join("ratnet_fs_node");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }

    info!("Using filesystem storage at: {:?}", temp_dir);

    // Create the filesystem node
    let node = Arc::new(FilesystemNode::new(temp_dir.clone()));

    // Set up a router
    let router = Arc::new(DefaultRouter::new());
    node.set_router(router);

    // Start the node
    node.start().await?;
    info!("Filesystem node started successfully");

    // Test basic functionality
    test_basic_operations(&node).await?;

    // Test contact management
    test_contact_management(&node).await?;

    // Test channel management
    test_channel_management(&node).await?;

    // Test profile management
    test_profile_management(&node).await?;

    // Test peer management
    test_peer_management(&node).await?;

    // Test key operations
    test_key_operations(&node).await?;

    // Stop the node
    node.stop().await?;
    info!("Filesystem node stopped");

    // Clean up
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
        info!("Cleaned up temporary directory");
    }

    info!("Filesystem node example completed successfully!");
    Ok(())
}

async fn test_basic_operations(
    node: &Arc<FilesystemNode>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing basic operations...");

    // Test running state
    assert!(node.is_running());
    info!("✓ Node is running");

    // Test router
    let _router = node.router();
    info!("✓ Router initialized");

    // Test policies
    let policies = node.get_policies();
    assert_eq!(policies.len(), 0);
    info!("✓ Policies initialized (empty)");

    Ok(())
}

async fn test_contact_management(
    node: &Arc<FilesystemNode>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing contact management...");

    // Generate a proper key pair for the contact
    let keypair = KeyPair::generate_ed25519()?;
    let contact_key = keypair.public_key().to_string();

    // Add a contact
    let contact_name = "alice";
    node.add_contact(contact_name.to_string(), contact_key.clone())
        .await?;
    info!("✓ Added contact: {}", contact_name);

    // Get the contact
    let contact = node.get_contact(contact_name).await?;
    assert_eq!(contact.name, contact_name);
    assert_eq!(contact.pubkey, contact_key);
    info!("✓ Retrieved contact: {}", contact.name);

    // Get all contacts
    let contacts = node.get_contacts().await?;
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].name, contact_name);
    info!("✓ Retrieved all contacts: {}", contacts.len());

    // Delete the contact
    node.delete_contact(contact_name).await?;
    info!("✓ Deleted contact: {}", contact_name);

    // Verify deletion
    let contacts = node.get_contacts().await?;
    assert_eq!(contacts.len(), 0);
    info!("✓ Verified contact deletion");

    Ok(())
}

async fn test_channel_management(
    node: &Arc<FilesystemNode>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing channel management...");

    // Generate a proper keypair for the channel
    let keypair = KeyPair::generate_ed25519()?;
    let privkey = keypair.to_string()?;

    // Add channel
    let channel_name = "test-channel";
    node.add_channel(channel_name.to_string(), privkey).await?;
    info!("✓ Added channel: {}", channel_name);

    // Get channel
    let channel = node.get_channel(channel_name).await?;
    assert_eq!(channel.name, channel_name);
    info!("✓ Retrieved channel: {}", channel_name);

    // Get channel private key
    let retrieved_privkey = node.get_channel_privkey(channel_name).await?;
    assert!(!retrieved_privkey.is_empty());
    info!("✓ Retrieved channel private key");

    // Get all channels
    let channels = node.get_channels().await?;
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].name, channel_name);
    info!("✓ Retrieved all channels: {}", channels.len());

    // Delete channel
    node.delete_channel(channel_name).await?;
    info!("✓ Deleted channel: {}", channel_name);

    // Verify deletion
    assert!(node.get_channel(channel_name).await.is_err());
    info!("✓ Verified channel deletion");

    Ok(())
}

async fn test_profile_management(
    node: &Arc<FilesystemNode>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing profile management...");

    // Add a profile
    let profile_name = "test-profile";
    let enabled = true;

    node.add_profile(profile_name.to_string(), enabled).await?;
    info!("✓ Added profile: {}", profile_name);

    // Get the profile
    let profile = node.get_profile(profile_name).await?;
    assert_eq!(profile.name, profile_name);
    assert_eq!(profile.enabled, enabled);
    assert!(!profile.pubkey.is_empty());
    info!("✓ Retrieved profile: {}", profile.name);

    // Load profile (get public key)
    let profile_pubkey = node.load_profile(profile_name).await?;
    assert_eq!(profile_pubkey.to_string(), profile.pubkey);
    info!("✓ Loaded profile public key");

    // Get all profiles
    let profiles = node.get_profiles().await?;
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, profile_name);
    info!("✓ Retrieved all profiles: {}", profiles.len());

    // Delete the profile
    node.delete_profile(profile_name).await?;
    info!("✓ Deleted profile: {}", profile_name);

    // Verify deletion
    let profiles = node.get_profiles().await?;
    assert_eq!(profiles.len(), 0);
    info!("✓ Verified profile deletion");

    Ok(())
}

async fn test_peer_management(
    node: &Arc<FilesystemNode>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing peer management...");

    // Add a peer
    let peer_name = "test-peer";
    let enabled = true;
    let uri = "https://example.com:8080";
    let group = "test-group";

    node.add_peer(
        peer_name.to_string(),
        enabled,
        uri.to_string(),
        Some(group.to_string()),
    )
    .await?;
    info!("✓ Added peer: {}", peer_name);

    // Get the peer
    let peer = node.get_peer(peer_name).await?;
    assert_eq!(peer.name, peer_name);
    assert_eq!(peer.enabled, enabled);
    assert_eq!(peer.uri, uri);
    assert_eq!(peer.group, group);
    info!("✓ Retrieved peer: {}", peer.name);

    // Get peers by group
    let group_peers = node.get_peers(Some(group.to_string())).await?;
    assert_eq!(group_peers.len(), 1);
    assert_eq!(group_peers[0].name, peer_name);
    info!("✓ Retrieved peers by group: {}", group_peers.len());

    // Get all peers
    let all_peers = node.get_peers(None).await?;
    assert_eq!(all_peers.len(), 1);
    assert_eq!(all_peers[0].name, peer_name);
    info!("✓ Retrieved all peers: {}", all_peers.len());

    // Delete the peer
    node.delete_peer(peer_name).await?;
    info!("✓ Deleted peer: {}", peer_name);

    // Verify deletion
    let all_peers = node.get_peers(None).await?;
    assert_eq!(all_peers.len(), 0);
    info!("✓ Verified peer deletion");

    Ok(())
}

async fn test_key_operations(node: &Arc<FilesystemNode>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing key operations...");

    // Test routing key (ID)
    let routing_key = node.id().await?;
    assert!(matches!(routing_key, PubKey::Ed25519(_)));
    info!("✓ Retrieved routing key (ID)");

    // Test content key (CID)
    let content_key = node.cid().await?;
    assert!(matches!(content_key, PubKey::Ed25519(_)));
    info!("✓ Retrieved content key (CID)");

    // Verify keys are different
    assert_ne!(routing_key.to_string(), content_key.to_string());
    info!("✓ Verified routing and content keys are different");

    Ok(())
}
