//! OS-agnostic classification helpers shared by the linux/windows checks:
//! given a MAC address or disk model string already enumerated by the
//! platform-specific evidence source, decide whether it matches a known
//! hypervisor signature.

pub mod disk_model;
pub mod mac_oui;
