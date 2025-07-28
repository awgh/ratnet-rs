//! Database functionality tests

use ratnet::api::crypto::KeyPair; // <-- Add this import
use ratnet::api::{Channel, Contact, Peer, Profile};
use ratnet::database::{Database, SqliteDatabase};
use std::sync::Arc;
use tokio_test;

async fn setup_test_db() -> Arc<SqliteDatabase> {
    let db = SqliteDatabase::new_memory()
        .await
        .expect("Failed to create test database");
    db.bootstrap().await.expect("Failed to bootstrap database");
    Arc::new(db)
}

#[tokio::test]
async fn test_database_bootstrap() {
    let db = setup_test_db().await;
    // If we got here, bootstrap worked
    assert!(true);
}

#[tokio::test]
async fn test_contact_operations() {
    let db = setup_test_db().await;

    // Test add contact
    db.add_contact("test_contact".to_string(), "test_pubkey_123".to_string())
        .await
        .expect("Failed to add contact");

    // Test get contact
    let contact = db
        .get_contact("test_contact")
        .await
        .expect("Failed to get contact")
        .expect("Contact should exist");

    assert_eq!(contact.name, "test_contact");
    assert_eq!(contact.pubkey, "test_pubkey_123");

    // Test get all contacts
    let contacts = db.get_contacts().await.expect("Failed to get contacts");
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].name, "test_contact");

    // Test update contact (should replace)
    db.add_contact("test_contact".to_string(), "updated_pubkey_456".to_string())
        .await
        .expect("Failed to update contact");

    let updated_contact = db
        .get_contact("test_contact")
        .await
        .expect("Failed to get updated contact")
        .expect("Contact should exist");

    assert_eq!(updated_contact.pubkey, "updated_pubkey_456");

    // Test delete contact
    db.delete_contact("test_contact")
        .await
        .expect("Failed to delete contact");

    let deleted_contact = db
        .get_contact("test_contact")
        .await
        .expect("Failed to check deleted contact");

    assert!(deleted_contact.is_none());
}

#[tokio::test]
async fn test_channel_operations() {
    let db = setup_test_db().await;

    // Generate a real Ed25519 keypair for the channel
    let keypair = KeyPair::generate_ed25519().expect("Failed to generate keypair");
    let privkey_str = keypair.to_string().expect("Failed to serialize keypair");
    let pubkey = keypair.public_key().to_string();

    // Test add channel
    db.add_channel("test_channel".to_string(), privkey_str.clone())
        .await
        .expect("Failed to add channel");

    // Test get channel
    let channel = db
        .get_channel("test_channel")
        .await
        .expect("Failed to get channel")
        .expect("Channel should exist");

    assert_eq!(channel.name, "test_channel");
    assert_eq!(channel.pubkey, pubkey); // Now check real pubkey

    // Test get channel private key
    let privkey = db
        .get_channel_privkey("test_channel")
        .await
        .expect("Failed to get channel privkey")
        .expect("Privkey should exist");

    assert_eq!(privkey, privkey_str);

    // Test get all channels
    let channels = db.get_channels().await.expect("Failed to get channels");
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].name, "test_channel");
    assert_eq!(channels[0].pubkey, pubkey);

    // Test delete channel
    db.delete_channel("test_channel")
        .await
        .expect("Failed to delete channel");

    let deleted_channel = db
        .get_channel("test_channel")
        .await
        .expect("Failed to check deleted channel");

    assert!(deleted_channel.is_none());
}

#[tokio::test]
async fn test_profile_operations() {
    let db = setup_test_db().await;

    // Generate a real Ed25519 keypair for the profile
    let keypair = KeyPair::generate_ed25519().expect("Failed to generate keypair");
    let privkey_str = keypair.to_string().expect("Failed to serialize keypair");
    let pubkey = keypair.public_key().to_string();

    // Test add profile
    db.add_profile("test_profile".to_string(), true, privkey_str.clone())
        .await
        .expect("Failed to add profile");

    // Test get profile
    let profile = db
        .get_profile("test_profile")
        .await
        .expect("Failed to get profile")
        .expect("Profile should exist");

    assert_eq!(profile.name, "test_profile");
    assert_eq!(profile.enabled, true);
    assert_eq!(profile.pubkey, pubkey);

    // Test get profile private key
    let privkey = db
        .get_profile_privkey("test_profile")
        .await
        .expect("Failed to get profile privkey")
        .expect("Privkey should exist");

    assert_eq!(privkey, privkey_str);

    // Test get all profiles
    let profiles = db.get_profiles().await.expect("Failed to get profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "test_profile");
    assert_eq!(profiles[0].enabled, true);
    assert_eq!(profiles[0].pubkey, pubkey);

    // Test update profile (disabled)
    let keypair2 = KeyPair::generate_ed25519().expect("Failed to generate keypair");
    let privkey_str2 = keypair2.to_string().expect("Failed to serialize keypair");
    let pubkey2 = keypair2.public_key().to_string();
    db.add_profile("test_profile".to_string(), false, privkey_str2.clone())
        .await
        .expect("Failed to update profile");

    let updated_profile = db
        .get_profile("test_profile")
        .await
        .expect("Failed to get updated profile")
        .expect("Profile should exist");

    assert_eq!(updated_profile.enabled, false);
    assert_eq!(updated_profile.pubkey, pubkey2);

    // Test delete profile
    db.delete_profile("test_profile")
        .await
        .expect("Failed to delete profile");

    let deleted_profile = db
        .get_profile("test_profile")
        .await
        .expect("Failed to check deleted profile");

    assert!(deleted_profile.is_none());
}

#[tokio::test]
async fn test_peer_operations() {
    let db = setup_test_db().await;

    // Test add peer
    db.add_peer(
        "test_peer".to_string(),
        true,
        "127.0.0.1:8080".to_string(),
        "default".to_string(),
    )
    .await
    .expect("Failed to add peer");

    // Test get peer
    let peer = db
        .get_peer("test_peer")
        .await
        .expect("Failed to get peer")
        .expect("Peer should exist");

    assert_eq!(peer.name, "test_peer");
    assert_eq!(peer.enabled, true);
    assert_eq!(peer.uri, "127.0.0.1:8080");
    assert_eq!(peer.group, "default");

    // Test get all peers
    let peers = db.get_peers(None).await.expect("Failed to get peers");
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].name, "test_peer");

    // Add another peer in a different group
    db.add_peer(
        "test_peer2".to_string(),
        false,
        "127.0.0.1:8081".to_string(),
        "secondary".to_string(),
    )
    .await
    .expect("Failed to add second peer");

    // Test get peers by group
    let default_peers = db
        .get_peers(Some("default"))
        .await
        .expect("Failed to get default peers");
    assert_eq!(default_peers.len(), 1);
    assert_eq!(default_peers[0].name, "test_peer");

    let secondary_peers = db
        .get_peers(Some("secondary"))
        .await
        .expect("Failed to get secondary peers");
    assert_eq!(secondary_peers.len(), 1);
    assert_eq!(secondary_peers[0].name, "test_peer2");
    assert_eq!(secondary_peers[0].enabled, false);

    // Test get all peers
    let all_peers = db.get_peers(None).await.expect("Failed to get all peers");
    assert_eq!(all_peers.len(), 2);

    // Test delete peer
    db.delete_peer("test_peer")
        .await
        .expect("Failed to delete peer");

    let deleted_peer = db
        .get_peer("test_peer")
        .await
        .expect("Failed to check deleted peer");

    assert!(deleted_peer.is_none());

    // Should still have the second peer
    let remaining_peers = db
        .get_peers(None)
        .await
        .expect("Failed to get remaining peers");
    assert_eq!(remaining_peers.len(), 1);
    assert_eq!(remaining_peers[0].name, "test_peer2");
}

#[tokio::test]
async fn test_config_operations() {
    let db = setup_test_db().await;

    // Test set config
    db.set_config("test_key".to_string(), "test_value".to_string())
        .await
        .expect("Failed to set config");

    // Test get config
    let value = db
        .get_config("test_key")
        .await
        .expect("Failed to get config")
        .expect("Config should exist");

    assert_eq!(value, "test_value");

    // Test update config
    db.set_config("test_key".to_string(), "updated_value".to_string())
        .await
        .expect("Failed to update config");

    let updated_value = db
        .get_config("test_key")
        .await
        .expect("Failed to get updated config")
        .expect("Config should exist");

    assert_eq!(updated_value, "updated_value");

    // Test delete config
    db.delete_config("test_key")
        .await
        .expect("Failed to delete config");

    let deleted_value = db
        .get_config("test_key")
        .await
        .expect("Failed to check deleted config");

    assert!(deleted_value.is_none());
}

#[tokio::test]
async fn test_outbox_operations() {
    let db = setup_test_db().await;

    let test_msg = b"Hello, RatNet!";
    let timestamp1 = 1000;
    let timestamp2 = 2000;
    let timestamp3 = 3000;

    // Test add outbox messages
    db.add_outbox_msg(Some("general".to_string()), test_msg, timestamp1)
        .await
        .expect("Failed to add outbox message");

    db.add_outbox_msg(None, b"Another message", timestamp2)
        .await
        .expect("Failed to add second outbox message");

    db.add_outbox_msg(Some("private".to_string()), b"Private message", timestamp3)
        .await
        .expect("Failed to add third outbox message");

    // Test get messages since timestamp
    let messages = db
        .get_outbox_msgs(500)
        .await
        .expect("Failed to get outbox messages");
    assert_eq!(messages.len(), 3);

    let messages_since_1500 = db
        .get_outbox_msgs(1500)
        .await
        .expect("Failed to get filtered messages");
    assert_eq!(messages_since_1500.len(), 2);

    // Test delete old messages
    db.delete_outbox_msgs(2500)
        .await
        .expect("Failed to delete old messages");

    let remaining_messages = db
        .get_outbox_msgs(0)
        .await
        .expect("Failed to get remaining messages");
    assert_eq!(remaining_messages.len(), 1);
    assert_eq!(remaining_messages[0].timestamp, timestamp3);
}

#[tokio::test]
async fn test_streaming_operations() {
    let db = setup_test_db().await;

    let stream_id = 12345;
    let total_chunks = 3;
    let channel_name = "test_stream_channel".to_string();

    // Test add stream
    db.add_stream(stream_id, total_chunks, channel_name.clone())
        .await
        .expect("Failed to add stream");

    // Test get stream
    let stream = db
        .get_stream(stream_id)
        .await
        .expect("Failed to get stream")
        .expect("Stream should exist");

    assert_eq!(stream.stream_id, stream_id);
    assert_eq!(stream.num_chunks, total_chunks);
    assert_eq!(stream.channel_name, channel_name);

    // Test add chunks
    let chunk1_data = b"Chunk 1 data";
    let chunk2_data = b"Chunk 2 data";
    let chunk3_data = b"Chunk 3 data";

    db.add_chunk(stream_id, 0, chunk1_data)
        .await
        .expect("Failed to add chunk 0");

    db.add_chunk(stream_id, 1, chunk2_data)
        .await
        .expect("Failed to add chunk 1");

    db.add_chunk(stream_id, 2, chunk3_data)
        .await
        .expect("Failed to add chunk 2");

    // Test get individual chunk
    let chunk1 = db
        .get_chunk(stream_id, 0)
        .await
        .expect("Failed to get chunk 0")
        .expect("Chunk 0 should exist");

    assert_eq!(chunk1.stream_id, stream_id);
    assert_eq!(chunk1.chunk_num, 0);
    assert_eq!(chunk1.data.as_ref(), chunk1_data);

    // Test get all chunks for stream
    let all_chunks = db
        .get_stream_chunks(stream_id)
        .await
        .expect("Failed to get stream chunks");

    assert_eq!(all_chunks.len(), 3);
    assert_eq!(all_chunks[0].chunk_num, 0);
    assert_eq!(all_chunks[1].chunk_num, 1);
    assert_eq!(all_chunks[2].chunk_num, 2);

    // Test delete stream chunks
    db.delete_stream_chunks(stream_id)
        .await
        .expect("Failed to delete stream chunks");

    let remaining_chunks = db
        .get_stream_chunks(stream_id)
        .await
        .expect("Failed to get remaining chunks");

    assert_eq!(remaining_chunks.len(), 0);

    // Test delete stream
    db.delete_stream(stream_id)
        .await
        .expect("Failed to delete stream");

    let deleted_stream = db
        .get_stream(stream_id)
        .await
        .expect("Failed to check deleted stream");

    assert!(deleted_stream.is_none());
}

#[tokio::test]
async fn test_multiple_database_instances() {
    // Test that multiple database instances work correctly
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;

    // Add data to first database
    db1.add_contact("contact1".to_string(), "pubkey1".to_string())
        .await
        .expect("Failed to add contact to db1");

    // Add different data to second database
    db2.add_contact("contact2".to_string(), "pubkey2".to_string())
        .await
        .expect("Failed to add contact to db2");

    // Verify isolation
    let db1_contacts = db1
        .get_contacts()
        .await
        .expect("Failed to get db1 contacts");
    let db2_contacts = db2
        .get_contacts()
        .await
        .expect("Failed to get db2 contacts");

    assert_eq!(db1_contacts.len(), 1);
    assert_eq!(db1_contacts[0].name, "contact1");

    assert_eq!(db2_contacts.len(), 1);
    assert_eq!(db2_contacts[0].name, "contact2");
}
