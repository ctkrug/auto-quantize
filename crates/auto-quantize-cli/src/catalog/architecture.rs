//! Best-effort fetch of a HuggingFace repo's `config.json`, used to size an
//! exact KV-cache headroom for `--context` (docs/BACKLOG.md 1.6, 3.3).
//!
//! Unlike the GGUF catalog fetch, failure here is never fatal: not every
//! repo publishes a `config.json` with fields this tool recognizes, and the
//! decision engine has an honest fallback (the flat headroom fraction) for
//! exactly that case.

use std::time::Duration;

use auto_quantize_core::ModelArchitecture;
use serde::Deserialize;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// The subset of a HuggingFace `config.json` needed to size a KV cache.
/// Field names cover the common `transformers`-style config plus the
/// GPT-2-style aliases some older repos still use.
#[derive(Debug, Deserialize)]
struct RawConfig {
    num_hidden_layers: Option<u32>,
    n_layer: Option<u32>,
    hidden_size: Option<u32>,
    n_embd: Option<u32>,
}

impl RawConfig {
    fn into_architecture(self) -> Option<ModelArchitecture> {
        let num_layers = self.num_hidden_layers.or(self.n_layer)?;
        let hidden_size = self.hidden_size.or(self.n_embd)?;
        if num_layers == 0 || hidden_size == 0 {
            return None;
        }
        Some(ModelArchitecture {
            num_layers,
            hidden_size,
        })
    }
}

/// Parses a `config.json` body into a [`ModelArchitecture`], or `None` if
/// the body isn't valid JSON or doesn't carry a recognized layer-count /
/// hidden-size field pair.
fn parse_architecture(json: &str) -> Option<ModelArchitecture> {
    serde_json::from_str::<RawConfig>(json)
        .ok()
        .and_then(RawConfig::into_architecture)
}

/// Best-effort fetch of `repo`'s model architecture. Returns `None` on any
/// failure (network, missing file, unrecognized shape) rather than an
/// error: the caller falls back to the flat headroom fraction.
pub fn fetch_architecture(repo: &str) -> Option<ModelArchitecture> {
    let url = format!("https://huggingface.co/{repo}/resolve/main/config.json");

    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("auto-quantize/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    let response = client.get(&url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().ok()?;
    parse_architecture(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_transformers_config() {
        let json = r#"{"num_hidden_layers": 32, "hidden_size": 4096, "model_type": "llama"}"#;
        let arch = parse_architecture(json).unwrap();
        assert_eq!(arch.num_layers, 32);
        assert_eq!(arch.hidden_size, 4096);
    }

    #[test]
    fn parses_gpt2_style_aliases() {
        let json = r#"{"n_layer": 12, "n_embd": 768}"#;
        let arch = parse_architecture(json).unwrap();
        assert_eq!(arch.num_layers, 12);
        assert_eq!(arch.hidden_size, 768);
    }

    #[test]
    fn missing_fields_returns_none() {
        let json = r#"{"model_type": "llama", "vocab_size": 32000}"#;
        assert!(parse_architecture(json).is_none());
    }

    #[test]
    fn malformed_json_returns_none() {
        assert!(parse_architecture("not json").is_none());
    }

    #[test]
    fn zero_layers_returns_none() {
        let json = r#"{"num_hidden_layers": 0, "hidden_size": 4096}"#;
        assert!(parse_architecture(json).is_none());
    }

    #[test]
    fn prefers_transformers_field_names_over_gpt2_aliases() {
        let json =
            r#"{"num_hidden_layers": 32, "hidden_size": 4096, "n_layer": 12, "n_embd": 768}"#;
        let arch = parse_architecture(json).unwrap();
        assert_eq!(arch.num_layers, 32);
        assert_eq!(arch.hidden_size, 4096);
    }
}
