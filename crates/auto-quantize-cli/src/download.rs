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
///
/// An existing partial file at the destination path is resumed via an HTTP
/// `Range` request rather than restarted from zero (docs/BACKLOG.md 2.2); a
/// destination that already matches the expected size is left untouched.
pub fn download_files(
    repo: &str,
    files: &[QuantFile],
    output_dir: &Path,
) -> Result<Vec<PathBuf>, DownloadError> {
    fs::create_dir_all(output_dir).map_err(|e| DownloadError::Io(e.to_string()))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("snug/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| DownloadError::Network(e.to_string()))?;

    let mut written = Vec::with_capacity(files.len());
    for (idx, file) in files.iter().enumerate() {
        eprintln!("Downloading {} ({}/{})...", file.path, idx + 1, files.len());
        let url = format!("https://huggingface.co/{repo}/resolve/main/{}", file.path);
        let dest_name = Path::new(&file.path)
            .file_name()
            .map(|n| n.to_owned())
            .unwrap_or_else(|| file.path.clone().into());
        let dest_path = output_dir.join(dest_name);
        download_to(&client, &url, &dest_path, file.size_bytes, &file.path)?;
        written.push(dest_path);
    }
    Ok(written)
}

/// Streams `url` to `dest_path`, verifying the final size against
/// `expected_size`. `label` is used only in error messages. Returns the
/// number of bytes actually transferred over the network during this call
/// (0 if the destination already matched `expected_size`).
fn download_to(
    client: &reqwest::blocking::Client,
    url: &str,
    dest_path: &Path,
    expected_size: u64,
    label: &str,
) -> Result<u64, DownloadError> {
    let mut existing_len = fs::metadata(dest_path).map(|m| m.len()).unwrap_or(0);

    if existing_len == expected_size {
        eprintln!("  already downloaded, skipping");
        return Ok(0);
    }
    if existing_len > expected_size {
        // Bigger than the real file: a stale/corrupt leftover, not partial
        // progress. Treat as absent so the fresh-download path below
        // truncates and rewrites it from scratch.
        existing_len = 0;
    }

    let mut request = client.get(url);
    if existing_len > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing_len}-"));
    }

    let mut response = request
        .send()
        .map_err(|e| DownloadError::Network(e.to_string()))?;

    let (mut out, mut downloaded) =
        if existing_len > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            let file = fs::OpenOptions::new()
                .append(true)
                .open(dest_path)
                .map_err(|e| DownloadError::Io(e.to_string()))?;
            (file, existing_len)
        } else {
            if !response.status().is_success() {
                return Err(DownloadError::Network(format!(
                    "HTTP {} fetching {label}",
                    response.status(),
                )));
            }
            // Either a fresh download, or the server ignored our Range request
            // (some don't support it) and sent the full body back: start over.
            let file = fs::File::create(dest_path).map_err(|e| DownloadError::Io(e.to_string()))?;
            (file, 0)
        };
    let transferred_start = downloaded;

    let mut buf = [0u8; CHUNK_SIZE];
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
            expected_size as f64 / 1e9
        );
    }
    eprintln!();

    if downloaded != expected_size {
        return Err(DownloadError::SizeMismatch {
            path: label.to_string(),
            expected: expected_size,
            actual: downloaded,
        });
    }

    Ok(downloaded - transferred_start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

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

    /// Minimal single-request HTTP/1.1 test double. Serves `body` in full
    /// unless the request carries a `Range: bytes=N-` header, in which case
    /// (when `honor_range` is true) it replies 206 with only the remaining
    /// bytes; when `honor_range` is false it ignores Range and always
    /// replies 200 with the full body, mimicking a server without range
    /// support.
    fn serve_once(body: &'static [u8], honor_range: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                handle_request(stream, body, honor_range);
            }
        });
        format!("http://{addr}")
    }

    fn handle_request(mut stream: TcpStream, body: &[u8], honor_range: bool) {
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut range_start: Option<usize> = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(rest) = line
                .strip_prefix("Range: bytes=")
                .or_else(|| line.strip_prefix("range: bytes="))
            {
                range_start = rest.trim_end_matches('-').parse::<usize>().ok();
            }
        }

        if honor_range {
            if let Some(start) = range_start.filter(|&s| s < body.len()) {
                let slice = &body[start..];
                let headers = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    start, body.len() - 1, body.len(), slice.len()
                );
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(slice);
                return;
            }
        }

        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(headers.as_bytes());
        let _ = stream.write_all(body);
    }

    fn temp_dest(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("snug-test-{name}-{:?}", thread::current().id()));
        path
    }

    const FIXTURE: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

    #[test]
    fn fresh_download_writes_the_full_body() {
        let url = serve_once(FIXTURE, true);
        let dest = temp_dest("fresh");
        let _ = fs::remove_file(&dest);

        let client = reqwest::blocking::Client::new();
        let transferred =
            download_to(&client, &url, &dest, FIXTURE.len() as u64, "fixture").unwrap();

        assert_eq!(transferred, FIXTURE.len() as u64);
        assert_eq!(fs::read(&dest).unwrap(), FIXTURE);
        let _ = fs::remove_file(&dest);
    }

    #[test]
    fn resumes_from_existing_partial_file_via_range_request() {
        let half = FIXTURE.len() / 2;
        let url = serve_once(FIXTURE, true);
        let dest = temp_dest("resume");
        fs::write(&dest, &FIXTURE[..half]).unwrap();

        let client = reqwest::blocking::Client::new();
        let transferred =
            download_to(&client, &url, &dest, FIXTURE.len() as u64, "fixture").unwrap();

        assert!(
            transferred < FIXTURE.len() as u64,
            "resume should transfer fewer bytes than the full file, got {transferred}"
        );
        assert_eq!(transferred as usize, FIXTURE.len() - half);
        assert_eq!(fs::read(&dest).unwrap(), FIXTURE);
        let _ = fs::remove_file(&dest);
    }

    #[test]
    fn falls_back_to_full_restart_when_server_ignores_range() {
        let half = FIXTURE.len() / 2;
        let url = serve_once(FIXTURE, false);
        let dest = temp_dest("no-range-support");
        fs::write(&dest, &FIXTURE[..half]).unwrap();

        let client = reqwest::blocking::Client::new();
        let transferred =
            download_to(&client, &url, &dest, FIXTURE.len() as u64, "fixture").unwrap();

        assert_eq!(transferred, FIXTURE.len() as u64);
        assert_eq!(fs::read(&dest).unwrap(), FIXTURE);
        let _ = fs::remove_file(&dest);
    }

    #[test]
    fn oversized_stale_file_is_discarded_and_redownloaded() {
        // A destination file bigger than the expected size is stale/corrupt
        // (e.g. left over from an interrupted write, or a same-named quant
        // from a different repo revision) — it must not be silently kept.
        let url = serve_once(FIXTURE, true);
        let dest = temp_dest("oversized-stale");
        let mut garbage = FIXTURE.to_vec();
        garbage.extend_from_slice(b"-TRAILING-GARBAGE-FROM-A-STALE-WRITE");
        fs::write(&dest, &garbage).unwrap();

        let client = reqwest::blocking::Client::new();
        let transferred =
            download_to(&client, &url, &dest, FIXTURE.len() as u64, "fixture").unwrap();

        assert_eq!(transferred, FIXTURE.len() as u64);
        assert_eq!(fs::read(&dest).unwrap(), FIXTURE);
        let _ = fs::remove_file(&dest);
    }

    #[test]
    fn already_complete_file_is_skipped_without_a_network_call() {
        let dest = temp_dest("already-complete");
        fs::write(&dest, FIXTURE).unwrap();

        // An unroutable address: if download_to tried to connect, this
        // would error out (or hang past the client's timeout) instead of
        // returning immediately.
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let transferred = download_to(
            &client,
            "http://127.0.0.1:1",
            &dest,
            FIXTURE.len() as u64,
            "fixture",
        )
        .unwrap();

        assert_eq!(transferred, 0);
        let _ = fs::remove_file(&dest);
    }
}
