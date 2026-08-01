use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::Ctx;

pub const DPKG_GUEST_TOOLS: Check = Check {
    id: "dpkg_guest_tools",
    name: "VirtualBox/VMware guest tools installed (dpkg)",
    description: "Checks the dpkg package database for virtualbox-guest or open-vm-tools packages.",
    weight: 0.5,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "VirtualBox-/VMware-Gasttools installiert (dpkg)",
        description:
            "Prüft die dpkg-Paketdatenbank auf die Pakete virtualbox-guest oder open-vm-tools.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let signal = match ctx.run("dpkg", &["-l"]) {
        Ok(output) if output.success => {
            let lower = output.stdout.to_ascii_lowercase();
            if lower.contains("virtualbox-guest") || lower.contains("open-vm-tools") {
                Signal::Detected
            } else {
                Signal::NotDetected
            }
        }
        Ok(_) => Signal::Inconclusive("dpkg exited with an error"),
        Err(_) => Signal::Inconclusive("dpkg not found (non-Debian distribution)"),
    };
    CheckResult {
        id: DPKG_GUEST_TOOLS.id,
        name: DPKG_GUEST_TOOLS.name,
        signal,
        weight: DPKG_GUEST_TOOLS.weight,
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;
    use crate::evidence::CommandOutput;

    #[test]
    fn detects_open_vm_tools() {
        let ev = MockLinuxEvidence::new().with_command(
            "dpkg",
            &["-l"],
            CommandOutput::ok("ii  open-vm-tools  12.3.0  amd64  VMware guest tools\n"),
        );
        assert_eq!((DPKG_GUEST_TOOLS.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn no_guest_tools_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_command(
            "dpkg",
            &["-l"],
            CommandOutput::ok("ii  bash  5.1  amd64  GNU Bourne Again SHell\n"),
        );
        assert_eq!((DPKG_GUEST_TOOLS.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn dpkg_error_exit_is_inconclusive() {
        let ev = MockLinuxEvidence::new().with_command(
            "dpkg",
            &["-l"],
            CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "dpkg: error: database is locked".to_string(),
            },
        );
        assert!(matches!(
            (DPKG_GUEST_TOOLS.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }

    #[test]
    fn missing_dpkg_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (DPKG_GUEST_TOOLS.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
