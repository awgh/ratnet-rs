//! Cryptographic types and utilities for RatNet

use base64::Engine;
use pqcrypto_kyber::kyber1024::{
    decapsulate, encapsulate, keypair, Ciphertext as KyberCiphertext, PublicKey as KyberPublicKey,
    SecretKey as KyberSecretKey,
};
use pqcrypto_traits::kem::{
    Ciphertext, PublicKey as KemPublicKey, SecretKey as KemSecretKey, SharedSecret,
};
use ring::signature::{Ed25519KeyPair, KeyPair as RingKeyPair};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{RatNetError, Result};

/// Public key types supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PubKey {
    /// Ed25519 public key (for signatures and encryption)
    Ed25519(Vec<u8>),
    /// Kyber post-quantum public key (for key encapsulation)
    Kyber(Vec<u8>),
    /// Nil/empty key
    Nil,
}

/// Private key types supported by the system
pub enum KeyPair {
    /// Ed25519 key pair with PKCS8 bytes for serialization
    Ed25519 {
        key_pair: Ed25519KeyPair,
        pkcs8_bytes: Vec<u8>,
    },
    /// Kyber post-quantum key pair (stores both public and secret keys)
    Kyber {
        public_key: KyberPublicKey,
        secret_key: KyberSecretKey,
    },
}

// Manual Debug implementation for KeyPair since KyberSecretKey doesn't implement Debug
impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyPair::Ed25519 { .. } => write!(f, "KeyPair::Ed25519({{...}})"),
            KeyPair::Kyber { .. } => write!(f, "KeyPair::Kyber({{...}})"),
        }
    }
}

impl PubKey {
    /// Create a nil/empty public key
    pub fn nil() -> Self {
        PubKey::Nil
    }

    /// Check if this is a nil key
    pub fn is_nil(&self) -> bool {
        matches!(self, PubKey::Nil)
    }

    /// Get the raw bytes of the public key
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            PubKey::Ed25519(bytes) => bytes,
            PubKey::Kyber(bytes) => bytes,
            PubKey::Nil => &[],
        }
    }

    /// Create Ed25519 public key from bytes
    pub fn ed25519(bytes: Vec<u8>) -> Self {
        PubKey::Ed25519(bytes)
    }

    /// Create Kyber public key from bytes
    pub fn kyber(bytes: Vec<u8>) -> Self {
        PubKey::Kyber(bytes)
    }

    /// Parse a public key from a string representation
    pub fn from_string(s: &str) -> Result<Self> {
        if s.is_empty() || s == "nil" {
            return Ok(PubKey::Nil);
        }

        if s.starts_with("ed25519:") {
            let encoded = &s[8..];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Ed25519 key encoding: {}", e)))?;
            Ok(PubKey::Ed25519(bytes))
        } else if s.starts_with("kyber:") {
            let encoded = &s[6..];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Kyber key encoding: {}", e)))?;
            Ok(PubKey::Kyber(bytes))
        } else {
            // Try to parse as base64 Ed25519 key (backward compatibility)
            match base64::engine::general_purpose::STANDARD.decode(s) {
                Ok(bytes) => Ok(PubKey::Ed25519(bytes)),
                Err(_) => Err(RatNetError::Crypto("Invalid public key format".to_string())),
            }
        }
    }
}

impl fmt::Display for PubKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PubKey::Ed25519(bytes) => write!(
                f,
                "ed25519:{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
            PubKey::Kyber(bytes) => write!(
                f,
                "kyber:{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
            PubKey::Nil => write!(f, "nil"),
        }
    }
}

impl KeyPair {
    /// Generate a new Ed25519 key pair
    pub fn generate_ed25519() -> Result<Self> {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8_doc = Ed25519KeyPair::generate_pkcs8(&rng)
            .map_err(|e| RatNetError::Crypto(format!("Failed to generate Ed25519 key: {}", e)))?;

        let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_doc.as_ref())
            .map_err(|e| RatNetError::Crypto(format!("Failed to parse Ed25519 key: {}", e)))?;

        Ok(KeyPair::Ed25519 {
            key_pair,
            pkcs8_bytes: pkcs8_doc.as_ref().to_vec(),
        })
    }

    /// Generate a new Kyber key pair
    pub fn generate_kyber() -> Result<Self> {
        let (public_key, secret_key) = keypair();

        Ok(KeyPair::Kyber {
            public_key,
            secret_key,
        })
    }

    /// Get the public key from this key pair
    pub fn public_key(&self) -> PubKey {
        match self {
            KeyPair::Ed25519 { key_pair, .. } => {
                let public_key_bytes = key_pair.public_key().as_ref().to_vec();
                PubKey::Ed25519(public_key_bytes)
            }
            KeyPair::Kyber { public_key, .. } => {
                // Extract public key bytes from the Kyber public key
                let public_key_bytes = public_key.as_bytes().to_vec();
                PubKey::Kyber(public_key_bytes)
            }
        }
    }

    /// Sign data with this key pair
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            KeyPair::Ed25519 { key_pair, .. } => {
                let signature = key_pair.sign(data);
                Ok(signature.as_ref().to_vec())
            }
            KeyPair::Kyber { .. } => Err(RatNetError::Crypto(
                "Kyber keys cannot be used for signing".to_string(),
            )),
        }
    }

    /// Verify a signature
    pub fn verify(pubkey: &PubKey, data: &[u8], signature: &[u8]) -> Result<bool> {
        match pubkey {
            PubKey::Ed25519(public_key_bytes) => {
                let public_key = ring::signature::UnparsedPublicKey::new(
                    &ring::signature::ED25519,
                    public_key_bytes,
                );

                match public_key.verify(data, signature) {
                    Ok(()) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            PubKey::Kyber(_) => Err(RatNetError::Crypto(
                "Kyber keys cannot be used for signature verification".to_string(),
            )),
            PubKey::Nil => Ok(false),
        }
    }

    /// Parse a key pair from a string representation
    pub fn from_string(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(RatNetError::Crypto("Empty key string".to_string()));
        }

        if s.starts_with("ed25519:") {
            let encoded = &s[8..];
            let pkcs8_bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Ed25519 key encoding: {}", e)))?;

            let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
                .map_err(|e| RatNetError::Crypto(format!("Invalid Ed25519 key format: {}", e)))?;

            Ok(KeyPair::Ed25519 {
                key_pair,
                pkcs8_bytes,
            })
        } else if s.starts_with("kyber:") {
            let encoded = &s[6..];
            let key_data = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Kyber key encoding: {}", e)))?;

            // Kyber key data should contain both public and secret keys
            // Format: [public_key_bytes][secret_key_bytes]
            if key_data.len() < 1568 + 3168 {
                // Kyber1024 public + secret key sizes
                return Err(RatNetError::Crypto(
                    "Invalid Kyber key data length".to_string(),
                ));
            }

            let public_key_bytes = &key_data[..1568];
            let secret_key_bytes = &key_data[1568..1568 + 3168];

            let public_key = KyberPublicKey::from_bytes(public_key_bytes)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Kyber public key: {}", e)))?;
            let secret_key = KyberSecretKey::from_bytes(secret_key_bytes)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Kyber secret key: {}", e)))?;

            Ok(KeyPair::Kyber {
                public_key,
                secret_key,
            })
        } else {
            // Try to parse as base64 Ed25519 PKCS8 key (backward compatibility)
            match base64::engine::general_purpose::STANDARD.decode(s) {
                Ok(pkcs8_bytes) => {
                    let key_pair =
                        Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).map_err(|e| {
                            RatNetError::Crypto(format!("Invalid Ed25519 key format: {}", e))
                        })?;
                    Ok(KeyPair::Ed25519 {
                        key_pair,
                        pkcs8_bytes,
                    })
                }
                Err(_) => Err(RatNetError::Crypto("Invalid key format".to_string())),
            }
        }
    }

    /// Convert this key pair to a string representation
    pub fn to_string(&self) -> Result<String> {
        match self {
            KeyPair::Ed25519 { pkcs8_bytes, .. } => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(pkcs8_bytes);
                Ok(format!("ed25519:{}", encoded))
            }
            KeyPair::Kyber {
                public_key,
                secret_key,
            } => {
                // Combine public and secret key bytes
                let mut key_data = Vec::new();
                key_data.extend_from_slice(public_key.as_bytes());
                key_data.extend_from_slice(secret_key.as_bytes());

                let encoded = base64::engine::general_purpose::STANDARD.encode(&key_data);
                Ok(format!("kyber:{}", encoded))
            }
        }
    }
}

/// Encrypt data using a public key
pub fn encrypt(pubkey: &PubKey, data: &[u8]) -> Result<Vec<u8>> {
    match pubkey {
        PubKey::Ed25519(_) => {
            // For Ed25519, we'll use a hybrid approach with a symmetric key
            // This is a simplified implementation - in practice you'd want to use
            // a proper hybrid encryption scheme
            Err(RatNetError::Crypto(
                "Ed25519 encryption not implemented".to_string(),
            ))
        }
        PubKey::Kyber(public_key_bytes) => {
            // Reconstruct the Kyber public key from bytes
            let public_key = KyberPublicKey::from_bytes(public_key_bytes)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Kyber public key: {}", e)))?;

            // Generate a random symmetric key for AES encryption
            let symmetric_key = random_bytes(32)?; // 256-bit key

            // Encrypt the data with AES-256-GCM
            let encrypted_data = encrypt_aes_gcm(&symmetric_key, data)?;

            // Use Kyber to encapsulate the symmetric key
            let (shared_secret, ciphertext) = encapsulate(&public_key);

            // XOR the symmetric key with the shared secret to create the final key
            let final_key: Vec<u8> = symmetric_key
                .iter()
                .zip(shared_secret.as_bytes().iter())
                .map(|(a, b)| a ^ b)
                .collect();

            // Combine the Kyber ciphertext and encrypted data
            let mut result = Vec::new();
            result.extend_from_slice(ciphertext.as_bytes());
            result.extend_from_slice(&final_key);
            result.extend_from_slice(&encrypted_data);

            Ok(result)
        }
        PubKey::Nil => Err(RatNetError::Crypto(
            "Cannot encrypt with nil key".to_string(),
        )),
    }
}

/// Decrypt data using a private key
pub fn decrypt(keypair: &KeyPair, data: &[u8]) -> Result<Vec<u8>> {
    match keypair {
        KeyPair::Ed25519 { .. } => Err(RatNetError::Crypto(
            "Ed25519 decryption not implemented".to_string(),
        )),
        KeyPair::Kyber { secret_key, .. } => {
            // Check minimum data size (ciphertext + key + encrypted data)
            if data.len() < 1568 + 32 + 16 {
                // Kyber1024 ciphertext + 32-byte key + minimum AES-GCM overhead
                return Err(RatNetError::Crypto(
                    "Invalid encrypted data size".to_string(),
                ));
            }

            let ciphertext_bytes = &data[..1568]; // Fixed from 1088 to 1568
            let ciphertext = KyberCiphertext::from_bytes(ciphertext_bytes)
                .map_err(|e| RatNetError::Crypto(format!("Invalid Kyber ciphertext: {}", e)))?;

            let shared_secret = decapsulate(&ciphertext, secret_key);

            // Extract the XORed symmetric key
            let xor_key = &data[1568..1568 + 32];
            let symmetric_key: Vec<u8> = xor_key
                .iter()
                .zip(shared_secret.as_bytes().iter())
                .map(|(a, b)| a ^ b)
                .collect();

            // Decrypt the AES-GCM data
            let encrypted_data = &data[1568 + 32..];
            decrypt_aes_gcm(&symmetric_key, encrypted_data)
        }
    }
}

/// Encrypt data using AES-256-GCM
fn encrypt_aes_gcm(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    use ring::aead::NonceSequence;
    use ring::aead::{Aad, BoundKey, Nonce, SealingKey, AES_256_GCM};
    use std::sync::atomic::{AtomicU64, Ordering};

    // Simple nonce sequence that increments
    struct IncrementingNonce {
        counter: AtomicU64,
    }

    impl NonceSequence for IncrementingNonce {
        fn advance(&mut self) -> std::result::Result<Nonce, ring::error::Unspecified> {
            let counter = self.counter.fetch_add(1, Ordering::Relaxed);
            let mut nonce_bytes = [0u8; 12];
            nonce_bytes[4..].copy_from_slice(&counter.to_le_bytes());
            Ok(Nonce::assume_unique_for_key(nonce_bytes))
        }
    }

    // Create the sealing key
    let unbound_key = ring::aead::UnboundKey::new(&AES_256_GCM, key)
        .map_err(|e| RatNetError::Crypto(format!("Invalid AES key: {:?}", e)))?;
    let mut sealing_key = SealingKey::new(
        unbound_key,
        IncrementingNonce {
            counter: AtomicU64::new(0),
        },
    );

    // Encrypt the data
    let mut encrypted_data = data.to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut encrypted_data)
        .map_err(|e| RatNetError::Crypto(format!("AES encryption failed: {:?}", e)))?;

    Ok(encrypted_data)
}

/// Decrypt data using AES-256-GCM
fn decrypt_aes_gcm(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    use ring::aead::NonceSequence;
    use ring::aead::{Aad, BoundKey, Nonce, OpeningKey, AES_256_GCM};
    use std::sync::atomic::{AtomicU64, Ordering};

    if data.len() < 16 {
        // minimum tag size
        return Err(RatNetError::Crypto("Invalid AES-GCM data size".to_string()));
    }

    // Simple nonce sequence that increments
    struct IncrementingNonce {
        counter: AtomicU64,
    }

    impl NonceSequence for IncrementingNonce {
        fn advance(&mut self) -> std::result::Result<Nonce, ring::error::Unspecified> {
            let counter = self.counter.fetch_add(1, Ordering::Relaxed);
            let mut nonce_bytes = [0u8; 12];
            nonce_bytes[4..].copy_from_slice(&counter.to_le_bytes());
            Ok(Nonce::assume_unique_for_key(nonce_bytes))
        }
    }

    // Create the opening key
    let unbound_key = ring::aead::UnboundKey::new(&AES_256_GCM, key)
        .map_err(|e| RatNetError::Crypto(format!("Invalid AES key: {:?}", e)))?;
    let mut opening_key = OpeningKey::new(
        unbound_key,
        IncrementingNonce {
            counter: AtomicU64::new(0),
        },
    );

    // Decrypt the data
    let mut decrypted_data = data.to_vec();
    let decrypted_len = opening_key
        .open_in_place(Aad::empty(), &mut decrypted_data)
        .map_err(|e| RatNetError::Crypto(format!("AES decryption failed: {:?}", e)))?
        .len();

    decrypted_data.truncate(decrypted_len);
    Ok(decrypted_data)
}

/// Generate a hybrid key pair (Ed25519 for signatures + Kyber for encryption)
pub fn generate_hybrid_keypair() -> Result<(KeyPair, KeyPair)> {
    let ed25519_keypair = KeyPair::generate_ed25519()?;
    let kyber_keypair = KeyPair::generate_kyber()?;

    Ok((ed25519_keypair, kyber_keypair))
}

/// Hash data using SHA-256
pub fn hash_sha256(data: &[u8]) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA256, data)
        .as_ref()
        .to_vec()
}

/// Generate random bytes
pub fn random_bytes(len: usize) -> Result<Vec<u8>> {
    let rng = ring::rand::SystemRandom::new();
    let mut bytes = vec![0u8; len];
    ring::rand::SecureRandom::fill(&rng, &mut bytes)
        .map_err(|e| RatNetError::Crypto(format!("Random generation failed: {:?}", e)))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ed25519_key_generation() {
        let keypair = KeyPair::generate_ed25519().unwrap();
        let pubkey = keypair.public_key();

        assert!(!pubkey.is_nil());
        assert!(matches!(pubkey, PubKey::Ed25519(_)));
    }

    #[test]
    fn test_kyber_key_generation() {
        let keypair = KeyPair::generate_kyber().unwrap();
        let pubkey = keypair.public_key();

        assert!(!pubkey.is_nil());
        assert!(matches!(pubkey, PubKey::Kyber(_)));
    }

    #[test]
    fn test_hybrid_key_generation() {
        let (ed25519_keypair, kyber_keypair) = generate_hybrid_keypair().unwrap();

        let ed25519_pubkey = ed25519_keypair.public_key();
        let kyber_pubkey = kyber_keypair.public_key();

        assert!(matches!(ed25519_pubkey, PubKey::Ed25519(_)));
        assert!(matches!(kyber_pubkey, PubKey::Kyber(_)));
    }

    #[test]
    fn test_ed25519_signing_and_verification() {
        let keypair = KeyPair::generate_ed25519().unwrap();
        let pubkey = keypair.public_key();

        let data = b"Hello, world!";
        let signature = keypair.sign(data).unwrap();

        let is_valid = KeyPair::verify(&pubkey, data, &signature).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_pubkey_serialization() {
        let keypair = KeyPair::generate_ed25519().unwrap();
        let pubkey = keypair.public_key();

        let serialized = pubkey.to_string();
        let deserialized = PubKey::from_string(&serialized).unwrap();

        assert_eq!(pubkey, deserialized);
    }

    #[test]
    fn test_kyber_public_key_extraction() {
        let keypair = KeyPair::generate_kyber().unwrap();
        let pubkey = keypair.public_key();

        // Verify it's a Kyber public key
        assert!(matches!(pubkey, PubKey::Kyber(_)));

        // Verify it's not empty or placeholder
        let pubkey_bytes = pubkey.as_bytes();
        assert!(!pubkey_bytes.is_empty());
        assert_eq!(pubkey_bytes.len(), 1568); // Kyber1024 public key size
    }

    #[test]
    fn test_kyber_encryption_and_decryption() {
        // Generate a Kyber key pair
        let keypair = KeyPair::generate_kyber().unwrap();
        let pubkey = keypair.public_key();

        // Test data
        let test_data = b"Hello, Kyber encryption!";

        // Encrypt the data
        let encrypted = encrypt(&pubkey, test_data).unwrap();

        // Decrypt the data
        let decrypted = decrypt(&keypair, &encrypted).unwrap();

        // Verify the decrypted data matches the original
        assert_eq!(test_data, decrypted.as_slice());
    }

    #[test]
    fn test_keypair_serialization() {
        // Test Ed25519 key pair serialization
        let ed25519_keypair = KeyPair::generate_ed25519().unwrap();
        let ed25519_string = ed25519_keypair.to_string().unwrap();
        assert!(ed25519_string.starts_with("ed25519:"));

        let deserialized_ed25519 = KeyPair::from_string(&ed25519_string).unwrap();
        assert_eq!(
            ed25519_keypair.public_key(),
            deserialized_ed25519.public_key()
        );

        // Test Kyber key pair serialization
        let kyber_keypair = KeyPair::generate_kyber().unwrap();
        let kyber_string = kyber_keypair.to_string().unwrap();
        assert!(kyber_string.starts_with("kyber:"));

        let deserialized_kyber = KeyPair::from_string(&kyber_string).unwrap();
        assert_eq!(kyber_keypair.public_key(), deserialized_kyber.public_key());
    }

    #[test]
    fn test_keypair_serialization_error_handling() {
        // Test empty string
        assert!(KeyPair::from_string("").is_err());

        // Test invalid format
        assert!(KeyPair::from_string("invalid:format").is_err());

        // Test invalid base64
        assert!(KeyPair::from_string("ed25519:invalid-base64!").is_err());
        assert!(KeyPair::from_string("kyber:invalid-base64!").is_err());
    }
}
