use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::Ctx;

/// Guest-driver kernel modules. Distinct from the dpkg check: a kernel module
/// can be loaded (e.g. bundled in a generic kernel) without the userspace
/// guest-tools package being installed at all.
///
/// Deliberately excludes `vmw_vmci`: it's the shared VMware host<->guest
/// communication channel and gets loaded by VMware Workstation/Player on the
/// *host* side too, so its presence alone doesn't distinguish "this machine is
/// a VMware guest" from "this machine runs VMware as a hypervisor host".
const KNOWN_MODULES: &[&str] = &[
    "vboxguest",
    "vboxsf",
    "vboxvideo",
    "vmw_balloon",
    "vmwgfx",
    "hv_vmbus",
    "hv_utils",
    "hv_balloon",
];

pub const KERNEL_MODULE_SIGNATURE: Check = Check {
    id: "kernel_module_signature",
    name: "Hypervisor guest kernel modules",
    description:
        "Checks loaded kernel modules (lsmod) for VirtualBox/VMware/Hyper-V guest drivers.",
    weight: 0.6,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "Hypervisor-Gastkernelmodule",
        description:
            "Prüft geladene Kernelmodule (lsmod) auf VirtualBox-/VMware-/Hyper-V-Gasttreiber.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.run("lsmod", &[]) {
        Ok(output) if output.success => {
            match KNOWN_MODULES
                .iter()
                .find(|module| output.stdout.lines().any(|line| line.starts_with(*module)))
            {
                Some(module) => {
                    detail = Some(module.to_string());
                    Signal::Detected
                }
                None => Signal::NotDetected,
            }
        }
        Ok(_) => Signal::Inconclusive("lsmod exited with an error"),
        Err(_) => Signal::Inconclusive("lsmod not found"),
    };
    CheckResult {
        id: KERNEL_MODULE_SIGNATURE.id,
        name: KERNEL_MODULE_SIGNATURE.name,
        signal,
        weight: KERNEL_MODULE_SIGNATURE.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;
    use crate::evidence::CommandOutput;

    #[test]
    fn detects_vboxguest_module() {
        let ev = MockLinuxEvidence::new().with_command(
            "lsmod",
            &[],
            CommandOutput::ok(
                "Module                  Size  Used by\nvboxguest             417792  2 vboxsf\n",
            ),
        );
        let result = (KERNEL_MODULE_SIGNATURE.run)(&ev);
        assert_eq!(result.signal, Signal::Detected);
        assert_eq!(result.detail.as_deref(), Some("vboxguest"));
    }

    #[test]
    fn no_hypervisor_modules_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_command(
            "lsmod",
            &[],
            CommandOutput::ok(
                "Module                  Size  Used by\next4                  978944  1\n",
            ),
        );
        assert_eq!(
            (KERNEL_MODULE_SIGNATURE.run)(&ev).signal,
            Signal::NotDetected
        );
    }

    #[test]
    fn lsmod_error_exit_is_inconclusive() {
        let ev = MockLinuxEvidence::new().with_command(
            "lsmod",
            &[],
            CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "lsmod: permission denied".to_string(),
            },
        );
        assert!(matches!(
            (KERNEL_MODULE_SIGNATURE.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }

    #[test]
    fn missing_lsmod_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (KERNEL_MODULE_SIGNATURE.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
