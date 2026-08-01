use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::Ctx;

/// `systemd-detect-virt` is purpose-built for exactly this question and needs
/// no root, so it gets the highest weight of any Linux check.
pub const SYSTEMD_DETECT_VIRT: Check = Check {
    id: "systemd_detect_virt",
    name: "systemd-detect-virt",
    description: "Asks systemd's own virtualization detector, which covers containers as well as hypervisors.",
    weight: 1.0,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "systemd-detect-virt",
        description: "Fragt systemds eigenen Virtualisierungsdetektor ab, der neben Hypervisoren auch Container erkennt.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    // Unlike other checks, we don't gate on output.success: systemd-detect-virt
    // exits 1 (with stdout "none") when nothing is detected, so success alone
    // can't distinguish "no VM" from "tool failed".
    let signal = match ctx.run("systemd-detect-virt", &[]) {
        Ok(output) => {
            let virt = output.stdout.trim();
            if virt.is_empty() || virt.eq_ignore_ascii_case("none") {
                Signal::NotDetected
            } else {
                detail = Some(virt.to_string());
                Signal::Detected
            }
        }
        Err(_) => Signal::Inconclusive("systemd-detect-virt not found"),
    };
    CheckResult {
        id: SYSTEMD_DETECT_VIRT.id,
        name: SYSTEMD_DETECT_VIRT.name,
        signal,
        weight: SYSTEMD_DETECT_VIRT.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;
    use crate::evidence::CommandOutput;

    #[test]
    fn detects_kvm() {
        let ev = MockLinuxEvidence::new().with_command(
            "systemd-detect-virt",
            &[],
            CommandOutput::ok("kvm\n"),
        );
        let result = (SYSTEMD_DETECT_VIRT.run)(&ev);
        assert_eq!(result.signal, Signal::Detected);
        assert_eq!(result.detail.as_deref(), Some("kvm"));
    }

    #[test]
    fn none_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_command(
            "systemd-detect-virt",
            &[],
            CommandOutput::ok("none\n"),
        );
        assert_eq!((SYSTEMD_DETECT_VIRT.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn missing_binary_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (SYSTEMD_DETECT_VIRT.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
