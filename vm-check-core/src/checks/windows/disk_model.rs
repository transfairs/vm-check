use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::checks::common::disk_model::classify;
use crate::evidence::Ctx;

pub const DISK_MODEL_SIGNATURE: Check = Check {
    id: "disk_model_signature",
    name: "Hypervisor disk model signature",
    description:
        "Checks disk drive model strings (WMI Win32_DiskDrive) for known virtual-disk signatures.",
    weight: 0.7,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "Hypervisor-Datenträgermodellsignatur",
        description: "Prüft Datenträgermodellbezeichnungen (WMI Win32_DiskDrive) auf bekannte Signaturen virtueller Datenträger.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.wmi_disk_models() {
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
        Ok(_) => Signal::Inconclusive("no disk drives found"),
        Err(_) => Signal::Inconclusive("WMI query failed"),
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
    use crate::evidence::testing::MockWindowsEvidence;

    #[test]
    fn detects_vbox_disk() {
        let ev = MockWindowsEvidence::new().with_disk_model("VBOX HARDDISK");
        assert_eq!((DISK_MODEL_SIGNATURE.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn ordinary_disk_is_not_detected() {
        let ev = MockWindowsEvidence::new().with_disk_model("Samsung SSD 980 PRO 1TB");
        assert_eq!((DISK_MODEL_SIGNATURE.run)(&ev).signal, Signal::NotDetected);
    }
}
