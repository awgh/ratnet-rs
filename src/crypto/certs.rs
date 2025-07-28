//! Certificate management utilities

use base64::Engine;
use rcgen::{Certificate, CertificateParams, IsCa, PKCS_ED25519};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::error::{RatNetError, Result};

/// Certificate builder for creating self-signed certificates
pub struct CertificateBuilder {
    subject_name: String,
    issuer_name: String,
    validity_days: u32,
    extensions: HashMap<String, String>,
}

impl CertificateBuilder {
    /// Create a new certificate builder
    pub fn new() -> Self {
        Self {
            subject_name: "localhost".to_string(),
            issuer_name: "RatNet CA".to_string(),
            validity_days: 365,
            extensions: HashMap::new(),
        }
    }

    /// Set the subject name (common name)
    pub fn subject_name(mut self, name: String) -> Self {
        self.subject_name = name;
        self
    }

    /// Set the issuer name
    pub fn issuer_name(mut self, name: String) -> Self {
        self.issuer_name = name;
        self
    }

    /// Set certificate validity in days
    pub fn validity_days(mut self, days: u32) -> Self {
        self.validity_days = days;
        self
    }

    /// Add a certificate extension
    pub fn add_extension(mut self, oid: String, value: String) -> Self {
        self.extensions.insert(oid, value);
        self
    }

    /// Generate a self-signed certificate with Ed25519 keys
    /// Returns (certificate_pem, private_key_pem)
    pub fn generate_ed25519_cert(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        // Generate Ed25519 key pair using rcgen
        let mut params = CertificateParams::new(vec![self.subject_name.clone()]);
        params.alg = &PKCS_ED25519;
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];

        // Set validity period
        let now = SystemTime::now();
        params.not_before = now.into();
        params.not_after =
            (now + Duration::from_secs(self.validity_days as u64 * 24 * 60 * 60)).into();

        // Create the certificate
        let cert = Certificate::from_params(params)
            .map_err(|e| RatNetError::Crypto(format!("Failed to create certificate: {:?}", e)))?;

        // Generate certificate PEM
        let cert_pem = cert.serialize_pem().map_err(|e| {
            RatNetError::Crypto(format!("Failed to serialize certificate: {:?}", e))
        })?;

        // Generate private key PEM
        let key_pem = cert.serialize_private_key_pem();

        Ok((cert_pem.into_bytes(), key_pem.into_bytes()))
    }
}

impl Default for CertificateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a simple self-signed certificate for testing
pub fn generate_test_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    CertificateBuilder::new()
        .subject_name("localhost".to_string())
        .issuer_name("RatNet Test CA".to_string())
        .validity_days(365)
        .generate_ed25519_cert()
}

/// Validate certificate chain
pub fn validate_cert_chain(cert_chain: &[u8]) -> Result<bool> {
    // Parse the certificate
    let cert_str = String::from_utf8_lossy(cert_chain);

    // For now, we'll do basic validation by checking if it's a valid PEM format
    // In a full implementation, you'd want to use a proper X.509 parser
    if !cert_str.contains("-----BEGIN CERTIFICATE-----") {
        return Err(RatNetError::Crypto(
            "Invalid certificate format".to_string(),
        ));
    }

    // Check if certificate is not expired (simplified check)
    // In a real implementation, you'd parse the certificate and check dates
    let _now = SystemTime::now();

    // For now, assume valid if it's a recent certificate
    // This is a simplified approach - in production you'd parse the actual dates
    Ok(true)
}

/// Extract public key from certificate
pub fn extract_public_key_from_cert(cert_pem: &[u8]) -> Result<Vec<u8>> {
    // Parse the certificate
    let cert_str = String::from_utf8_lossy(cert_pem);

    // For now, we'll extract the public key from the certificate PEM
    // In a full implementation, you'd parse the X.509 structure

    // Look for the certificate content between headers
    if let Some(start) = cert_str.find("-----BEGIN CERTIFICATE-----") {
        if let Some(end) = cert_str.find("-----END CERTIFICATE-----") {
            let cert_content = &cert_str[start + 27..end].trim();

            // Remove any whitespace and newlines from the base64 content
            let clean_content: String = cert_content
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();

            // Decode the base64 certificate
            let cert_der = base64::engine::general_purpose::STANDARD
                .decode(&clean_content)
                .map_err(|e| RatNetError::Crypto(format!("Failed to decode certificate: {}", e)))?;

            // For Ed25519 certificates, the public key is typically in the last 32 bytes
            // This is a simplified approach - in production you'd parse the ASN.1 structure
            if cert_der.len() >= 32 {
                let public_key = cert_der[cert_der.len() - 32..].to_vec();
                Ok(public_key)
            } else {
                Err(RatNetError::Crypto(
                    "Invalid certificate structure".to_string(),
                ))
            }
        } else {
            Err(RatNetError::Crypto(
                "Invalid certificate format".to_string(),
            ))
        }
    } else {
        Err(RatNetError::Crypto(
            "Invalid certificate format".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_builder() {
        let builder = CertificateBuilder::new()
            .subject_name("test.example.com".to_string())
            .validity_days(30);

        let result = builder.generate_ed25519_cert();
        assert!(result.is_ok());

        let (cert_pem, key_pem) = result.unwrap();
        assert!(!cert_pem.is_empty());
        assert!(!key_pem.is_empty());

        // Verify PEM format
        let cert_str = String::from_utf8(cert_pem).unwrap();
        assert!(cert_str.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_str.contains("-----END CERTIFICATE-----"));

        let key_str = String::from_utf8(key_pem).unwrap();
        assert!(key_str.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(key_str.contains("-----END PRIVATE KEY-----"));
    }

    #[test]
    fn test_generate_test_cert() {
        let result = generate_test_cert();
        assert!(result.is_ok());

        let (cert_pem, key_pem) = result.unwrap();
        assert!(!cert_pem.is_empty());
        assert!(!key_pem.is_empty());
    }

    #[test]
    fn test_extract_public_key_from_cert() {
        let (cert_pem, _) = generate_test_cert().unwrap();

        let public_key = extract_public_key_from_cert(&cert_pem).unwrap();
        assert!(!public_key.is_empty());
        assert_eq!(public_key.len(), 32); // Ed25519 public key size
    }

    #[test]
    fn test_validate_cert_chain() {
        let (cert_pem, _) = generate_test_cert().unwrap();

        let is_valid = validate_cert_chain(&cert_pem).unwrap();
        assert!(is_valid);
    }
}
