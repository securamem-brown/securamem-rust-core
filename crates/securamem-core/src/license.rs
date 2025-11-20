//! License Management - Node-locked hardware binding and JWT verification

use crate::{Result, SecuraMemError};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use sha2::{Sha256, Digest};
use std::path::Path;

/// VENDOR PUBLIC KEY (Ed25519)
/// This is the public key used to verify license signatures.
/// The corresponding private key is kept secure by the vendor.
///
/// PLACEHOLDER: Replace with actual vendor public key after generation
pub const VENDOR_PUBLIC_KEY: &str = "PLACEHOLDER_VENDOR_PUBLIC_KEY_PEM";

/// License claims structure (JWT payload)
#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseClaims {
    /// Subject: Machine ID (SHA-256 of hardware fingerprint)
    pub sub: String,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Company/Organization name
    pub company: String,
    /// License type (trial, standard, enterprise, etc.)
    pub license_type: String,
}

/// Verified license information
#[derive(Debug, Clone)]
pub struct LicenseInfo {
    pub machine_id: String,
    pub company: String,
    pub license_type: String,
    pub issued_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub days_remaining: i64,
}

/// Get the unique machine ID (hardware fingerprint)
///
/// Returns SHA-256 hash of the system's hardware UUID
pub fn get_machine_id() -> Result<String> {
    let machine_uuid = machine_uid::get()
        .map_err(|e| SecuraMemError::LicenseError(format!("Failed to get machine ID: {}", e)))?;

    // Hash the machine UUID for privacy and consistency
    let mut hasher = Sha256::new();
    hasher.update(machine_uuid.as_bytes());
    let hash = hasher.finalize();

    Ok(hex::encode(hash))
}

/// Verify a license file
pub fn verify_license(license_path: &Path) -> Result<LicenseInfo> {
    // Read license file
    let license_jwt = std::fs::read_to_string(license_path)
        .map_err(|e| SecuraMemError::LicenseError(format!("Failed to read license file: {}", e)))?;

    // For development: Allow bypass if placeholder key is still present
    if VENDOR_PUBLIC_KEY == "PLACEHOLDER_VENDOR_PUBLIC_KEY_PEM" {
        tracing::warn!("⚠️  VENDOR_PUBLIC_KEY is placeholder - license verification bypassed for development");
        return create_development_license();
    }

    // Decode and verify JWT signature
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_exp = true;

    let decoding_key = DecodingKey::from_ed_pem(VENDOR_PUBLIC_KEY.as_bytes())
        .map_err(|e| SecuraMemError::LicenseError(format!("Invalid vendor public key: {}", e)))?;

    let token_data = decode::<LicenseClaims>(&license_jwt.trim(), &decoding_key, &validation)
        .map_err(|e| SecuraMemError::LicenseError(format!("License verification failed: {}", e)))?;

    let claims = token_data.claims;

    // Check 1: Verify expiration
    let now = chrono::Utc::now().timestamp();
    if claims.exp < now {
        return Err(SecuraMemError::LicenseExpired {
            expired_at: chrono::DateTime::from_timestamp(claims.exp, 0)
                .unwrap_or_default()
                .to_rfc3339(),
        });
    }

    // Check 2: Verify machine ID (node-lock)
    let current_machine_id = get_machine_id()?;
    if claims.sub != current_machine_id {
        return Err(SecuraMemError::LicenseError(format!(
            "License is not valid for this machine. Expected: {}, Got: {}",
            claims.sub, current_machine_id
        )));
    }

    // Calculate days remaining
    let expires_at = chrono::DateTime::from_timestamp(claims.exp, 0)
        .ok_or_else(|| SecuraMemError::LicenseError("Invalid expiration timestamp".to_string()))?;
    let days_remaining = (expires_at - chrono::Utc::now()).num_days();

    Ok(LicenseInfo {
        machine_id: claims.sub,
        company: claims.company,
        license_type: claims.license_type,
        issued_at: chrono::DateTime::from_timestamp(claims.iat, 0)
            .unwrap_or_default(),
        expires_at,
        days_remaining,
    })
}

/// Create a development license (when vendor key is placeholder)
fn create_development_license() -> Result<LicenseInfo> {
    let machine_id = get_machine_id()?;
    Ok(LicenseInfo {
        machine_id,
        company: "Development".to_string(),
        license_type: "dev".to_string(),
        issued_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(365),
        days_remaining: 365,
    })
}

/// Generate a license JWT (vendor-side tool)
///
/// This should only be used by the vendor to create licenses for customers
pub fn generate_license(
    machine_id: &str,
    company: &str,
    license_type: &str,
    days_valid: i64,
    vendor_private_key_pem: &str,
) -> Result<String> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let now = chrono::Utc::now().timestamp();
    let exp = now + (days_valid * 86400); // days to seconds

    let claims = LicenseClaims {
        sub: machine_id.to_string(),
        iat: now,
        exp,
        company: company.to_string(),
        license_type: license_type.to_string(),
    };

    let header = Header::new(Algorithm::EdDSA);
    let encoding_key = EncodingKey::from_ed_pem(vendor_private_key_pem.as_bytes())
        .map_err(|e| SecuraMemError::LicenseError(format!("Invalid vendor private key: {}", e)))?;

    let token = encode(&header, &claims, &encoding_key)
        .map_err(|e| SecuraMemError::LicenseError(format!("Failed to generate license: {}", e)))?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_machine_id() {
        let machine_id = get_machine_id().unwrap();
        assert_eq!(machine_id.len(), 64); // SHA-256 = 64 hex chars

        // Should be deterministic (same machine = same ID)
        let machine_id2 = get_machine_id().unwrap();
        assert_eq!(machine_id, machine_id2);
    }
}
