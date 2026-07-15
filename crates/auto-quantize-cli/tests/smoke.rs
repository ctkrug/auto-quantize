//! Confirms the built binary actually runs, independent of unit tests.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_snug"))
}

#[test]
fn runs_with_no_args_and_exits_successfully() {
    let output = bin().output().expect("failed to execute binary");
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());
}

#[test]
fn probe_subcommand_exits_successfully() {
    let output = bin()
        .arg("probe")
        .output()
        .expect("failed to execute binary");
    assert!(output.status.success());
}

#[test]
fn reports_version() {
    let output = bin()
        .arg("--version")
        .output()
        .expect("failed to execute binary");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("snug"));
}

#[test]
fn recommend_help_documents_every_flag() {
    let output = bin()
        .args(["recommend", "--help"])
        .output()
        .expect("failed to execute binary");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for flag in [
        "--json",
        "--yes",
        "--timing",
        "--output",
        "--reserve-vram",
        "--prefer",
        "--context",
    ] {
        assert!(stdout.contains(flag), "--help missing {flag}:\n{stdout}");
    }
}

#[test]
fn top_level_help_documents_both_subcommands() {
    let output = bin()
        .arg("--help")
        .output()
        .expect("failed to execute binary");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("probe"));
    assert!(stdout.contains("recommend"));
}

#[test]
fn recommend_missing_repo_arg_is_a_clean_usage_error_not_a_panic() {
    let output = bin()
        .arg("recommend")
        .output()
        .expect("failed to execute binary");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"));
}

/// Exercises the real HuggingFace API against a repo with zero GGUF files
/// (docs/BACKLOG.md 1.1, 2.3): distinct, non-panicking error and exit code.
#[test]
fn recommend_repo_with_no_gguf_files_exits_non_zero_without_panicking() {
    let output = bin()
        .args(["recommend", "bert-base-uncased"])
        .output()
        .expect("failed to execute binary");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"));
    assert!(
        !stderr.contains("Err("),
        "leaked raw Debug output:\n{stderr}"
    );
    assert!(stderr.contains("no GGUF quantizations") || stderr.contains("GGUF"));
}

/// docs/BACKLOG.md 3.2: each failure class gets its own documented exit code.
#[test]
fn recommend_nonexistent_repo_exits_with_repo_not_found_code() {
    let output = bin()
        .args(["recommend", "ctkrug/this-repo-does-not-exist-xyz123"])
        .output()
        .expect("failed to execute binary");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"));
    assert!(
        !stderr.contains("Err("),
        "leaked raw Debug output:\n{stderr}"
    );
    assert!(stderr.contains("was not found"));
}

#[derive(serde::Deserialize)]
struct JsonOutput {
    hardware: JsonHardware,
    recommendation: JsonRecommendation,
    reason: String,
}

#[derive(serde::Deserialize)]
struct JsonHardware {
    ram_bytes: u64,
}

#[derive(serde::Deserialize)]
struct JsonRecommendation {
    quant: String,
    size_bytes: u64,
    fits_fully: bool,
}

/// docs/BACKLOG.md 3.1: --json emits exactly one JSON object on stdout
/// that round-trips through serde_json into a typed struct.
#[test]
fn recommend_json_output_round_trips_and_has_no_other_stdout() {
    let output = bin()
        .args(["recommend", "TheBloke/Llama-2-7B-Chat-GGUF", "--json"])
        .output()
        .expect("failed to execute binary");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().count(),
        1,
        "expected exactly one line of JSON on stdout, got:\n{stdout}"
    );

    let parsed: JsonOutput =
        serde_json::from_str(stdout.trim()).expect("stdout must be a single valid JSON object");
    assert!(parsed.hardware.ram_bytes > 0);
    assert!(!parsed.recommendation.quant.is_empty());
    assert!(parsed.recommendation.size_bytes > 0);
    assert!(!parsed.reason.is_empty());
    let _ = parsed.recommendation.fits_fully;
}

/// docs/BACKLOG.md 1.6, 3.3: --context resolves the repo's base-model
/// architecture (this repo's own config.json only has model_type; the
/// KV-cache sizing comes from its tagged base model) and names the context
/// length in the reason instead of using the flat headroom fraction.
#[test]
fn recommend_with_context_names_the_context_length_in_the_reason() {
    let output = bin()
        .args([
            "recommend",
            "TheBloke/Mistral-7B-Instruct-v0.2-GGUF",
            "--json",
            "--context",
            "8192",
        ])
        .output()
        .expect("failed to execute binary");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: JsonOutput =
        serde_json::from_str(stdout.trim()).expect("stdout must be a single valid JSON object");
    assert!(
        parsed.reason.contains("8192"),
        "reason should name the context length:\n{}",
        parsed.reason
    );
    assert!(parsed.reason.contains("KV cache"));
}
