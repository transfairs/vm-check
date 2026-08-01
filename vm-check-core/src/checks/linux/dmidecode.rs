use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::{CommandOutput, Ctx};

const VENDOR_NEEDLES: &[&str] = &["vmware", "virtualbox", "qemu", "kvm", "xen", "microsoft"];

/// Runs `dmidecode -s <field>` unprivileged first (many distros expose
/// `/sys/firmware/dmi/tables/DMI` world-readable, so dmidecode often works
/// without root), and only escalates via sudo if that fails. This keeps the
/// common case prompt-free instead of always requesting elevation up front.
fn read_dmidecode_field(ctx: Ctx, field: &str) -> Option<CommandOutput> {
    if let Ok(output) = ctx.run("dmidecode", &["-s", field]) {
        if output.success {
            return Some(output);
        }
    }
    ctx.run_elevated("dmidecode", &["-s", field]).ok()
}

fn contains_hypervisor_vendor(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    VENDOR_NEEDLES.iter().any(|needle| lower.contains(needle))
}

pub const SYSTEM_MANUFACTURER: Check = Check {
    id: "dmidecode_system_manufacturer",
    name: "System manufacturer (dmidecode)",
    description: "Checks the DMI system-manufacturer string for known hypervisor vendors.",
    weight: 0.7,
    privilege: Privilege::Elevated,
    run: run_system_manufacturer,
    translations: &[Translation {
        language: Language::De,
        name: "Systemhersteller (dmidecode)",
        description: "Prüft die DMI-Systemherstellerzeichenkette auf bekannte Hypervisor-Anbieter.",
    }],
};

fn run_system_manufacturer(ctx: Ctx) -> CheckResult {
    let signal = match read_dmidecode_field(ctx, "system-manufacturer") {
        Some(output) if output.success => {
            if contains_hypervisor_vendor(&output.stdout) {
                Signal::Detected
            } else {
                Signal::NotDetected
            }
        }
        _ => Signal::Inconclusive("dmidecode unavailable or permission denied"),
    };
    CheckResult {
        id: SYSTEM_MANUFACTURER.id,
        name: SYSTEM_MANUFACTURER.name,
        signal,
        weight: SYSTEM_MANUFACTURER.weight,
        detail: None,
    }
}

pub const BIOS_VENDOR: Check = Check {
    id: "dmidecode_bios_vendor",
    name: "BIOS vendor (dmidecode)",
    description: "Checks the DMI bios-vendor string for known hypervisor vendors.",
    weight: 0.7,
    privilege: Privilege::Elevated,
    run: run_bios_vendor,
    translations: &[Translation {
        language: Language::De,
        name: "BIOS-Hersteller (dmidecode)",
        description: "Prüft die DMI-BIOS-Herstellerzeichenkette auf bekannte Hypervisor-Anbieter.",
    }],
};

fn run_bios_vendor(ctx: Ctx) -> CheckResult {
    let signal = match read_dmidecode_field(ctx, "bios-vendor") {
        Some(output) if output.success => {
            if contains_hypervisor_vendor(&output.stdout) {
                Signal::Detected
            } else {
                Signal::NotDetected
            }
        }
        _ => Signal::Inconclusive("dmidecode unavailable or permission denied"),
    };
    CheckResult {
        id: BIOS_VENDOR.id,
        name: BIOS_VENDOR.name,
        signal,
        weight: BIOS_VENDOR.weight,
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;

    #[test]
    fn detects_qemu_manufacturer_unprivileged() {
        let ev = MockLinuxEvidence::new().with_command(
            "dmidecode",
            &["-s", "system-manufacturer"],
            CommandOutput::ok("QEMU\n"),
        );
        assert_eq!((SYSTEM_MANUFACTURER.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn falls_back_to_elevated_when_unprivileged_fails() {
        // No unprivileged command registered at all, only an elevated one:
        // the check must escalate rather than reporting Inconclusive.
        let ev = MockLinuxEvidence::new().with_elevated_command(
            "dmidecode",
            &["-s", "bios-vendor"],
            CommandOutput::ok("QEMU\n"),
        );
        assert_eq!((BIOS_VENDOR.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn real_hardware_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_command(
            "dmidecode",
            &["-s", "system-manufacturer"],
            CommandOutput::ok("Dell Inc.\n"),
        );
        assert_eq!((SYSTEM_MANUFACTURER.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn real_bios_vendor_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_command(
            "dmidecode",
            &["-s", "bios-vendor"],
            CommandOutput::ok("Dell Inc.\n"),
        );
        assert_eq!((BIOS_VENDOR.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn missing_dmidecode_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (SYSTEM_MANUFACTURER.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
