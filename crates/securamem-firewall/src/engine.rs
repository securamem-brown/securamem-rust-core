//! Semantic Engine - ONNX-based vector embeddings with exact Node.js parity
//!
//! Model: all-MiniLM-L6-v2 (sentence-transformers)
//! Dimensions: 384
//! Tokenizer: BertTokenizer with WordPiece
//! Max length: 128 tokens
//!
//! Critical implementation details:
//! - Input tensors MUST be int64 (not int32)
//! - Mean pooling MUST skip padding tokens (attention_mask == 0)
//! - L2 normalization applied after mean pooling

use anyhow::{Context, Result};
use tokenizers::Tokenizer;
use tract_onnx::prelude::*;

/// Embedded ONNX model (all-MiniLM-L6-v2)
const MODEL_BYTES: &[u8] = include_bytes!("../../../.securamem/models/all-MiniLM-L6-v2/model.onnx");

/// Embedded tokenizer configuration
const TOKENIZER_BYTES: &[u8] = include_bytes!("../../../.securamem/models/all-MiniLM-L6-v2/tokenizer.json");

/// Special tokens (BERT vocabulary)
const CLS_TOKEN_ID: i64 = 101;
const SEP_TOKEN_ID: i64 = 102;
const PAD_TOKEN_ID: i64 = 0;

/// Maximum sequence length (must match tokenizer config)
const MAX_LENGTH: usize = 128;

/// Expected embedding dimensions (all-MiniLM-L6-v2)
const EMBEDDING_DIM: usize = 384;

pub struct SemanticEngine {
    model: SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>,
    tokenizer: Tokenizer,
}

impl SemanticEngine {
    /// Initialize the semantic engine with embedded model and tokenizer
    pub fn new() -> Result<Self> {
        tracing::info!("Loading embedded ONNX model (all-MiniLM-L6-v2)...");

        // Load ONNX model from embedded bytes
        let model = tract_onnx::onnx()
            .model_for_read(&mut &MODEL_BYTES[..])
            .context("Failed to load ONNX model")?
            .into_optimized()
            .context("Failed to optimize ONNX model")?
            .into_runnable()
            .context("Failed to create runnable model")?;

        tracing::info!("Loading embedded tokenizer...");

        // Load tokenizer from embedded bytes
        let tokenizer = Tokenizer::from_bytes(TOKENIZER_BYTES)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        tracing::info!("Semantic engine initialized (384D embeddings)");

        Ok(Self { model, tokenizer })
    }

    /// Generate embedding for text with exact Node.js implementation parity
    ///
    /// Steps (matching LocalOnnxProvider.ts):
    /// 1. Tokenize with BertTokenizer
    /// 2. Add [CLS] and [SEP] special tokens
    /// 3. Pad to MAX_LENGTH
    /// 4. Create THREE int64 tensors: input_ids, attention_mask, token_type_ids
    /// 5. Run ONNX inference
    /// 6. Mean pooling (skip padding tokens where attention_mask == 0)
    /// 7. L2 normalization
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Step 1: Tokenize
        let encoding = self.tokenizer
            .encode(text, false)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        // Step 2: Add special tokens [CLS] ... [SEP]
        let mut input_ids: Vec<i64> = Vec::with_capacity(MAX_LENGTH);
        input_ids.push(CLS_TOKEN_ID);
        input_ids.extend(encoding.get_ids().iter().map(|&id| id as i64));
        input_ids.push(SEP_TOKEN_ID);

        let actual_length = input_ids.len();

        // Step 3: Pad to MAX_LENGTH
        let mut attention_mask: Vec<i64> = vec![1; actual_length];

        while input_ids.len() < MAX_LENGTH {
            input_ids.push(PAD_TOKEN_ID);
            attention_mask.push(0); // Mark padding tokens
        }

        // Truncate if too long (shouldn't happen with max_length=128)
        input_ids.truncate(MAX_LENGTH);
        attention_mask.truncate(MAX_LENGTH);

        // Token type IDs (all zeros for single-sequence classification)
        let token_type_ids: Vec<i64> = vec![0; MAX_LENGTH];

        tracing::debug!(
            "Tokenized: {} tokens (actual: {}, padded: {})",
            text.chars().take(30).collect::<String>(),
            actual_length,
            MAX_LENGTH
        );

        // Step 4: Create THREE int64 tensors (CRITICAL: must be i64, not i32!)
        let input_ids_tensor = tract_ndarray::Array2::from_shape_vec(
            (1, MAX_LENGTH),
            input_ids.clone(),
        )
        .context("Failed to create input_ids tensor")?
        .into_dyn();

        let attention_mask_tensor = tract_ndarray::Array2::from_shape_vec(
            (1, MAX_LENGTH),
            attention_mask.clone(),
        )
        .context("Failed to create attention_mask tensor")?
        .into_dyn();

        let token_type_ids_tensor = tract_ndarray::Array2::from_shape_vec(
            (1, MAX_LENGTH),
            token_type_ids,
        )
        .context("Failed to create token_type_ids tensor")?
        .into_dyn();

        // Step 5: Run ONNX inference
        // Convert ndarrays to tract Tensors
        let outputs = self.model.run(tvec![
            Tensor::from(input_ids_tensor).into(),
            Tensor::from(attention_mask_tensor).into(),
            Tensor::from(token_type_ids_tensor).into(),
        ])?;

        // Extract last_hidden_state: shape [batch_size, seq_len, hidden_size]
        let last_hidden_state = outputs[0]
            .to_array_view::<f32>()
            .context("Failed to extract output tensor")?
            .into_dimensionality::<tract_ndarray::Ix3>()
            .context("Failed to convert to 3D array")?;

        // Step 6: Mean pooling (skip padding tokens)
        let embedding = self.mean_pooling(&last_hidden_state, &attention_mask)?;

        // Step 7: L2 normalization
        let normalized = self.l2_normalize(&embedding);

        Ok(normalized)
    }

    /// Mean pooling implementation (matches Node.js LocalOnnxProvider.ts lines 136-169)
    ///
    /// Key: Only average over tokens where attention_mask == 1 (skip padding)
    fn mean_pooling(&self, hidden_states: &tract_ndarray::ArrayView3<f32>, attention_mask: &[i64]) -> Result<Vec<f32>> {
        let seq_len = hidden_states.shape()[1];
        let hidden_size = hidden_states.shape()[2];

        if hidden_size != EMBEDDING_DIM {
            anyhow::bail!(
                "Model output dimension mismatch: expected {}, got {}",
                EMBEDDING_DIM,
                hidden_size
            );
        }

        // Initialize accumulator
        let mut embedding = vec![0.0f32; hidden_size];
        let mut valid_tokens = 0;

        // Sum embeddings for non-padding tokens
        for i in 0..seq_len {
            if attention_mask[i] == 1 {
                valid_tokens += 1;
                for j in 0..hidden_size {
                    embedding[j] += hidden_states[[0, i, j]];
                }
            }
        }

        // Average by valid token count (not total length!)
        if valid_tokens == 0 {
            anyhow::bail!("No valid tokens found for mean pooling");
        }

        for val in &mut embedding {
            *val /= valid_tokens as f32;
        }

        tracing::debug!(
            "Mean pooling: {} valid tokens (skipped {} padding tokens)",
            valid_tokens,
            seq_len - valid_tokens
        );

        Ok(embedding)
    }

    /// L2 normalization (convert to unit vector)
    ///
    /// Matches Node.js implementation: norm = sqrt(sum(x^2)), result = x / norm
    fn l2_normalize(&self, embedding: &[f32]) -> Vec<f32> {
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm == 0.0 {
            tracing::warn!("L2 norm is zero, returning original embedding");
            return embedding.to_vec();
        }

        embedding.iter().map(|x| x / norm).collect()
    }

    /// Compute cosine similarity between two embeddings
    ///
    /// Both embeddings should be L2-normalized (unit vectors)
    /// Result: [-1.0, 1.0] where 1.0 = identical, 0.0 = orthogonal, -1.0 = opposite
    pub fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            anyhow::bail!(
                "Embedding dimension mismatch: {} vs {}",
                a.len(),
                b.len()
            );
        }

        // For unit vectors, cosine similarity = dot product
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();

        Ok(dot_product)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_engine_initialization() {
        let engine = SemanticEngine::new().expect("Failed to initialize engine");
        assert!(true, "Engine loaded successfully");
    }

    #[test]
    fn test_embedding_generation() {
        let engine = SemanticEngine::new().expect("Failed to initialize engine");
        let embedding = engine.embed("Hello world").expect("Failed to generate embedding");

        assert_eq!(embedding.len(), EMBEDDING_DIM, "Embedding dimension mismatch");

        // Check L2 normalization (unit vector)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "Embedding is not L2 normalized");
    }

    #[test]
    fn test_cosine_similarity() {
        let engine = SemanticEngine::new().expect("Failed to initialize engine");

        let embedding1 = engine.embed("This is a test").expect("Failed to embed");
        let embedding2 = engine.embed("This is a test").expect("Failed to embed");
        let embedding3 = engine.embed("Completely different text").expect("Failed to embed");

        let similarity_same = engine
            .cosine_similarity(&embedding1, &embedding2)
            .expect("Failed to compute similarity");

        let similarity_different = engine
            .cosine_similarity(&embedding1, &embedding3)
            .expect("Failed to compute similarity");

        assert!(
            similarity_same > 0.99,
            "Identical text should have ~1.0 similarity, got {}",
            similarity_same
        );

        assert!(
            similarity_different < similarity_same,
            "Different text should have lower similarity"
        );
    }
}
