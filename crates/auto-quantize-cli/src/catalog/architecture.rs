//! Best-effort fetch of a model's architecture shape (layer count + hidden
//! size), used to size an exact KV-cache headroom for `--context`
//! (docs/BACKLOG.md 1.6, 3.3).
//!
//! GGUF quant repos rarely ship a full `config.json` themselves — the
//! conversion only needs `model_type` for llama.cpp's own bookkeeping. But
//! HuggingFace requires quantizers to tag the repo with the base model it
//! was converted from (`base_model:<org>/<name>`), and that base repo
//! usually *does* publish a full `config.json`. So this fetch tries the
//! repo's own config.json first, then falls back to its tagged base model.
//!
//! Unlike the GGUF catalog fetch, failure here is never fatal: the decision
//! engine has an honest fallback (the flat headroom fraction) for exactly
//! the case where no architecture can be determined.

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

/// The subset of a HuggingFace `/api/models/{repo}` response needed to find
/// a quant repo's base model.
#[derive(Debug, Deserialize)]
struct RawModelInfo {
    #[serde(default)]
    tags: Vec<String>,
}

/// Parses a `config.json` body into a [`ModelArchitecture`], or `None` if
/// the body isn't valid JSON or doesn't carry a recognized layer-count /
/// hidden-size field pair.
fn parse_architecture(json: &str) -> Option<ModelArchitecture> {
    serde_json::from_str::<RawConfig>(json)
        .ok()
        .and_then(RawConfig::into_architecture)
}

/// Finds the base model a quant repo was converted from, from its
/// `base_model:<org>/<name>` tag (HuggingFace's convention for quantized/
/// fine-tuned derivatives). Ignores the more specific
/// `base_model:quantized:<org>/<name>` variant, which names the same repo.
fn extract_base_model(tags: &[String]) -> Option<&str> {
    tags.iter().find_map(|tag| {
        let rest = tag.strip_prefix("base_model:")?;
        if rest.starts_with("quantized:") || rest.starts_with("finetune:") {
            return None;
        }
        (!rest.is_empty()).then_some(rest)
    })
}

fn http_client() -> Option<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("auto-quantize/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()
}

fn fetch_config_json(client: &reqwest::blocking::Client, repo: &str) -> Option<String> {
    let url = format!("https://huggingface.co/{repo}/resolve/main/config.json");
    let response = client.get(&url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().ok()
}

fn fetch_base_model(client: &reqwest::blocking::Client, repo: &str) -> Option<String> {
    let url = format!("https://huggingface.co/api/models/{repo}");
    let response = client.get(&url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().ok()?;
    let info: RawModelInfo = serde_json::from_str(&body).ok()?;
    extract_base_model(&info.tags).map(str::to_string)
}

/// Best-effort fetch of `repo`'s model architecture: tries the repo's own
/// `config.json`, then its tagged base model's. Returns `None` on any
/// failure (network, missing file, unrecognized shape) rather than an
/// error: the caller falls back to the flat headroom fraction.
pub fn fetch_architecture(repo: &str) -> Option<ModelArchitecture> {
    let client = http_client()?;

    if let Some(json) = fetch_config_json(&client, repo) {
        if let Some(arch) = parse_architecture(&json) {
            return Some(arch);
        }
    }

    let base_model = fetch_base_model(&client, repo)?;
    let json = fetch_config_json(&client, &base_model)?;
    parse_architecture(&json)
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

    #[test]
    fn extract_base_model_finds_the_conversion_source() {
        let tags = vec![
            "transformers".to_string(),
            "gguf".to_string(),
            "base_model:mistralai/Mistral-7B-Instruct-v0.2".to_string(),
            "base_model:quantized:mistralai/Mistral-7B-Instruct-v0.2".to_string(),
        ];
        assert_eq!(
            extract_base_model(&tags),
            Some("mistralai/Mistral-7B-Instruct-v0.2")
        );
    }

    #[test]
    fn extract_base_model_returns_none_without_a_base_model_tag() {
        let tags = vec!["transformers".to_string(), "gguf".to_string()];
        assert!(extract_base_model(&tags).is_none());
    }

    #[test]
    fn extract_base_model_empty_tags_returns_none() {
        assert!(extract_base_model(&[]).is_none());
    }

    #[test]
    fn extract_base_model_ignores_bare_quantized_tag() {
        let tags = vec!["base_model:quantized:mistralai/Mistral-7B-Instruct-v0.2".to_string()];
        assert!(extract_base_model(&tags).is_none());
    }
}
