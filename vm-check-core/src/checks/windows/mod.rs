pub mod disk_model;
pub mod mac_oui;
pub mod registry_bios;
pub mod registry_scsi;
pub mod wmi_bios;
pub mod wmi_computer_system;

use crate::check::Check;

/// The full set of Windows checks, in the order they're run and displayed.
pub fn all() -> Vec<Check> {
    vec![
        wmi_computer_system::WMI_COMPUTER_SYSTEM_MANUFACTURER,
        wmi_bios::WMI_BIOS_INFO,
        mac_oui::MAC_OUI_HYPERVISOR,
        disk_model::DISK_MODEL_SIGNATURE,
        registry_bios::REGISTRY_BIOS_STRINGS,
        registry_scsi::REGISTRY_SCSI_DISK_SIGNATURE,
    ]
}
