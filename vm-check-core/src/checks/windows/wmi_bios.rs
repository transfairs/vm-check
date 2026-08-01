use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::Ctx;

const VENDOR_NEEDLES: &[&str] = &[
    "vmware",
    "virtualbox",
    "qemu",
    "kvm",
    "xen",
    "hyper-v",
    "amazon",
    "google",
];

pub const WMI_BIOS_INFO: Check = Check {
    id: "wmi_bios_info",
    name: "BIOS manufacturer/version (WMI)",
    description: "Checks Win32_BIOS manufacturer and version strings for known hypervisor vendors.",
    weight: 0.7,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "BIOS-Hersteller/-Version (WMI)",
        description:
            "Prüft Win32_BIOS-Hersteller- und Versionsangaben auf bekannte Hypervisor-Anbieter.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.wmi_bios() {
        Ok(info) => {
            let combined = format!("{} {}", info.manufacturer, info.version).to_ascii_lowercase();
            match VENDOR_NEEDLES
                .iter()
                .find(|needle| combined.contains(**needle))
            {
                Some(needle) => {
                    detail = Some(format!("matched '{needle}' in BIOS info"));
                    Signal::Detected
                }
                None => Signal::NotDetected,
            }
        }
        Err(_) => Signal::Inconclusive("WMI query failed"),
    };
    CheckResult {
        id: WMI_BIOS_INFO.id,
        name: WMI_BIOS_INFO.name,
        signal,
        weight: WMI_BIOS_INFO.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockWindowsEvidence;

    #[test]
    fn detects_qemu_bios() {
        let ev = MockWindowsEvidence::new().with_bios("QEMU", "0.0.0", "0");
        assert_eq!((WMI_BIOS_INFO.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn real_bios_is_not_detected() {
        let ev = MockWindowsEvidence::new().with_bios("Dell Inc.", "2.15.0", "ABC123");
        assert_eq!((WMI_BIOS_INFO.run)(&ev).signal, Signal::NotDetected);
    }
}
