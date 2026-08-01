//! Pure classification of hypervisor-assigned MAC address OUI prefixes.
//! OS-specific modules enumerate the host's MAC addresses and call `classify`;
//! this module itself does no I/O so it needs no `Evidence` trait and is
//! trivially unit-testable.

// Well-known hypervisor-assigned OUI blocks; not exhaustive, and a non-match
// doesn't rule out virtualization (e.g. bridged/passthrough NICs keep the
// physical vendor's OUI).
const KNOWN_OUIS: &[(&str, &str)] = &[
    ("08:00:27", "VirtualBox"),
    ("0A:00:27", "VirtualBox"),
    ("00:0C:29", "VMware"),
    ("00:50:56", "VMware"),
    ("00:05:69", "VMware"),
    ("00:1C:14", "VMware"),
    ("52:54:00", "QEMU/KVM"),
    ("00:16:3E", "Xen"),
    ("00:15:5D", "Hyper-V"),
];

/// Returns the hypervisor name if `mac` starts with a known virtualization OUI.
pub fn classify(mac: &str) -> Option<&'static str> {
    let normalized = mac.trim().to_ascii_uppercase();
    KNOWN_OUIS
        .iter()
        .find(|(prefix, _)| normalized.starts_with(prefix))
        .map(|(_, vendor)| *vendor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_virtualbox() {
        assert_eq!(classify("08:00:27:aa:bb:cc"), Some("VirtualBox"));
    }

    #[test]
    fn recognizes_vmware() {
        assert_eq!(classify("00:0c:29:11:22:33"), Some("VMware"));
    }

    #[test]
    fn recognizes_qemu_kvm() {
        assert_eq!(classify("52:54:00:12:34:56"), Some("QEMU/KVM"));
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(
            classify("52:54:00:12:34:56"),
            classify("52:54:00:12:34:56".to_uppercase().as_str())
        );
    }

    #[test]
    fn rejects_ordinary_mac() {
        assert_eq!(classify("dc:a6:32:aa:bb:cc"), None);
    }
}
