use clap::Parser;
use owo_colors::{OwoColorize, Stream::Stdout, Style};
use std::io::{BufRead, Write};
use vm_check_core::check::{CheckResult, Privilege, Signal};
use vm_check_core::{all_checks, Report, Verdict};

#[derive(Parser)]
#[command(
    name = "vm-check",
    version,
    about = "Checks whether the current system is running inside a virtual machine."
)]
struct Cli {
    /// Output a machine-readable JSON report instead of the human-readable summary.
    #[arg(long)]
    json: bool,

    /// Skip checks that require elevated privileges instead of asking.
    #[arg(long)]
    no_elevate: bool,

    /// Don't ask for confirmation before running privileged checks.
    #[arg(short = 'y', long)]
    yes: bool,

    /// Print each check's inconclusive reason and matched-evidence detail alongside its result.
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();
    let evidence = vm_check_core::evidence::SystemEvidence;
    let all = all_checks();
    let (privileged, unprivileged): (Vec<_>, Vec<_>) = all
        .into_iter()
        .partition(|c| c.privilege == Privilege::Elevated);

    let mut results: Vec<CheckResult> = unprivileged.iter().map(|c| (c.run)(&evidence)).collect();

    let run_privileged = decide_run_privileged(
        !privileged.is_empty(),
        cli.no_elevate,
        cli.yes,
        vm_check_core::evidence::running_as_root(),
        || confirm_elevation(&privileged, &mut std::io::stdin().lock()),
    );

    for check in &privileged {
        if run_privileged {
            results.push((check.run)(&evidence));
        } else {
            results.push(CheckResult {
                id: check.id,
                name: check.name,
                signal: Signal::Inconclusive(
                    "skipped: requires elevated privileges, pass -y or omit --no-elevate to include",
                ),
                weight: check.weight,
                detail: None,
            });
        }
    }

    let report = Report::new(results);

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("Report is always serializable")
        );
    } else {
        print_human_readable(&report, cli.verbose, &mut std::io::stdout());
    }

    std::process::exit(exit_code(report.verdict()));
}

/// Whether to actually run the privileged checks, without ever prompting
/// (`confirm` is only called, not eagerly evaluated) when there's nothing to
/// ask about or the answer is already decided by a flag/being root. Split
/// out from `main()` so the "nothing to ask about" case is exercisable
/// without depending on the current (always non-empty) real check list.
fn decide_run_privileged(
    has_privileged: bool,
    no_elevate: bool,
    yes: bool,
    is_root: bool,
    confirm: impl FnOnce() -> bool,
) -> bool {
    if !has_privileged {
        return false;
    }
    if no_elevate {
        false
    } else if yes || is_root {
        // Already root (e.g. launched via `sudo vm-check`): there is no
        // privilege left to escalate to, so asking permission to elevate
        // is a question that doesn't apply, so just run them.
        true
    } else {
        confirm()
    }
}

fn confirm_elevation(privileged: &[vm_check_core::Check], reader: &mut impl BufRead) -> bool {
    let names: Vec<&str> = privileged.iter().map(|c| c.name).collect();
    print!(
        "{} check(s) need elevated privileges ({}), run them? [Y/n] ",
        privileged.len(),
        names.join(", ")
    );
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    if reader.read_line(&mut answer).is_err() {
        return false;
    }
    let answer = answer.trim().to_ascii_lowercase();
    answer.is_empty() || answer == "y" || answer == "yes"
}

fn print_human_readable(report: &Report, verbose: bool, out: &mut impl Write) {
    writeln!(out).ok();
    writeln!(
        out,
        "{}",
        "vm-check".if_supports_color(Stdout, |t| t.bold())
    )
    .ok();
    writeln!(
        out,
        "{}",
        "─".repeat(8).if_supports_color(Stdout, |t| t.dimmed())
    )
    .ok();
    writeln!(out).ok();

    let name_width = report
        .results
        .iter()
        .map(|r| r.name.len())
        .max()
        .unwrap_or(0);

    let bold_red = Style::new().red().bold();
    let bold_green = Style::new().green().bold();
    let bold_yellow = Style::new().yellow().bold();

    for result in &report.results {
        let (icon, label) = match result.signal {
            Signal::Detected => (
                "✗".if_supports_color(Stdout, |t| t.red()).to_string(),
                "FAIL"
                    .if_supports_color(Stdout, |t| t.style(bold_red))
                    .to_string(),
            ),
            Signal::NotDetected => (
                "✓".if_supports_color(Stdout, |t| t.green()).to_string(),
                "PASS"
                    .if_supports_color(Stdout, |t| t.style(bold_green))
                    .to_string(),
            ),
            Signal::Inconclusive(_) => (
                "○".if_supports_color(Stdout, |t| t.yellow()).to_string(),
                "SKIP"
                    .if_supports_color(Stdout, |t| t.style(bold_yellow))
                    .to_string(),
            ),
        };
        writeln!(out, "  {icon}  {:<name_width$}  {label}", result.name).ok();
        if verbose {
            if let Signal::Inconclusive(reason) = result.signal {
                writeln!(
                    out,
                    "     {}",
                    reason.if_supports_color(Stdout, |t| t.dimmed())
                )
                .ok();
            }
            if let Some(detail) = &result.detail {
                writeln!(
                    out,
                    "     {}",
                    detail.if_supports_color(Stdout, |t| t.dimmed())
                )
                .ok();
            }
        }
    }

    writeln!(out).ok();
    let confidence = report.confidence();
    let confidence_pct = (confidence * 100.0).round();
    let (verdict_text, bar) = match report.verdict() {
        Verdict::LikelyVirtualMachine => (
            "running in a virtual machine"
                .if_supports_color(Stdout, |t| t.style(bold_red))
                .to_string(),
            confidence_bar(confidence)
                .if_supports_color(Stdout, |t| t.red())
                .to_string(),
        ),
        Verdict::LikelyPhysicalMachine => (
            "NOT running in a virtual machine"
                .if_supports_color(Stdout, |t| t.style(bold_green))
                .to_string(),
            confidence_bar(confidence)
                .if_supports_color(Stdout, |t| t.green())
                .to_string(),
        ),
        Verdict::Uncertain => (
            "uncertain: inconclusive signals"
                .if_supports_color(Stdout, |t| t.style(bold_yellow))
                .to_string(),
            confidence_bar(confidence)
                .if_supports_color(Stdout, |t| t.yellow())
                .to_string(),
        ),
    };
    writeln!(out, "  Verdict         {verdict_text}").ok();
    writeln!(out, "  VM likelihood   {bar}  {confidence_pct}%").ok();
    writeln!(out).ok();
}

/// A fixed-width bar of filled/empty blocks proportional to `confidence` (0.0–1.0).
fn confidence_bar(confidence: f32) -> String {
    const WIDTH: usize = 24;
    let filled = ((confidence.clamp(0.0, 1.0) * WIDTH as f32).round() as usize).min(WIDTH);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(WIDTH - filled))
}

fn exit_code(verdict: Verdict) -> i32 {
    match verdict {
        Verdict::LikelyPhysicalMachine => 0,
        Verdict::LikelyVirtualMachine => 1,
        Verdict::Uncertain => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;
    use vm_check_core::Check;

    fn dummy_check(id: &'static str, name: &'static str) -> Check {
        fn run(_: vm_check_core::evidence::Ctx) -> CheckResult {
            unreachable!("confirm_elevation only reads check metadata")
        }
        Check {
            id,
            name,
            description: "dummy",
            weight: 1.0,
            privilege: Privilege::Elevated,
            run,
            translations: &[],
        }
    }

    fn result(name: &'static str, signal: Signal) -> CheckResult {
        CheckResult {
            id: name,
            name,
            signal,
            weight: 1.0,
            detail: Some("some detail".to_string()),
        }
    }

    #[test]
    fn confidence_bar_is_empty_at_zero() {
        assert_eq!(confidence_bar(0.0), format!("[{}]", "░".repeat(24)));
    }

    #[test]
    fn confidence_bar_is_full_at_one() {
        assert_eq!(confidence_bar(1.0), format!("[{}]", "█".repeat(24)));
    }

    #[test]
    fn confidence_bar_clamps_out_of_range_values() {
        assert_eq!(confidence_bar(-1.0), confidence_bar(0.0));
        assert_eq!(confidence_bar(2.0), confidence_bar(1.0));
    }

    #[test]
    fn exit_code_matches_verdict() {
        assert_eq!(exit_code(Verdict::LikelyPhysicalMachine), 0);
        assert_eq!(exit_code(Verdict::LikelyVirtualMachine), 1);
        assert_eq!(exit_code(Verdict::Uncertain), 2);
    }

    #[test]
    fn decide_run_privileged_skips_the_prompt_when_nothing_is_privileged() {
        let mut confirm_called = false;
        let result = decide_run_privileged(false, false, false, false, || {
            confirm_called = true;
            true
        });
        assert!(!result);
        assert!(
            !confirm_called,
            "must not prompt when there's nothing to ask about"
        );
    }

    #[test]
    fn decide_run_privileged_no_elevate_skips_without_prompting() {
        let result = decide_run_privileged(true, true, false, false, || {
            panic!("must not prompt when --no-elevate was passed")
        });
        assert!(!result);
    }

    #[test]
    fn decide_run_privileged_yes_flag_skips_the_prompt() {
        let result = decide_run_privileged(true, false, true, false, || {
            panic!("must not prompt when -y/--yes was passed")
        });
        assert!(result);
    }

    #[test]
    fn decide_run_privileged_already_root_skips_the_prompt() {
        let result = decide_run_privileged(true, false, false, true, || {
            panic!("must not prompt when already root")
        });
        assert!(result);
    }

    #[test]
    fn decide_run_privileged_falls_back_to_the_prompt() {
        assert!(decide_run_privileged(true, false, false, false, || true));
        assert!(!decide_run_privileged(true, false, false, false, || false));
    }

    #[test]
    fn confirm_elevation_accepts_yes_variants() {
        for input in ["y\n", "Y\n", "yes\n", "\n"] {
            let mut reader = BufReader::new(input.as_bytes());
            let checks = [dummy_check("a", "Check A")];
            assert!(
                confirm_elevation(&checks, &mut reader),
                "expected {input:?} to be accepted"
            );
        }
    }

    #[test]
    fn confirm_elevation_rejects_other_input() {
        let mut reader = BufReader::new("nope\n".as_bytes());
        let checks = [dummy_check("a", "Check A")];
        assert!(!confirm_elevation(&checks, &mut reader));
    }

    #[test]
    fn confirm_elevation_rejects_on_read_error() {
        struct ErrReader;
        impl std::io::Read for ErrReader {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("boom"))
            }
        }
        let mut reader = BufReader::new(ErrReader);
        let checks = [dummy_check("a", "Check A")];
        assert!(!confirm_elevation(&checks, &mut reader));
    }

    #[test]
    fn print_human_readable_covers_every_signal_and_verdict() {
        let cases: Vec<(Vec<CheckResult>, bool)> = vec![
            (
                vec![
                    result("Detected check", Signal::Detected),
                    result("Undetected check", Signal::NotDetected),
                    result("Skipped check", Signal::Inconclusive("skipped: reason")),
                ],
                true,
            ),
            (vec![result("Physical-leaning", Signal::NotDetected)], false),
            (vec![result("VM-leaning", Signal::Detected)], false),
            (vec![], false),
        ];
        for (results, verbose) in cases {
            let report = Report::new(results);
            let mut out = Vec::new();
            print_human_readable(&report, verbose, &mut out);
            let text = String::from_utf8(out).unwrap();
            assert!(text.contains("vm-check"));
            assert!(text.contains("Verdict"));
        }
    }
}
