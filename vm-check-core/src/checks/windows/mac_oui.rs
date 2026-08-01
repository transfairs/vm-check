use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::checks::common::mac_oui::classify;
use crate::evidence::Ctx;

pub const MAC_OUI_HYPERVISOR: Check = Check {
    id: "mac_oui_hypervisor",
    name: "Hypervisor MAC address prefix",
    description: "Checks network adapter MAC addresses (WMI) for known hypervisor OUI prefixes.",
    weight: 0.7,
    privilege: Privilege::None,
    run: run_check,
    translations: &[Translation {
        language: Language::De,
        name: "Hypervisor-MAC-Adresspräfix",
        description:
            "Prüft MAC-Adressen der Netzwerkadapter (WMI) auf bekannte Hypervisor-OUI-Präfixe.",
    }],
};

fn run_check(ctx: Ctx) -> CheckResult {
    let mut detail = None;
    let signal = match ctx.wmi_mac_addresses() {
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
        Ok(_) => Signal::Inconclusive("no network adapters found"),
        Err(_) => Signal::Inconclusive("WMI query failed"),
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
    use crate::evidence::testing::MockWindowsEvidence;

    #[test]
    fn detects_hyperv_mac() {
        let ev = MockWindowsEvidence::new().with_mac("00:15:5D:aa:bb:cc");
        assert_eq!((MAC_OUI_HYPERVISOR.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn ordinary_mac_is_not_detected() {
        let ev = MockWindowsEvidence::new().with_mac("dc:a6:32:aa:bb:cc");
        assert_eq!((MAC_OUI_HYPERVISOR.run)(&ev).signal, Signal::NotDetected);
    }
}
