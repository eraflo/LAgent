// SPDX-License-Identifier: Apache-2.0
//! Anthropic Claude inference backend (requires the `backend-remote` feature).

use super::InferenceBackend;
use anyhow::{anyhow, Context, Result};

/// Anthropic Claude backend. Requires a valid `WISPEE_API_KEY` and the
/// `backend-remote` Cargo feature to be enabled.
pub struct AnthropicBackend {
    api_key: String,
    model: String,
    temperature: f32,
    client: reqwest::blocking::Client,
}

impl AnthropicBackend {
    /// Create a new backend.
    ///
    /// * `api_key` — Anthropic API key (from `WISPEE_API_KEY`).
    /// * `deterministic` — when `true`, sets temperature to `0.0`.
    pub fn new(api_key: impl Into<String>, deterministic: bool) -> Self {
        Self {
            api_key: api_key.into(),
            model: "claude-sonnet-4-6".to_string(),
            temperature: if deterministic { 0.0 } else { 1.0 },
            client: reqwest::blocking::Client::new(),
        }
    }

    fn send_message(&self, prompt: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "temperature": self.temperature,
            "messages": [{"role": "user", "content": prompt}]
        });

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .context("failed to reach Anthropic API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(anyhow!("Anthropic API error {status}: {text}"));
        }

        let json: serde_json::Value = resp.json().context("invalid JSON from Anthropic API")?;
        json["content"][0]["text"]
            .as_str()
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("unexpected Anthropic response shape"))
    }
}

impl InferenceBackend for AnthropicBackend {
    fn infer(&self, prompt: &str) -> Result<String> {
        self.send_message(prompt)
    }

    fn classify(&self, prompt: &str, labels: &[String]) -> Result<Vec<(String, f32)>> {
        if labels.is_empty() {
            return Ok(vec![]);
        }
        let label_list = labels.join(", ");
        let p = format!(
            "Classify the following text. Reply with ONLY one of these labels: {label_list}\n\nText: {prompt}"
        );
        let response = self.send_message(&p)?;
        let winner = response.trim().to_string();
        // Give the winner weight 1.0, all others 0.0.
        Ok(labels
            .iter()
            .map(|l| {
                let w = if l.to_lowercase() == winner.to_lowercase() {
                    1.0_f32
                } else {
                    0.0_f32
                };
                (l.clone(), w)
            })
            .collect())
    }

    fn compress(&self, text: &str) -> Result<String> {
        let p = format!(
            "Summarise the following text in half the number of words. \
             Reply with ONLY the summary, no preamble.\n\n{text}"
        );
        self.send_message(&p)
    }

    fn act(&self, payload: &str) -> Result<String> {
        // Phase 4: log and echo; real action routing deferred to Phase 5.
        Ok(payload.to_string())
    }

    fn oracle(&self, name: &str, _args: &[String]) -> Result<String> {
        // Phase 4: return a placeholder; real RAG routing deferred to Phase 5.
        Ok(format!("<oracle:{name}>"))
    }
}
