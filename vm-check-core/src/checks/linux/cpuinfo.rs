use crate::check::{Check, CheckResult, Language, Privilege, Signal, Translation};
use crate::evidence::Ctx;

// The original script's TEST_1 and TEST_5 both grepped `/proc/cpuinfo` with
// near-identical patterns (`hypervisor|vmware|kvm` vs plain `hypervisor`),
// accidental duplication rather than intentional design. Here they become two
// explicitly distinct checks: a precise flag check and a broader vendor scan.

pub const HYPERVISOR_FLAG: Check = Check {
    id: "cpuinfo_hypervisor_flag",
    name: "CPU hypervisor flag in /proc/cpuinfo",
    description:
        "Checks the CPU 'flags' line for the 'hypervisor' feature bit, which real CPUs never set.",
    weight: 0.8,
    privilege: Privilege::None,
    run: run_hypervisor_flag,
    translations: &[Translation {
        language: Language::De,
        name: "CPU-Hypervisor-Flag in /proc/cpuinfo",
        description: "Prüft die 'flags'-Zeile der CPU auf das Feature-Bit 'hypervisor', das echte CPUs nie setzen.",
    }],
};

fn run_hypervisor_flag(ctx: Ctx) -> CheckResult {
    let signal = match ctx.read_file("/proc/cpuinfo") {
        Ok(content) => {
            let has_flag = content
                .lines()
                .filter(|line| line.starts_with("flags"))
                .any(|line| line.contains("hypervisor"));
            if has_flag {
                Signal::Detected
            } else {
                Signal::NotDetected
            }
        }
        Err(_) => Signal::Inconclusive("/proc/cpuinfo not readable"),
    };
    CheckResult {
        id: HYPERVISOR_FLAG.id,
        name: HYPERVISOR_FLAG.name,
        signal,
        weight: HYPERVISOR_FLAG.weight,
        detail: None,
    }
}

pub const VENDOR_STRINGS: Check = Check {
    id: "cpuinfo_vendor_strings",
    name: "Hypervisor vendor strings in /proc/cpuinfo",
    description: "Scans /proc/cpuinfo for vendor substrings such as 'vmware' or 'kvm'.",
    weight: 0.6,
    privilege: Privilege::None,
    run: run_vendor_strings,
    translations: &[Translation {
        language: Language::De,
        name: "Hypervisor-Herstellerzeichenketten in /proc/cpuinfo",
        description: "Durchsucht /proc/cpuinfo nach Herstellersubstrings wie 'vmware' oder 'kvm'.",
    }],
};

fn run_vendor_strings(ctx: Ctx) -> CheckResult {
    let signal = match ctx.read_file("/proc/cpuinfo") {
        Ok(content) => {
            let lower = content.to_ascii_lowercase();
            if lower.contains("vmware") || lower.contains("kvm") {
                Signal::Detected
            } else {
                Signal::NotDetected
            }
        }
        Err(_) => Signal::Inconclusive("/proc/cpuinfo not readable"),
    };
    CheckResult {
        id: VENDOR_STRINGS.id,
        name: VENDOR_STRINGS.name,
        signal,
        weight: VENDOR_STRINGS.weight,
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::testing::MockLinuxEvidence;

    #[test]
    fn detects_hypervisor_flag() {
        let ev = MockLinuxEvidence::new().with_file(
            "/proc/cpuinfo",
            "processor: 0\nflags: fpu vme de pse hypervisor\n",
        );
        assert_eq!((HYPERVISOR_FLAG.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn no_hypervisor_flag_is_not_detected() {
        let ev = MockLinuxEvidence::new().with_file("/proc/cpuinfo", "flags: fpu vme de pse\n");
        assert_eq!((HYPERVISOR_FLAG.run)(&ev).signal, Signal::NotDetected);
    }

    #[test]
    fn vendor_string_kvm_is_detected() {
        let ev = MockLinuxEvidence::new()
            .with_file("/proc/cpuinfo", "model name: Common KVM processor\n");
        assert_eq!((VENDOR_STRINGS.run)(&ev).signal, Signal::Detected);
    }

    #[test]
    fn vendor_strings_missing_cpuinfo_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (VENDOR_STRINGS.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }

    #[test]
    fn missing_cpuinfo_is_inconclusive() {
        let ev = MockLinuxEvidence::new();
        assert!(matches!(
            (HYPERVISOR_FLAG.run)(&ev).signal,
            Signal::Inconclusive(_)
        ));
    }
}
