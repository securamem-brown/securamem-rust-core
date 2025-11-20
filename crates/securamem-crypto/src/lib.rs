//! SecuraMem Cryptography - ED25519 signing, SHA-256 hashing, RFC 3161 timestamping

use securamem_core::{Result, SecuraMemError};
use ed25519_dalek::{Signer as _, Verifier as _, SigningKey, VerifyingKey, Signature};
use rand_core::OsRng;
use ring::digest::{digest, SHA256};

pub mod receipt;
pub mod hash_chain;

#[cfg(feature = "tsa-client")]
pub mod rfc3161;

/// ED25519 signing key wrapper
pub struct SecuraMemSigningKey {
    key: SigningKey,
    key_id: String,
}

impl SecuraMemSigningKey {
    /// Generate a new random signing key
    pub fn generate() -> Self {
        let key = SigningKey::generate(&mut OsRng);
        let key_id = Self::compute_key_id(&key.verifying_key());

        Self { key, key_id }
    }

    /// Load from PEM file on disk
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let pem = std::fs::read_to_string(path)
            .map_err(|e| SecuraMemError::Io(e))?;
        Self::from_pkcs8_pem(&pem)
    }

    /// Save to PEM file on disk
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SecuraMemError::Io(e))?;
        }

        let pem = self.to_pkcs8_pem()?;
        std::fs::write(path, pem)
            .map_err(|e| SecuraMemError::Io(e))?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)
                .map_err(|e| SecuraMemError::Io(e))?
                .permissions();
            perms.set_mode(0o600); // rw-------
            std::fs::set_permissions(path, perms)
                .map_err(|e| SecuraMemError::Io(e))?;
        }

        Ok(())
    }

    /// Load from PEM-encoded PKCS#8 private key
    pub fn from_pkcs8_pem(pem: &str) -> Result<Self> {
        use ed25519_dalek::pkcs8::DecodePrivateKey;

        let key = SigningKey::from_pkcs8_pem(pem)
            .map_err(|e| SecuraMemError::KeyGenerationFailed(e.to_string()))?;

        let key_id = Self::compute_key_id(&key.verifying_key());

        Ok(Self { key, key_id })
    }

    /// Export to PEM-encoded PKCS#8 private key
    pub fn to_pkcs8_pem(&self) -> Result<String> {
        use ed25519_dalek::pkcs8::EncodePrivateKey;

        self.key
            .to_pkcs8_pem(pkcs8::LineEnding::LF)
            .map(|s| s.to_string())
            .map_err(|e| SecuraMemError::Internal(e.to_string()))
    }

    /// Get the verifying (public) key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.key.verifying_key()
    }

    /// Export public key as PEM
    pub fn verifying_key_pem(&self) -> Result<String> {
        use ed25519_dalek::pkcs8::EncodePublicKey;

        self.key
            .verifying_key()
            .to_public_key_pem(pkcs8::LineEnding::LF)
            .map_err(|e| SecuraMemError::Internal(e.to_string()))
    }

    /// Get the key ID (SHA-256 fingerprint of public key)
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Sign data and return base64-encoded signature
    pub fn sign(&self, data: &[u8]) -> String {
        use base64::Engine;
        let signature = self.key.sign(data);
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    }

    /// Sign data and return raw signature bytes
    pub fn sign_raw(&self, data: &[u8]) -> Signature {
        self.key.sign(data)
    }

    /// Compute key ID from public key (SHA-256 fingerprint)
    fn compute_key_id(public_key: &VerifyingKey) -> String {
        let public_key_bytes = public_key.to_bytes();
        let hash = digest(&SHA256, &public_key_bytes);
        format!("ed25519:sha256:{}", hex::encode(hash.as_ref()))
    }
}

/// Verify an ED25519 signature
pub fn verify_signature(
    public_key_pem: &str,
    data: &[u8],
    signature_base64: &str,
) -> Result<bool> {
    use ed25519_dalek::pkcs8::DecodePublicKey;

    // Decode public key
    let public_key = VerifyingKey::from_public_key_pem(public_key_pem)
        .map_err(|e| SecuraMemError::InvalidSignature(e.to_string()))?;

    // Decode signature
    use base64::Engine;
    let signature_bytes = base64::engine::general_purpose::STANDARD.decode(signature_base64)
        .map_err(|e| SecuraMemError::InvalidSignature(e.to_string()))?;

    let signature = Signature::from_bytes(
        signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| SecuraMemError::InvalidSignature("Invalid signature length".to_string()))?,
    );

    // Verify
    Ok(public_key.verify(data, &signature).is_ok())
}

/// Compute SHA-256 hash and return hex-encoded string
pub fn sha256_hex(data: &[u8]) -> String {
    let hash = digest(&SHA256, data);
    hex::encode(hash.as_ref())
}

/// Compute SHA-256 hash and return raw bytes
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let hash = digest(&SHA256, data);
    let mut result = [0u8; 32];
    result.copy_from_slice(hash.as_ref());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let key = SecuraMemSigningKey::generate();
        assert!(key.key_id().starts_with("ed25519:sha256:"));
    }

    #[test]
    fn test_sign_and_verify() {
        let key = SecuraMemSigningKey::generate();
        let data = b"Hello, SecuraMem!";

        let signature = key.sign(data);
        let public_key_pem = key.verifying_key_pem().unwrap();

        let valid = verify_signature(&public_key_pem, data, &signature).unwrap();
        assert!(valid);

        // Tampered data should fail
        let tampered = b"Hello, Tampered!";
        let invalid = verify_signature(&public_key_pem, tampered, &signature).unwrap();
        assert!(!invalid);
    }

    #[test]
    fn test_pem_roundtrip() {
        let key = SecuraMemSigningKey::generate();
        let pem = key.to_pkcs8_pem().unwrap();

        let key2 = SecuraMemSigningKey::from_pkcs8_pem(&pem).unwrap();
        assert_eq!(key.key_id(), key2.key_id());
    }

    #[test]
    fn test_sha256() {
        let data = b"test data";
        let hash = sha256_hex(data);
        assert_eq!(hash.len(), 64); // 32 bytes = 64 hex chars

        let hash_bytes = sha256_bytes(data);
        assert_eq!(hash_bytes.len(), 32);
        assert_eq!(hex::encode(hash_bytes), hash);
    }
}
