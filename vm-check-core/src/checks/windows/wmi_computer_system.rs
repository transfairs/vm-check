use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::Ctx;

/// Checks `Win32_ComputerSystem.Model` for the literal "Virtual Machine" string
/// that Hyper-V reports, rather than a bare `Manufacturer == "Microsoft
/// Corporation"` substring match, since real Surface/Microsoft-branded hardware
/// also reports that manufacturer, so the model field is the reliable signal.
pub const WMI_COMPUTER_SYSTEM_MANUFACTURER: Check = Check {
    id: "wmi_computer_system_manufacturer",
    name: "Computer system manufacturer/model (WMI)",
    description: "Checks Win32_ComputerSystem for hypervisor vendor names or a Hyper-V 'Virtual Machine' model.",
    weight: 0.8,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "Hersteller/Modell des Computersystems (WMI)",
        description: "Prüft Win32_ComputerSystem auf Hypervisor-Herstellernamen oder das Hyper-V-Modell 'Virtual Machine'.",
    }],
};

const VENDOR_NEEDLES: &[&str] = &["vmware", "virtualbox", "qemu", "kvm", "xen"];

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.wmi_computer_system() {
        Ok(info) => {
            let manufacturer_lower = info.manufacturer.to_ascii_lowercase();
            if VENDOR_NEEDLES
                .iter()
                .any(|needle| manufacturer_lower.contains(needle))
            {
                detail = Some(info.manufacturer.clone());
                Signal::Detected
            } else if info.model.eq_ignore_ascii_case("Virtual Machine") {
                detail = Some("Hyper-V (Model = Virtual Machine)".to_string());
                Signal::Detected
            } else {
                Signal::NotDetected
            }
        }
        Err(_) => Signal::Inconclusive("WMI query failed"),
    };
    CheckResult {
        id: WMI_COMPUTER_SYSTEM_MANUFACTURER.id,
        name: WMI_COMPUTER_SYSTEM_MANUFACTURER.name,
        signal,
        weight: WMI_COMPUTER_SYSTEM_MANUFACTURER.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockWindowsEvidence;

    #[test]
    fn detects_hyperv_via_model() {
        let ev = MockWindowsEvidence::new()
            .with_computer_system("Microsoft Corporation", "Virtual Machine");
        assert_eq!(
            (WMI_COMPUTER_SYSTEM_MANUFACTURER.run)(&ev).signal,
            Signal::Detected
        );
    }

    #[test]
    fn real_surface_hardware_is_not_detected() {
        let ev = MockWindowsEvidence::new()
            .with_computer_system("Microsoft Corporation", "Surface Laptop 5");
        assert_eq!(
            (WMI_COMPUTER_SYSTEM_MANUFACTURER.run)(&ev).signal,
            Signal::NotDetected
        );
    }

    #[test]
    fn detects_vmware_manufacturer() {
        let ev = MockWindowsEvidence::new()
            .with_computer_system("VMware, Inc.", "VMware Virtual Platform");
        assert_eq!(
            (WMI_COMPUTER_SYSTEM_MANUFACTURER.run)(&ev).signal,
            Signal::Detected
        );
    }
}
