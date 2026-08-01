use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::{Ctx, RegHive};

const VENDOR_NEEDLES: &[&str] = &[
    "vmware",
    "virtualbox",
    "qemu",
    "kvm",
    "xen",
    "vbox",
    "bochs",
];
const REGISTRY_PATH: &str = r"HARDWARE\DESCRIPTION\System";

pub const REGISTRY_BIOS_STRINGS: Check = Check {
    id: "registry_bios_strings",
    name: "BIOS strings (registry)",
    description: r"Checks HKLM\HARDWARE\DESCRIPTION\System BIOS version/vendor strings for hypervisor signatures.",
    weight: 0.6,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "BIOS-Zeichenketten (Registry)",
        description: r"Prüft HKLM\HARDWARE\DESCRIPTION\System auf BIOS-Version/Hersteller-Zeichenketten mit Hypervisor-Signaturen.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    // Stays true only if *none* of the three values are readable; a single
    // readable-but-non-matching value is enough to conclude NotDetected even
    // if the other two fail.
    let mut inconclusive = true;
    let mut detected = false;
    for value_name in ["SystemBiosVersion", "VideoBiosVersion", "SystemBiosDate"] {
        if let Ok(value) = ctx.registry_value(RegHive::LocalMachine, REGISTRY_PATH, value_name) {
            inconclusive = false;
            let lower = value.to_ascii_lowercase();
            if let Some(needle) = VENDOR_NEEDLES
                .iter()
                .find(|needle| lower.contains(**needle))
            {
                detail = Some(format!("{value_name} matched '{needle}'"));
                detected = true;
                break;
            }
        }
    }
    let signal = if detected {
        Signal::Detected
    } else if inconclusive {
        Signal::Inconclusive("registry keys not readable")
    } else {
        Signal::NotDetected
    };
    CheckResult {
        id: REGISTRY_BIOS_STRINGS.id,
        name: REGISTRY_BIOS_STRINGS.name,
        signal,
        weight: REGISTRY_BIOS_STRINGS.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockWindowsEvidence;

    #[test]
    fn detects_vbox_in_bios_version() {
        let ev = MockWindowsEvidence::new().with_registry_value(
            REGISTRY_PATH,
            "SystemBiosVersion",
            "VBOX   -1",
        );
        assert_eq!((REGISTRY_BIOS_STRINGS.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn real_bios_is_not_detected() {
        let ev = MockWindowsEvidence::new().with_registry_value(
            REGISTRY_PATH,
            "SystemBiosVersion",
            "DELL   - 1072009",
        );
        assert_eq!((REGISTRY_BIOS_STRINGS.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn unreadable_registry_is_inconclusive() {
        let ev = MockWindowsEvidence::new();
        assert!(matches!(
            (REGISTRY_BIOS_STRINGS.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
