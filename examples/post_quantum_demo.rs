use ratnet::api::crypto::{decrypt, encrypt, generate_hybrid_keypair, KeyPair, PubKey};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting Post-Quantum Cryptography Demo...");

    // Demo 1: Ed25519 Key Generation and Signing
    info!("\n=== Demo 1: Ed25519 Key Generation and Signing ===");
    demo_ed25519_functionality().await?;

    // Demo 2: Kyber Post-Quantum Key Generation
    info!("\n=== Demo 2: Kyber Post-Quantum Key Generation ===");
    demo_kyber_key_generation().await?;

    // Demo 3: Kyber Encryption and Decryption
    info!("\n=== Demo 3: Kyber Encryption and Decryption ===");
    demo_kyber_encryption().await?;

    // Demo 4: Hybrid Key Pairs
    info!("\n=== Demo 4: Hybrid Key Pairs (Ed25519 + Kyber) ===");
    demo_hybrid_keypairs().await?;

    // Demo 5: Performance Comparison
    info!("\n=== Demo 5: Performance Comparison ===");
    demo_performance_comparison().await?;

    // Demo 6: Key Serialization
    info!("\n=== Demo 6: Key Serialization ===");
    demo_key_serialization().await?;

    info!("Post-Quantum Cryptography Demo completed successfully!");
    Ok(())
}

async fn demo_ed25519_functionality() -> Result<(), Box<dyn std::error::Error>> {
    info!("Generating Ed25519 key pair...");
    let keypair = KeyPair::generate_ed25519()?;
    let pubkey = keypair.public_key();

    info!("Ed25519 Public Key: {}", pubkey);
    info!("Key type: {:?}", pubkey);

    // Test signing and verification
    let message = b"Hello, post-quantum world!";
    info!("Signing message: {}", String::from_utf8_lossy(message));

    let signature = keypair.sign(message)?;
    info!("Signature length: {} bytes", signature.len());

    let is_valid = KeyPair::verify(&pubkey, message, &signature)?;
    info!(
        "Signature verification: {}",
        if is_valid { "SUCCESS" } else { "FAILED" }
    );

    // Test signature verification with wrong data
    let wrong_message = b"Wrong message!";
    let is_valid_wrong = KeyPair::verify(&pubkey, wrong_message, &signature)?;
    info!(
        "Wrong message verification: {}",
        if is_valid_wrong {
            "SUCCESS (UNEXPECTED)"
        } else {
            "FAILED (EXPECTED)"
        }
    );

    Ok(())
}

async fn demo_kyber_key_generation() -> Result<(), Box<dyn std::error::Error>> {
    info!("Generating Kyber post-quantum key pair...");
    let keypair = KeyPair::generate_kyber()?;
    let pubkey = keypair.public_key();

    info!("Kyber Public Key: {}", pubkey);
    info!("Key type: {:?}", pubkey);
    info!("Public key length: {} bytes", pubkey.as_bytes().len());

    // Note: Kyber keys cannot be used for signing
    match keypair.sign(b"test") {
        Ok(_) => warn!("Unexpected: Kyber key signing succeeded"),
        Err(e) => info!("Expected: Kyber key signing failed - {}", e),
    }

    Ok(())
}

async fn demo_kyber_encryption() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing Kyber encryption and decryption...");

    // Generate Kyber key pair
    let keypair = KeyPair::generate_kyber()?;
    let pubkey = keypair.public_key();

    info!("Generated Kyber key pair successfully");
    info!("Public key: {}", pubkey);

    // Test encryption and decryption
    let test_messages = [b"Short message".to_vec(),
        b"Medium length message for testing".to_vec(),
        b"A longer message that tests the encryption capabilities of the Kyber post-quantum cryptography system".to_vec()];

    for (i, message) in test_messages.iter().enumerate() {
        info!("Test {}: Encrypting {} bytes", i + 1, message.len());

        // Encrypt the message
        let encrypted = encrypt(&pubkey, message)?;
        info!("  Encrypted length: {} bytes", encrypted.len());
        info!("  Overhead: {} bytes", encrypted.len() - message.len());

        // Decrypt the message
        let decrypted = decrypt(&keypair, &encrypted)?;

        // Verify decryption
        if decrypted == *message {
            info!("  ✅ Decryption successful");
        } else {
            info!("  ❌ Decryption failed");
            return Err("Decryption failed".into());
        }
    }

    info!("✅ All Kyber encryption/decryption tests passed!");

    // Test that we can generate multiple keys
    let keypair2 = KeyPair::generate_kyber()?;
    let pubkey2 = keypair2.public_key();
    info!("Generated second Kyber key pair: {}", pubkey2);

    // Verify keys are different
    if pubkey != pubkey2 {
        info!("✅ Generated keys are different (as expected)");
    } else {
        warn!("❌ Generated keys are identical (unexpected)");
    }

    Ok(())
}

async fn demo_hybrid_keypairs() -> Result<(), Box<dyn std::error::Error>> {
    info!("Generating hybrid key pair (Ed25519 + Kyber)...");

    let (ed25519_keypair, kyber_keypair) = generate_hybrid_keypair()?;

    let ed25519_pubkey = ed25519_keypair.public_key();
    let kyber_pubkey = kyber_keypair.public_key();

    info!("Ed25519 Public Key: {}", ed25519_pubkey);
    info!("Kyber Public Key: {}", kyber_pubkey);

    // Test Ed25519 signing
    let message = b"Hybrid key pair test message";
    let signature = ed25519_keypair.sign(message)?;
    let is_valid = KeyPair::verify(&ed25519_pubkey, message, &signature)?;
    info!(
        "Ed25519 signature verification: {}",
        if is_valid {
            "✅ SUCCESS"
        } else {
            "❌ FAILED"
        }
    );

    // Test Kyber encryption
    let encrypted = encrypt(&kyber_pubkey, message)?;
    let decrypted = decrypt(&kyber_keypair, &encrypted)?;
    let encryption_success = decrypted == message;
    info!(
        "Kyber encryption/decryption: {}",
        if encryption_success {
            "✅ SUCCESS"
        } else {
            "❌ FAILED"
        }
    );

    info!("Hybrid key pair provides both signing (Ed25519) and encryption (Kyber) capabilities!");

    Ok(())
}

async fn demo_performance_comparison() -> Result<(), Box<dyn std::error::Error>> {
    info!("Performance comparison between Ed25519 and Kyber...");

    let iterations = 10;
    let test_data = b"Performance test message";

    // Ed25519 performance test
    let ed25519_start = std::time::Instant::now();
    let ed25519_keypair = KeyPair::generate_ed25519()?;
    let ed25519_keygen_time = ed25519_start.elapsed();

    let ed25519_sign_start = std::time::Instant::now();
    for _ in 0..iterations {
        let _signature = ed25519_keypair.sign(test_data)?;
    }
    let ed25519_sign_time = ed25519_sign_start.elapsed();

    // Kyber performance test
    let kyber_start = std::time::Instant::now();
    let kyber_keypair = KeyPair::generate_kyber()?;
    let kyber_keygen_time = kyber_start.elapsed();

    let kyber_encrypt_start = std::time::Instant::now();
    for _ in 0..iterations {
        let encrypted = encrypt(&kyber_keypair.public_key(), test_data)?;
        let _decrypted = decrypt(&kyber_keypair, &encrypted)?;
    }
    let kyber_encrypt_time = kyber_encrypt_start.elapsed();

    info!("Ed25519 Key Generation: {:?}", ed25519_keygen_time);
    info!(
        "Ed25519 Signing ({} iterations): {:?}",
        iterations, ed25519_sign_time
    );
    info!(
        "Ed25519 Average signing time: {:?}",
        ed25519_sign_time / iterations
    );

    info!("Kyber Key Generation: {:?}", kyber_keygen_time);
    info!(
        "Kyber Encryption/Decryption ({} iterations): {:?}",
        iterations, kyber_encrypt_time
    );
    info!(
        "Kyber Average encryption/decryption time: {:?}",
        kyber_encrypt_time / iterations
    );

    info!("Note: Kyber is post-quantum secure but may be slower than Ed25519");

    Ok(())
}

async fn demo_key_serialization() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing key serialization and deserialization...");

    // Test Ed25519 serialization
    let ed25519_keypair = KeyPair::generate_ed25519()?;
    let ed25519_pubkey = ed25519_keypair.public_key();
    let ed25519_serialized = ed25519_pubkey.to_string();
    let ed25519_deserialized = PubKey::from_string(&ed25519_serialized)?;

    info!("Ed25519 Original: {}", ed25519_pubkey);
    info!("Ed25519 Serialized: {}", ed25519_serialized);
    info!("Ed25519 Deserialized: {}", ed25519_deserialized);
    info!(
        "Ed25519 Serialization: {}",
        if ed25519_pubkey == ed25519_deserialized {
            "✅ SUCCESS"
        } else {
            "❌ FAILED"
        }
    );

    // Test Kyber serialization
    let kyber_keypair = KeyPair::generate_kyber()?;
    let kyber_pubkey = kyber_keypair.public_key();
    let kyber_serialized = kyber_pubkey.to_string();
    let kyber_deserialized = PubKey::from_string(&kyber_serialized)?;

    info!("Kyber Original: {}", kyber_pubkey);
    info!("Kyber Serialized: {}", kyber_serialized);
    info!("Kyber Deserialized: {}", kyber_deserialized);
    info!(
        "Kyber Serialization: {}",
        if kyber_pubkey == kyber_deserialized {
            "✅ SUCCESS"
        } else {
            "❌ FAILED"
        }
    );

    // Test nil key
    let nil_key = PubKey::nil();
    let nil_serialized = nil_key.to_string();
    let nil_deserialized = PubKey::from_string(&nil_serialized)?;
    info!(
        "Nil key serialization: {}",
        if nil_key == nil_deserialized {
            "✅ SUCCESS"
        } else {
            "❌ FAILED"
        }
    );

    Ok(())
}
