//! Pure classification of hypervisor-signature disk model strings.
//! OS-specific modules enumerate the host's disk model strings and call
//! `classify`; this module does no I/O and is trivially unit-testable.

// Well-known virtual-disk model substrings; not exhaustive, and a non-match
// doesn't rule out virtualization (e.g. a passthrough/raw physical disk).
const KNOWN_SIGNATURES: &[(&str, &str)] = &[
    ("VBOX", "VirtualBox"),
    ("VMWARE VIRTUAL", "VMware"),
    ("VMWARE", "VMware"),
    ("QEMU", "QEMU/KVM"),
    ("MSFT VIRTUAL DISK", "Hyper-V"),
    ("VIRTUAL HD", "Hyper-V"),
    ("XEN", "Xen"),
];

/// Returns the hypervisor name if `model` contains a known virtual-disk signature.
pub fn classify(model: &str) -> Option<&'static str> {
    let normalized = model.trim().to_ascii_uppercase();
    KNOWN_SIGNATURES
        .iter()
        .find(|(signature, _)| normalized.contains(signature))
        .map(|(_, vendor)| *vendor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_virtualbox() {
        assert_eq!(classify("VBOX HARDDISK"), Some("VirtualBox"));
    }

    #[test]
    fn recognizes_qemu() {
        assert_eq!(classify("QEMU HARDDISK"), Some("QEMU/KVM"));
    }

    #[test]
    fn recognizes_vmware() {
        assert_eq!(classify("VMware Virtual disk"), Some("VMware"));
    }

    #[test]
    fn recognizes_hyperv() {
        assert_eq!(classify("Msft Virtual Disk"), Some("Hyper-V"));
    }

    #[test]
    fn rejects_ordinary_disk() {
        assert_eq!(classify("Samsung SSD 980 PRO 1TB"), None);
    }
}
