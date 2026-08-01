use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::{Ctx, RegHive};

// Registry device IDs use underscores where the plain string uses spaces
// (e.g. "Prod_Virtual_HD"), hence "virtual_hd" instead of the "VIRTUAL HD"
// used in checks::common::disk_model.
const VENDOR_NEEDLES: &[&str] = &["vmware", "vbox", "qemu", "virtual_hd", "msft virtual disk"];
const REGISTRY_PATH: &str = r"SYSTEM\CurrentControlSet\Services\Disk\Enum";

pub const REGISTRY_SCSI_DISK_SIGNATURE: Check = Check {
    id: "registry_scsi_disk_signature",
    name: "SCSI disk enumeration (registry)",
    description: r"Checks HKLM\SYSTEM\CurrentControlSet\Services\Disk\Enum for virtual disk device identifiers.",
    weight: 0.7,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "SCSI-Datenträgeraufzählung (Registry)",
        description: r"Prüft HKLM\SYSTEM\CurrentControlSet\Services\Disk\Enum auf Kennungen virtueller Datenträger.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.registry_value(RegHive::LocalMachine, REGISTRY_PATH, "0") {
        Ok(value) => {
            let lower = value.to_ascii_lowercase();
            match VENDOR_NEEDLES
                .iter()
                .find(|needle| lower.contains(**needle))
            {
                Some(needle) => {
                    detail = Some(format!("matched '{needle}'"));
                    Signal::Detected
                }
                None => Signal::NotDetected,
            }
        }
        Err(_) => Signal::Inconclusive("registry key not readable"),
    };
    CheckResult {
        id: REGISTRY_SCSI_DISK_SIGNATURE.id,
        name: REGISTRY_SCSI_DISK_SIGNATURE.name,
        signal,
        weight: REGISTRY_SCSI_DISK_SIGNATURE.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockWindowsEvidence;

    #[test]
    fn detects_vmware_disk_entry() {
        let ev = MockWindowsEvidence::new().with_registry_value(
            REGISTRY_PATH,
            "0",
            r"SCSI\Disk&Ven_VMware&Prod_Virtual_disk\5&26c4d By",
        );
        assert_eq!(
            (REGISTRY_SCSI_DISK_SIGNATURE.run)(&ev).signal,
            Signal::Detected
        );
    }

    #[test]
    fn real_disk_is_not_detected() {
        let ev = MockWindowsEvidence::new().with_registry_value(
            REGISTRY_PATH,
            "0",
            r"SCSI\Disk&Ven_Samsung&Prod_SSD_980_PRO\4&1a2b3c",
        );
        assert_eq!(
            (REGISTRY_SCSI_DISK_SIGNATURE.run)(&ev).signal,
            Signal::NotDetected
        );
    }
}
