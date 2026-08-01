//! Integration tests that exercise the compiled `vm-check` binary directly.
//! `fn main()` calls `std::process::exit`, so it cannot run in-process;
//! spawning the real binary is the only way to cover its argument-parsing
//! and orchestration logic (`cargo llvm-cov` picks up coverage from child
//! processes automatically).

#[cfg(target_os = "linux")]
use std::io::Write;
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::Stdio;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vm-check"))
}

#[test]
fn json_output_is_valid_and_exit_code_reflects_verdict() {
    let output = bin().args(["--no-elevate", "--json"]).output().unwrap();
    assert!(matches!(output.status.code(), Some(0..=2)));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON report");
    assert!(parsed["results"].is_array());
}

#[test]
fn human_readable_output_contains_verdict() {
    let output = bin().args(["--no-elevate"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("vm-check"));
    assert!(stdout.contains("Verdict"));
}

#[test]
fn verbose_flag_prints_check_details() {
    let output = bin().args(["--no-elevate", "--verbose"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Verdict"));
}

// Only Linux has privileged checks (dmesg, dmidecode); on Windows there's
// nothing for --no-elevate to skip or for the interactive prompt to offer.
#[test]
#[cfg(target_os = "linux")]
fn no_elevate_skips_privileged_checks() {
    let output = bin().args(["--no-elevate", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("skipped: requires elevated privileges"));
}

#[test]
fn yes_flag_runs_without_prompting() {
    // No stdin is provided at all; if the `-y` fast path fell through to the
    // interactive prompt, reading stdin would return EOF/empty (still
    // truthy) rather than hang, but this asserts the flag actually skips the
    // prompt: no "run them?" text should appear in the output.
    let output = bin().args(["--yes", "--json"]).output().unwrap();
    assert!(matches!(output.status.code(), Some(0..=2)));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("run them?"));
}

#[test]
#[cfg(target_os = "linux")]
fn declining_elevation_interactively_skips_privileged_checks() {
    // Elevated checks in turn shell out to `sudo`, which, with stdin closed
    // by `output()` and no cached credentials, fails immediately instead of
    // prompting, so this cannot hang the test suite.
    let mut child = bin()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn vm-check");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"n\n")
        .expect("failed to write to stdin");
    let output = child
        .wait_with_output()
        .expect("failed to wait for vm-check");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("run them?"));
    assert!(stdout.contains("SKIP"));
}

#[test]
#[cfg(target_os = "linux")]
fn accepting_elevation_interactively_runs_privileged_checks() {
    let mut child = bin()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn vm-check");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"y\n")
        .expect("failed to write to stdin");
    let output = child
        .wait_with_output()
        .expect("failed to wait for vm-check");
    assert!(matches!(output.status.code(), Some(0..=2)));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("run them?"));
}

#[test]
fn help_flag_exits_successfully() {
    let output = bin().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("Usage"));
}
