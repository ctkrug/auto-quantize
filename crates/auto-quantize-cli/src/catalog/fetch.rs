//! Live HuggingFace API calls. Thin by design: all the testable logic lives
//! in [`super::parse`]; this module is just the network round-trip.

use std::fmt;
use std::time::Duration;

use super::parse::{group_gguf_files, parse_tree_response, CatalogQuant};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Errors that can occur while fetching a repo's GGUF catalog, kept
/// distinguishable so the CLI can report a specific reason and exit code
/// per case (docs/BACKLOG.md 1.5, 2.3).
#[derive(Debug)]
pub enum CatalogError {
    /// The repo doesn't exist, or is private/gated and inaccessible.
    RepoNotFound(String),
    /// The repo exists but has no `.gguf` files.
    NoGgufFiles(String),
    /// The request failed before we got a usable response (DNS, timeout,
    /// TLS, non-2xx/404 status).
    Network(String),
    /// The response body wasn't the JSON shape we expected.
    InvalidResponse(String),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogError::RepoNotFound(repo) => {
                write!(f, "repo '{repo}' was not found on HuggingFace")
            }
            CatalogError::NoGgufFiles(repo) => {
                write!(f, "repo '{repo}' has no GGUF quantizations")
            }
            CatalogError::Network(msg) => write!(f, "network error contacting HuggingFace: {msg}"),
            CatalogError::InvalidResponse(msg) => {
                write!(f, "unexpected response from HuggingFace: {msg}")
            }
        }
    }
}

impl std::error::Error for CatalogError {}

/// Fetches and groups the GGUF quant catalog for a HuggingFace repo id
/// (e.g. `"TheBloke/Llama-2-7B-Chat-GGUF"`).
pub fn fetch_quants(repo: &str) -> Result<Vec<CatalogQuant>, CatalogError> {
    let url = format!("https://huggingface.co/api/models/{repo}/tree/main");

    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("snug/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| CatalogError::Network(e.to_string()))?;

    let response = client
        .get(&url)
        .send()
        .map_err(|e| CatalogError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(CatalogError::RepoNotFound(repo.to_string()));
    }
    if !status.is_success() {
        return Err(CatalogError::Network(format!("HTTP {status}")));
    }

    let body = response
        .text()
        .map_err(|e| CatalogError::Network(e.to_string()))?;

    let entries =
        parse_tree_response(&body).map_err(|e| CatalogError::InvalidResponse(e.to_string()))?;
    let quants = group_gguf_files(&entries);

    if quants.is_empty() {
        return Err(CatalogError::NoGgufFiles(repo.to_string()));
    }

    Ok(quants)
}
