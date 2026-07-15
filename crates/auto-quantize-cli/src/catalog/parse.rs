//! Pure parsing/grouping logic for the HuggingFace repo tree API response.
//!
//! Kept free of any HTTP client so it can be unit-tested against fixture
//! JSON without a network call.

use auto_quantize_core::QuantOption;
use serde::Deserialize;

/// One entry in a HuggingFace `GET /api/models/{repo}/tree/{revision}`
/// response. Only the fields we need are modeled; the API returns more
/// (`oid`, `lfs`, ...) that we don't care about.
#[derive(Debug, Deserialize)]
pub struct TreeEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub path: String,
    #[serde(default)]
    pub size: u64,
}

/// One `.gguf` file backing a [`CatalogQuant`], as it exists in the repo.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantFile {
    /// Path within the repo, e.g. `"model.Q4_K_M-00001-of-00002.gguf"`.
    pub path: String,
    pub size_bytes: u64,
}

/// A quant option as offered by a specific repo: the core [`QuantOption`]
/// used for the fit decision, plus the underlying file(s) needed to
/// download it (more than one for multi-part GGUF splits).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogQuant {
    pub option: QuantOption,
    pub files: Vec<QuantFile>,
}

/// Parses a HuggingFace tree API JSON response into entries.
pub fn parse_tree_response(json: &str) -> Result<Vec<TreeEntry>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Filters tree entries down to `.gguf` files and groups multi-part files
/// (e.g. `model.Q4_K_M-00001-of-00002.gguf`) into a single logical
/// [`CatalogQuant`] whose size is the sum of its parts.
pub fn group_gguf_files(entries: &[TreeEntry]) -> Vec<CatalogQuant> {
    let mut groups: Vec<(String, String, Vec<QuantFile>)> = Vec::new();

    for entry in entries {
        if entry.entry_type != "file" || !entry.path.ends_with(".gguf") {
            continue;
        }

        let stem = entry.path.strip_suffix(".gguf").unwrap_or(&entry.path);
        let base_key = strip_part_suffix(stem);
        let label = extract_quant_label(base_key);
        let file = QuantFile {
            path: entry.path.clone(),
            size_bytes: entry.size,
        };

        match groups.iter_mut().find(|(key, _, _)| key == base_key) {
            Some((_, _, files)) => files.push(file),
            None => groups.push((base_key.to_string(), label, vec![file])),
        }
    }

    groups
        .into_iter()
        .map(|(_, label, mut files)| {
            files.sort_by(|a, b| a.path.cmp(&b.path));
            // The tree API's `size` field is an unvalidated u64; saturate
            // rather than panic if a malformed response sums past u64::MAX.
            let total_size = files
                .iter()
                .fold(0u64, |acc, f| acc.saturating_add(f.size_bytes));
            CatalogQuant {
                option: QuantOption::new(label, total_size),
                files,
            }
        })
        .collect()
}

/// Strips a trailing `-<digits>-of-<digits>` multi-part suffix, if present.
fn strip_part_suffix(s: &str) -> &str {
    let Some(of_idx) = s.rfind("-of-") else {
        return s;
    };
    let after = &s[of_idx + 4..];
    if after.is_empty() || !after.chars().all(|c| c.is_ascii_digit()) {
        return s;
    }

    let before = &s[..of_idx];
    let Some(dash_idx) = before.rfind('-') else {
        return s;
    };
    let part_num = &before[dash_idx + 1..];
    if part_num.is_empty() || !part_num.chars().all(|c| c.is_ascii_digit()) {
        return s;
    }

    &before[..dash_idx]
}

/// Extracts the quant label from a GGUF base filename (extension and any
/// multi-part suffix already stripped): the segment after the last `.`, or
/// the whole string if there's no `.`.
fn extract_quant_label(base_key: &str) -> String {
    base_key.rsplit('.').next().unwrap_or(base_key).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(entry_type: &str, path: &str, size: u64) -> TreeEntry {
        TreeEntry {
            entry_type: entry_type.to_string(),
            path: path.to_string(),
            size,
        }
    }

    #[test]
    fn parses_tree_response_fixture() {
        let json = r#"[
            {"type":"file","oid":"abc","size":123,"path":"README.md"},
            {"type":"file","oid":"def","size":456,"path":"model.Q4_K_M.gguf"}
        ]"#;
        let entries = parse_tree_response(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].path, "model.Q4_K_M.gguf");
        assert_eq!(entries[1].size, 456);
    }

    #[test]
    fn parses_empty_array() {
        let entries = parse_tree_response("[]").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse_tree_response("not json").is_err());
    }

    #[test]
    fn groups_filters_non_gguf_and_directories() {
        let entries = vec![
            entry("file", "README.md", 100),
            entry("directory", "coreml", 0),
            entry("file", "model.Q4_K_M.gguf", 4_000_000_000),
        ];
        let quants = group_gguf_files(&entries);
        assert_eq!(quants.len(), 1);
        assert_eq!(quants[0].option.name, "Q4_K_M");
        assert_eq!(quants[0].option.size_bytes, 4_000_000_000);
        assert_eq!(quants[0].files.len(), 1);
        assert_eq!(quants[0].files[0].path, "model.Q4_K_M.gguf");
    }

    #[test]
    fn groups_sums_multi_part_files() {
        let entries = vec![
            entry("file", "model.Q4_K_M-00001-of-00003.gguf", 1_000_000_000),
            entry("file", "model.Q4_K_M-00002-of-00003.gguf", 1_000_000_000),
            entry("file", "model.Q4_K_M-00003-of-00003.gguf", 500_000_000),
        ];
        let quants = group_gguf_files(&entries);
        assert_eq!(quants.len(), 1);
        assert_eq!(quants[0].option.name, "Q4_K_M");
        assert_eq!(quants[0].option.size_bytes, 2_500_000_000);
        assert_eq!(quants[0].files.len(), 3);
        assert_eq!(quants[0].files[0].path, "model.Q4_K_M-00001-of-00003.gguf");
        assert_eq!(quants[0].files[2].path, "model.Q4_K_M-00003-of-00003.gguf");
    }

    #[test]
    fn groups_keeps_distinct_quants_separate() {
        let entries = vec![
            entry("file", "model.Q4_K_M.gguf", 4_000_000_000),
            entry("file", "model.Q5_K_M.gguf", 6_000_000_000),
        ];
        let quants = group_gguf_files(&entries);
        assert_eq!(quants.len(), 2);
        let names: Vec<&str> = quants.iter().map(|q| q.option.name.as_str()).collect();
        assert!(names.contains(&"Q4_K_M"));
        assert!(names.contains(&"Q5_K_M"));
    }

    #[test]
    fn groups_empty_entries_returns_empty() {
        assert!(group_gguf_files(&[]).is_empty());
    }

    #[test]
    fn groups_no_gguf_files_returns_empty() {
        let entries = vec![
            entry("file", "README.md", 100),
            entry("file", "config.json", 29),
        ];
        assert!(group_gguf_files(&entries).is_empty());
    }

    #[test]
    fn label_falls_back_to_filename_with_no_dot() {
        let entries = vec![entry("file", "model.gguf", 4_000_000_000)];
        let quants = group_gguf_files(&entries);
        assert_eq!(quants[0].option.name, "model");
    }

    #[test]
    fn groups_saturates_instead_of_overflowing_on_absurd_sizes() {
        // The HuggingFace tree API's `size` field is an unvalidated u64; a
        // malformed or hostile response summing multi-part sizes near
        // u64::MAX must not panic.
        let entries = vec![
            entry("file", "model.Q4_K_M-00001-of-00002.gguf", u64::MAX),
            entry("file", "model.Q4_K_M-00002-of-00002.gguf", u64::MAX),
        ];
        let quants = group_gguf_files(&entries);
        assert_eq!(quants[0].option.size_bytes, u64::MAX);
    }
}
