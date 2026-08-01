# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.0] - 2026-08-02

### Added

- Rewrite of the original `vm-check.sh` script into a Rust workspace:
  [`vm-check-core`](https://github.com/transfairs/vm-check/tree/main/vm-check-core) (the detection engine),
  [`vm-check-cli`](https://github.com/transfairs/vm-check/tree/main/vm-check-cli) (command-line interface), and
  [`vm-check-gui`](https://github.com/transfairs/vm-check/tree/main/vm-check-gui) (cross-platform desktop GUI).
- Weighted confidence scoring: each heuristic contributes a weighted signal
  instead of a single pass/fail check, with a `LikelyVirtualMachine` /
  `Uncertain` / `LikelyPhysicalMachine` verdict derived from the total.
- Windows support: WMI (`Win32_ComputerSystem`, BIOS) and registry-based
  checks, alongside the existing Linux heuristics.
- `vm-check-gui`, a native desktop GUI built with [egui](https://github.com/emilk/egui),
  with light/dark/system theme and English/German language support, both
  persisted across runs.
- `--json` output mode for scripting/CI, and `-v`/`--verbose` for
  matched-evidence detail per check.
- `-y`/`--yes` and `--no-elevate` flags to control the elevated-checks
  (`dmesg`, `dmidecode`) prompt non-interactively.
- CI (`cargo fmt`, `clippy -D warnings`, `cargo test`, a real-VM smoke test)
  and a release workflow producing Linux/Windows binary archives.
- The project page published via GitHub Pages, including this changelog.

### Changed

- Exit codes now distinguish `0` (likely physical), `1` (likely a VM), `2`
  (uncertain), and `3` (internal error), instead of a single pass/fail.
