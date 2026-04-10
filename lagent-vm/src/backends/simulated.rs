use super::InferenceBackend;
use anyhow::Result;

/// Simulated backend for deterministic testing.
/// Returns predictable results without calling any model.
pub struct SimulatedBackend {
    fixed_response: String,
}

impl SimulatedBackend {
    pub fn new(fixed_response: impl Into<String>) -> Self {
        Self { fixed_response: fixed_response.into() }
    }
}

impl InferenceBackend for SimulatedBackend {
    fn infer(&self, _prompt: &str) -> Result<String> {
        Ok(self.fixed_response.clone())
    }

    fn classify(&self, _prompt: &str, labels: &[String]) -> Result<Vec<(String, f32)>> {
        // Return uniform distribution over labels
        let weight = if labels.is_empty() { 0.0 } else { 1.0 / labels.len() as f32 };
        Ok(labels.iter().map(|l| (l.clone(), weight)).collect())
    }
}
