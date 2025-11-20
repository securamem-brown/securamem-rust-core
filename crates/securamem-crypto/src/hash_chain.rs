//! Hash chain utilities for immutable audit log

use securamem_core::Result;
use crate::{sha256_hex, sha256_bytes};

/// Compute hash chain link: SHA256(prev_hash || current_data)
///
/// This is the core of the immutable ledger. Each entry's hash depends on
/// the previous entry's hash, creating a tamper-evident chain.
pub fn compute_hash_chain_link(prev_hash: Option<&str>, current_data: &[u8]) -> Result<String> {
    let mut input = Vec::new();

    // Prepend previous hash (or empty for genesis)
    if let Some(prev) = prev_hash {
        input.extend_from_slice(prev.as_bytes());
    }

    // Append current entry data
    input.extend_from_slice(current_data);

    // Compute SHA-256 hash
    Ok(sha256_hex(&input))
}

/// Compute hash chain link returning raw bytes
pub fn compute_hash_chain_link_bytes(
    prev_hash: Option<&[u8]>,
    current_data: &[u8],
) -> [u8; 32] {
    let mut input = Vec::new();

    if let Some(prev) = prev_hash {
        input.extend_from_slice(prev);
    }

    input.extend_from_slice(current_data);

    sha256_bytes(&input)
}

/// Verify a chain of hashes
///
/// Given a list of (data, hash) pairs, verify that each hash correctly
/// chains to the next.
pub fn verify_hash_chain(entries: &[(Vec<u8>, String)]) -> Result<bool> {
    let mut expected_prev_hash: Option<String> = None;

    for (data, claimed_hash) in entries {
        let computed_hash = compute_hash_chain_link(expected_prev_hash.as_deref(), data)?;

        if &computed_hash != claimed_hash {
            return Ok(false);
        }

        expected_prev_hash = Some(claimed_hash.clone());
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_hash() {
        let data = b"genesis entry";
        let hash = compute_hash_chain_link(None, data).unwrap();
        assert_eq!(hash.len(), 64); // SHA-256 = 64 hex chars
    }

    #[test]
    fn test_chained_hash() {
        let data1 = b"first entry";
        let hash1 = compute_hash_chain_link(None, data1).unwrap();

        let data2 = b"second entry";
        let hash2 = compute_hash_chain_link(Some(&hash1), data2).unwrap();

        // Hash should be different
        assert_ne!(hash1, hash2);

        // Verify chain
        let entries = vec![
            (data1.to_vec(), hash1.clone()),
            (data2.to_vec(), hash2.clone()),
        ];
        assert!(verify_hash_chain(&entries).unwrap());
    }

    #[test]
    fn test_broken_chain() {
        let data1 = b"first entry";
        let hash1 = compute_hash_chain_link(None, data1).unwrap();

        let data2 = b"second entry";
        let hash2 = compute_hash_chain_link(Some(&hash1), data2).unwrap();

        // Tamper with data
        let tampered_data2 = b"tampered entry";

        let entries = vec![
            (data1.to_vec(), hash1),
            (tampered_data2.to_vec(), hash2), // Hash doesn't match tampered data
        ];

        assert!(!verify_hash_chain(&entries).unwrap());
    }

    #[test]
    fn test_hash_determinism() {
        let data = b"deterministic test";
        let hash1 = compute_hash_chain_link(None, data).unwrap();
        let hash2 = compute_hash_chain_link(None, data).unwrap();

        assert_eq!(hash1, hash2);
    }
}
