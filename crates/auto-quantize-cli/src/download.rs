//! Downloads the files backing a recommended quant to a local directory.

use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::catalog::QuantFile;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug)]
pub enum DownloadError {
    Network(String),
    Io(String),
    /// The number of bytes actually written didn't match the size the
    /// HuggingFace API reported for this file.
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::Network(msg) => write!(f, "download failed: {msg}"),
            DownloadError::Io(msg) => write!(f, "could not write downloaded file: {msg}"),
            DownloadError::SizeMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "downloaded '{path}' is {actual} bytes, expected {expected} bytes"
            ),
        }
    }
}

impl std::error::Error for DownloadError {}

/// Downloads every file backing a quant into `output_dir`, in order,
/// printing a progress line per file to stderr. Returns the paths written.
pub fn download_files(
    repo: &str,
    files: &[QuantFile],
    output_dir: &Path,
) -> Result<Vec<PathBuf>, DownloadError> {
    fs::create_dir_all(output_dir).map_err(|e| DownloadError::Io(e.to_string()))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("auto-quantize/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| DownloadError::Network(e.to_string()))?;

    let mut written = Vec::with_capacity(files.len());
    for (idx, file) in files.iter().enumerate() {
        eprintln!("Downloading {} ({}/{})...", file.path, idx + 1, files.len());
        written.push(download_one(&client, repo, file, output_dir)?);
    }
    Ok(written)
}

fn download_one(
    client: &reqwest::blocking::Client,
    repo: &str,
    file: &QuantFile,
    output_dir: &Path,
) -> Result<PathBuf, DownloadError> {
    let url = format!("https://huggingface.co/{repo}/resolve/main/{}", file.path);
    let dest_name = Path::new(&file.path)
        .file_name()
        .map(|n| n.to_owned())
        .unwrap_or_else(|| file.path.clone().into());
    let dest_path = output_dir.join(dest_name);

    let mut response = client
        .get(&url)
        .send()
        .map_err(|e| DownloadError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(DownloadError::Network(format!(
            "HTTP {} fetching {}",
            response.status(),
            file.path
        )));
    }

    let mut out = fs::File::create(&dest_path).map_err(|e| DownloadError::Io(e.to_string()))?;
    let mut buf = [0u8; CHUNK_SIZE];
    let mut downloaded: u64 = 0;

    loop {
        let n = response
            .read(&mut buf)
            .map_err(|e| DownloadError::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])
            .map_err(|e| DownloadError::Io(e.to_string()))?;
        downloaded += n as u64;
        eprint!(
            "\r  {:.1}/{:.1} GB",
            downloaded as f64 / 1e9,
            file.size_bytes as f64 / 1e9
        );
    }
    eprintln!();

    if downloaded != file.size_bytes {
        return Err(DownloadError::SizeMismatch {
            path: file.path.clone(),
            expected: file.size_bytes,
            actual: downloaded,
        });
    }

    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_mismatch_message_names_both_sizes() {
        let err = DownloadError::SizeMismatch {
            path: "model.Q4_K_M.gguf".into(),
            expected: 100,
            actual: 42,
        };
        let msg = err.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("42"));
        assert!(msg.contains("model.Q4_K_M.gguf"));
    }
}
