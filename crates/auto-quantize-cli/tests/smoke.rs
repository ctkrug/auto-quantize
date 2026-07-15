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
