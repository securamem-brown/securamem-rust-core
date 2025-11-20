//! Parity test - Compare Rust embeddings with Node.js reference implementation
//!
//! Run this test after generating Node.js reference embeddings with:
//!   node scripts/test-embedding-node.js

use securamem_firewall::SemanticEngine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingResult {
    text: String,
    embedding: Vec<f32>,
    dimensions: usize,
    norm: f64,
    first_10: Vec<f32>,
}

#[test]
fn test_embedding_parity_with_nodejs() {
    // Load Node.js reference results
    let reference_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("scripts/embedding-test-results-nodejs.json");

    if !reference_path.exists() {
        panic!(
            "Node.js reference file not found at {:?}\nRun: node scripts/test-embedding-node.js",
            reference_path
        );
    }

    let reference_json = std::fs::read_to_string(&reference_path)
        .expect("Failed to read Node.js reference file");
    let reference_results: Vec<EmbeddingResult> =
        serde_json::from_str(&reference_json).expect("Failed to parse Node.js results");

    // Initialize Rust engine
    let engine = SemanticEngine::new().expect("Failed to initialize Rust engine");

    println!("\n=== RUST vs NODE.JS EMBEDDING PARITY TEST ===\n");

    let mut all_passed = true;

    for (idx, reference) in reference_results.iter().enumerate() {
        println!("Test case {}: \"{}\"", idx + 1, reference.text);

        // Generate Rust embedding
        let rust_embedding = engine
            .embed(&reference.text)
            .expect("Failed to generate Rust embedding");

        // Check dimensions
        assert_eq!(
            rust_embedding.len(),
            reference.dimensions,
            "Dimension mismatch for '{}'",
            reference.text
        );
        println!("  ✓ Dimensions match: {}", rust_embedding.len());

        // Compare L2 norm (should be ~1.0 for both)
        let rust_norm: f32 = rust_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_diff = (rust_norm - reference.norm as f32).abs();
        assert!(
            norm_diff < 0.001,
            "L2 norm mismatch: Rust={:.6}, Node={:.6}",
            rust_norm,
            reference.norm
        );
        println!(
            "  ✓ L2 norm match: Rust={:.6}, Node={:.6}, diff={:.6}",
            rust_norm, reference.norm, norm_diff
        );

        // Compare first 10 values (critical for parity verification)
        println!("  Comparing first 10 embedding values:");
        let mut first_10_match = true;
        for i in 0..10.min(rust_embedding.len()) {
            let rust_val = rust_embedding[i];
            let node_val = reference.first_10[i];
            let diff = (rust_val - node_val).abs();

            // Allow small floating-point tolerance (1e-6)
            let matches = diff < 1e-6;

            if !matches {
                println!(
                    "    [{:2}] Rust: {:.8}, Node: {:.8}, diff: {:.8} ❌",
                    i, rust_val, node_val, diff
                );
                first_10_match = false;
            } else {
                println!(
                    "    [{:2}] Rust: {:.8}, Node: {:.8}, diff: {:.8} ✓",
                    i, rust_val, node_val, diff
                );
            }
        }

        if !first_10_match {
            all_passed = false;
            println!("  ❌ FAILED: First 10 values do not match within tolerance\n");
        } else {
            println!("  ✓ PASSED: First 10 values match within tolerance\n");
        }

        // Compare full embeddings with cosine similarity
        let dot_product: f32 = rust_embedding
            .iter()
            .zip(reference.embedding.iter())
            .map(|(a, b)| a * b)
            .sum();

        let cosine_similarity = dot_product; // Both are unit vectors, so dot product = cosine similarity
        println!(
            "  Cosine similarity: {:.8} (1.0 = identical)",
            cosine_similarity
        );

        assert!(
            cosine_similarity > 0.9999,
            "Cosine similarity too low: {}",
            cosine_similarity
        );
        println!("  ✓ PASSED: Embeddings are virtually identical\n");
    }

    if all_passed {
        println!("=== ✓ ALL TESTS PASSED - RUST/NODE.JS PARITY CONFIRMED ===\n");
    } else {
        panic!("Some parity tests failed!");
    }
}
