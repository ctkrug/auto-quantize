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
