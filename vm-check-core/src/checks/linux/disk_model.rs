use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::checks::common::disk_model::classify;
use crate::evidence::Ctx;

pub const DISK_MODEL_SIGNATURE: Check = Check {
    id: "disk_model_signature",
    name: "Hypervisor disk model signature",
    description: "Checks block device model strings for known virtual-disk signatures.",
    weight: 0.7,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "Hypervisor-Datenträgermodellsignatur",
        description:
            "Prüft Blockgerätemodellbezeichnungen auf bekannte Signaturen virtueller Datenträger.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.disk_models() {
        Ok(models) if !models.is_empty() => {
            match models
                .iter()
                .find_map(|model| classify(model).map(|vendor| (model, vendor)))
            {
                Some((model, vendor)) => {
                    detail = Some(format!("{model} ({vendor})"));
                    Signal::Detected
                }
                None => Signal::NotDetected,
            }
        }
        Ok(_) => Signal::Inconclusive("no block devices found"),
        Err(_) => Signal::Inconclusive("could not enumerate block devices"),
    };
    CheckResult {
        id: DISK_MODEL_SIGNATURE.id,
        name: DISK_MODEL_SIGNATURE.name,
        signal,
        weight: DISK_MODEL_SIGNATURE.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;

    #[test]
    fn detects_qemu_disk() {
        let ev = MockLinuxEvidence::new().with_disk_model("QEMU HARDDISK");
        let result = (DISK_MODEL_SIGNATURE.run)(&ev);
        assert_eq!(result.signal, Signal::Detected);
        assert!(result.detail.unwrap().contains("QEMU/KVM"));
    }

    #[test]
    fn ordinary_disk_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_disk_model("Samsung SSD 980 PRO 1TB");
        assert_eq!((DISK_MODEL_SIGNATURE.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn no_disks_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (DISK_MODEL_SIGNATURE.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }

    #[test]
    fn enumeration_failure_is_inconclusive() {
        let ev = MockLinuxEvidence::new().with_disk_models_error();
        assert!(matches!(
            (DISK_MODEL_SIGNATURE.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
