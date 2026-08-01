use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::Ctx;

pub const DMESG_HYPERVISOR_MODULES: Check = Check {
    id: "dmesg_hypervisor_modules",
    name: "Hypervisor modules in dmesg",
    description:
        "Scans the kernel ring buffer for hypervisor-related messages (vmware, qemu, kvm, hyper-v).",
    weight: 0.5,
    privilege: Privilege::Elevated,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "Hypervisor-Module in dmesg",
        description: "Durchsucht den Kernel-Ringpuffer nach hypervisorbezogenen Meldungen (vmware, qemu, kvm, hyper-v).",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let signal = match ctx.run_elevated("dmesg", &[]) {
        Ok(output) if output.success => {
            let lower = output.stdout.to_ascii_lowercase();
            if ["vmware", "qemu", "kvm", "hyper-v"]
                .iter()
                .any(|needle| lower.contains(needle))
            {
                Signal::Detected
            } else {
                Signal::NotDetected
            }
        }
        Ok(_) => Signal::Inconclusive("dmesg exited with an error"),
        Err(_) => Signal::Inconclusive("dmesg unavailable or permission denied"),
    };
    CheckResult {
        id: DMESG_HYPERVISOR_MODULES.id,
        name: DMESG_HYPERVISOR_MODULES.name,
        signal,
        weight: DMESG_HYPERVISOR_MODULES.weight,
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;
    use crate::evidence::CommandOutput;

    #[test]
    fn detects_qemu_in_dmesg() {
        let ev = MockLinuxEvidence::new().with_elevated_command(
            "dmesg",
            &[],
            CommandOutput::ok("Booting paravirtualized kernel on qemu\n"),
        );
        assert_eq!((DMESG_HYPERVISOR_MODULES.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn missing_dmesg_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (DMESG_HYPERVISOR_MODULES.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }

    #[test]
    fn ordinary_dmesg_output_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_elevated_command(
            "dmesg",
            &[],
            CommandOutput::ok("Linux version 6.1.0\nACPI: bus type PCI registered\n"),
        );
        assert_eq!(
            (DMESG_HYPERVISOR_MODULES.run)(&ev).signal,
            Signal::NotDetected
        );
    }

    #[test]
    fn dmesg_error_exit_is_inconclusive() {
        let ev = MockLinuxEvidence::new().with_elevated_command(
            "dmesg",
            &[],
            CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "dmesg: read kernel buffer failed: Operation not permitted".to_string(),
            },
        );
        assert!(matches!(
            (DMESG_HYPERVISOR_MODULES.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
