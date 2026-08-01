pub mod cpuinfo;
pub mod disk_model;
pub mod dmesg;
pub mod dmidecode;
pub mod dpkg_tools;
pub mod kernel_modules;
pub mod mac_oui;
pub mod systemd_detect_virt;

use crate::check::Check;

/// The full set of Linux checks, in the order they're run and displayed.
pub fn all() -> Vec<Check> {
    vec![
        systemd_detect_virt::SYSTEMD_DETECT_VIRT,
        cpuinfo::HYPERVISOR_FLAG,
        cpuinfo::VENDOR_STRINGS,
        mac_oui::MAC_OUI_HYPERVISOR,
        disk_model::DISK_MODEL_SIGNATURE,
        dmidecode::SYSTEM_MANUFACTURER,
        dmidecode::BIOS_VENDOR,
        kernel_modules::KERNEL_MODULE_SIGNATURE,
        dpkg_tools::DPKG_GUEST_TOOLS,
        dmesg::DMESG_HYPERVISOR_MODULES,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_checks_have_unique_ids() {
        let checks = all();
        assert!(!checks.is_empty());
        let ids: HashSet<_> = checks.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), checks.len());
    }
}
