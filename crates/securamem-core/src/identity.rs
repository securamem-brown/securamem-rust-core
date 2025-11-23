//! Identity Management - Persistent cryptographic identity for audit attribution

use crate::{Result, SecuraMemError};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};

/// Identity manager for persistent node identity
pub struct IdentityManager {
    signing_key: SigningKey,
    key_id: String,
    actor_name: String,
}

impl IdentityManager {
    /// Load or generate identity from the specified directory
    pub fn init(keys_dir: &Path) -> Result<Self> {
        let private_key_path = keys_dir.join("private.pem");

        // Ensure directory exists
        std::fs::create_dir_all(keys_dir)
            .map_err(SecuraMemError::Io)?;

        let signing_key = if private_key_path.exists() {
            // Load existing key
            tracing::info!("Loading existing identity from {:?}", private_key_path);
            Self::load_private_key(&private_key_path)?
        } else {
            // Generate new key
            tracing::info!("Generating new identity at {:?}", private_key_path);
            let key = SigningKey::generate(&mut OsRng);
            Self::save_private_key(&private_key_path, &key)?;
            key
        };

        // Compute key ID (fingerprint)
        let verifying_key = signing_key.verifying_key();
        let key_id = Self::compute_key_id(&verifying_key);

        // Get OS user for actor attribution
        let os_user = whoami::username();
        let actor_name = format!("{}@{}", os_user, key_id[..8].to_string());

        tracing::info!("Identity initialized: {}", actor_name);

        Ok(Self {
            signing_key,
            key_id,
            actor_name,
        })
    }

    /// Get the actor name for audit attribution
    pub fn actor(&self) -> &str {
        &self.actor_name
    }

    /// Get the key ID (fingerprint)
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Get the signing key
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    /// Get the verifying (public) key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Export public key as PEM
    pub fn public_key_pem(&self) -> Result<String> {
        use ed25519_dalek::pkcs8::EncodePublicKey;

        self.verifying_key()
            .to_public_key_pem(pkcs8::LineEnding::LF)
            .map_err(|e| SecuraMemError::Internal(e.to_string()))
    }

    /// Load private key from PEM file
    fn load_private_key(path: &Path) -> Result<SigningKey> {
        use ed25519_dalek::pkcs8::DecodePrivateKey;

        let pem_contents = std::fs::read_to_string(path)
            .map_err(SecuraMemError::Io)?;

        SigningKey::from_pkcs8_pem(&pem_contents)
            .map_err(|e| SecuraMemError::KeyGenerationFailed(e.to_string()))
    }

    /// Save private key as PEM file
    fn save_private_key(path: &Path, key: &SigningKey) -> Result<()> {
        use ed25519_dalek::pkcs8::EncodePrivateKey;

        let pem = key
            .to_pkcs8_pem(pkcs8::LineEnding::LF)
            .map_err(|e| SecuraMemError::Internal(e.to_string()))?;

        std::fs::write(path, pem.as_bytes())
            .map_err(SecuraMemError::Io)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)
                .map_err(SecuraMemError::Io)?
                .permissions();
            perms.set_mode(0o600); // rw-------
            std::fs::set_permissions(path, perms)
                .map_err(SecuraMemError::Io)?;
        }

        Ok(())
    }

    /// Compute key ID (SHA-256 fingerprint of public key)
    fn compute_key_id(verifying_key: &VerifyingKey) -> String {
        let mut hasher = Sha256::new();
        hasher.update(verifying_key.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_identity_persistence() {
        let temp_dir = std::env::temp_dir().join("smrust_test_identity");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // First initialization - generates new key
        let identity1 = IdentityManager::init(&temp_dir).unwrap();
        let key_id1 = identity1.key_id().to_string();

        // Second initialization - loads existing key
        let identity2 = IdentityManager::init(&temp_dir).unwrap();
        let key_id2 = identity2.key_id().to_string();

        // Should be the same identity
        assert_eq!(key_id1, key_id2);

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
