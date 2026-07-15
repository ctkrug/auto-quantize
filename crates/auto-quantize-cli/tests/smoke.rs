//! Confirms the built binary actually runs, independent of unit tests.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_auto-quantize"))
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
    assert!(stdout.contains("auto-quantize"));
}

#[test]
fn recommend_help_documents_every_flag() {
    let output = bin()
        .args(["recommend", "--help"])
        .output()
        .expect("failed to execute binary");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for flag in ["--json", "--yes", "--timing", "--output"] {
        assert!(stdout.contains(flag), "--help missing {flag}:\n{stdout}");
    }
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
    assert!(stderr.contains("no GGUF quantizations") || stderr.contains("GGUF"));
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
