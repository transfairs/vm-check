use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::checks::common::mac_oui::classify;
use crate::evidence::Ctx;

pub const MAC_OUI_HYPERVISOR: Check = Check {
    id: "mac_oui_hypervisor",
    name: "Hypervisor MAC address prefix",
    description: "Checks network interface MAC addresses for known hypervisor OUI prefixes.",
    weight: 0.7,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "Hypervisor-MAC-Adresspräfix",
        description:
            "Prüft MAC-Adressen der Netzwerkschnittstellen auf bekannte Hypervisor-OUI-Präfixe.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.network_mac_addresses() {
        Ok(macs) if !macs.is_empty() => {
            match macs
                .iter()
                .find_map(|mac| classify(mac).map(|vendor| (mac, vendor)))
            {
                Some((mac, vendor)) => {
                    detail = Some(format!("{mac} ({vendor})"));
                    Signal::Detected
                }
                None => Signal::NotDetected,
            }
        }
        Ok(_) => Signal::Inconclusive("no network interfaces found"),
        Err(_) => Signal::Inconclusive("could not enumerate network interfaces"),
    };
    CheckResult {
        id: MAC_OUI_HYPERVISOR.id,
        name: MAC_OUI_HYPERVISOR.name,
        signal,
        weight: MAC_OUI_HYPERVISOR.weight,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;

    #[test]
    fn detects_virtualbox_mac() {
        let ev = MockLinuxEvidence::new().with_mac("08:00:27:aa:bb:cc");
        let result = (MAC_OUI_HYPERVISOR.run)(&ev);
        assert_eq!(result.signal, Signal::Detected);
        assert!(result.detail.unwrap().contains("VirtualBox"));
    }

    #[test]
    fn ordinary_mac_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_mac("dc:a6:32:aa:bb:cc");
        assert_eq!((MAC_OUI_HYPERVISOR.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn no_interfaces_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (MAC_OUI_HYPERVISOR.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }

    #[test]
    fn enumeration_failure_is_inconclusive() {
        let ev = MockLinuxEvidence::new().with_mac_addresses_error();
        assert!(matches!(
            (MAC_OUI_HYPERVISOR.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
