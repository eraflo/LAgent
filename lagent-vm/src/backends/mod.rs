// SPDX-License-Identifier: Apache-2.0
//! Inference backend trait and built-in implementations.

pub mod simulated;

use anyhow::Result;

/// Trait implemented by all inference backends.
pub trait InferenceBackend: Send + Sync {
    /// Generate a completion for the given prompt.
    fn infer(&self, prompt: &str) -> Result<String>;

    /// Classify the prompt against a set of labels and return (label, confidence) pairs.
    fn classify(&self, prompt: &str, labels: &[String]) -> Result<Vec<(String, f32)>>;
}

pub use simulated::SimulatedBackend;
