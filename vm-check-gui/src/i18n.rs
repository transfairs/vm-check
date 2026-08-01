use serde::{Deserialize, Serialize};
use vm_check_core::{Language, Verdict};

/// The user's language choice: either follow the OS or pin a specific
/// [`Language`]. Adding a language only requires a new [`Language`] variant
/// in vm-check-core plus a match arm here and in [`strings`]/[`verdict_summary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguagePreference {
    System,
    Fixed(Language),
}

impl LanguagePreference {
    pub fn resolve(self, system_language: Language) -> Language {
        match self {
            LanguagePreference::System => system_language,
            LanguagePreference::Fixed(language) => language,
        }
    }
}

/// Best-effort detection of the OS UI language, falling back to English for
/// anything unrecognized or undetectable.
pub fn detect_system_language() -> Language {
    language_from_locale(sys_locale::get_locale())
}

/// The actual locale → [`Language`] mapping, split out from
/// [`detect_system_language`] so it's exercisable without depending on (or
/// mocking) `sys_locale::get_locale`'s real OS call: this is a pure
/// function of whatever locale string the OS would have reported.
fn language_from_locale(locale: Option<String>) -> Language {
    locale
        .and_then(|locale| match locale.split(['-', '_']).next()? {
            "de" => Some(Language::De),
            _ => None,
        })
        .unwrap_or(Language::En)
}

/// Static GUI chrome text for a given language (everything except the
/// per-check names/descriptions, which come from `Check::localized_name`/
/// `Check::localized_description`, and the verdict summary, which needs
/// runtime interpolation, see [`verdict_summary`]).
pub struct Strings {
    pub subtitle: &'static str,
    /// Only read on Linux, and only when not running as root: see the
    /// `#[cfg(target_os = "linux")]` block in `app.rs`.
    #[cfg(target_os = "linux")]
    pub privileged_hint: &'static str,
    pub run_checks: &'static str,
    pub running: &'static str,
    pub run_again: &'static str,
    pub column_check: &'static str,
    pub column_result: &'static str,
    pub badge_detected: &'static str,
    pub badge_not_detected: &'static str,
    pub badge_inconclusive: &'static str,
    pub theme_label: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub theme_system: &'static str,
    pub language_label: &'static str,
    pub language_system: &'static str,
    pub about_label: &'static str,
    pub about_window_title: &'static str,
    pub about_free_software: &'static str,
    pub about_project_page: &'static str,
}

pub fn strings(language: Language) -> Strings {
    match language {
        Language::En => Strings {
            subtitle: "Checks whether this system is running inside a virtual machine.",
            #[cfg(target_os = "linux")]
            privileged_hint: "Privileged checks (dmesg, dmidecode) are skipped; relaunch this app with sudo to include them.",
            run_checks: "Run checks",
            running: "Running…",
            run_again: "Run again",
            column_check: "Check",
            column_result: "Result",
            badge_detected: "FAIL",
            badge_not_detected: "PASS",
            badge_inconclusive: "SKIP",
            theme_label: "Theme:",
            theme_light: "Light",
            theme_dark: "Dark",
            theme_system: "System",
            language_label: "Language:",
            language_system: "System",
            about_label: "About",
            about_window_title: "About vm-check",
            about_free_software: "Free software, licensed under the GNU GPL v3 (or later).",
            about_project_page: "Project page",
        },
        Language::De => Strings {
            subtitle: "Prüft, ob dieses System in einer virtuellen Maschine läuft.",
            #[cfg(target_os = "linux")]
            privileged_hint: "Privilegierte Prüfungen (dmesg, dmidecode) werden übersprungen. App mit sudo neu starten, um sie einzubeziehen.",
            run_checks: "Prüfung starten",
            running: "Läuft…",
            run_again: "Wiederholen",
            column_check: "Prüfung",
            column_result: "Ergebnis",
            badge_detected: "GEFUNDEN",
            badge_not_detected: "OK",
            badge_inconclusive: "ÜBERSPRUNGEN",
            theme_label: "Design:",
            theme_light: "Hell",
            theme_dark: "Dunkel",
            theme_system: "System",
            language_label: "Sprache:",
            language_system: "System",
            about_label: "Über",
            about_window_title: "Über vm-check",
            about_free_software: "Freie Software, lizenziert unter der GNU GPL v3 (oder später).",
            about_project_page: "Projektseite",
        },
    }
}

pub fn verdict_summary(verdict: Verdict, confidence_pct: f32, language: Language) -> String {
    match (verdict, language) {
        (Verdict::LikelyVirtualMachine, Language::En) => {
            format!("Likely running inside a virtual machine (VM likelihood {confidence_pct}%)")
        }
        (Verdict::LikelyVirtualMachine, Language::De) => {
            format!(
                "Wahrscheinlich in einer virtuellen Maschine (VM-Wahrscheinlichkeit {confidence_pct}%)"
            )
        }
        (Verdict::LikelyPhysicalMachine, Language::En) => {
            format!("Likely NOT running inside a virtual machine (VM likelihood {confidence_pct}%)")
        }
        (Verdict::LikelyPhysicalMachine, Language::De) => {
            format!(
                "Wahrscheinlich NICHT in einer virtuellen Maschine (VM-Wahrscheinlichkeit {confidence_pct}%)"
            )
        }
        (Verdict::Uncertain, Language::En) => {
            format!("Uncertain whether this is a virtual machine (VM likelihood {confidence_pct}%)")
        }
        (Verdict::Uncertain, Language::De) => {
            format!(
                "Unklar, ob dies eine virtuelle Maschine ist (VM-Wahrscheinlichkeit {confidence_pct}%)"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_system_uses_the_system_language() {
        assert_eq!(
            LanguagePreference::System.resolve(Language::De),
            Language::De
        );
        assert_eq!(
            LanguagePreference::System.resolve(Language::En),
            Language::En
        );
    }

    #[test]
    fn resolve_fixed_ignores_the_system_language() {
        assert_eq!(
            LanguagePreference::Fixed(Language::De).resolve(Language::En),
            Language::De
        );
    }

    #[test]
    fn language_from_locale_recognizes_german_variants() {
        assert_eq!(language_from_locale(Some("de".to_string())), Language::De);
        assert_eq!(
            language_from_locale(Some("de_DE".to_string())),
            Language::De
        );
        assert_eq!(
            language_from_locale(Some("de-AT".to_string())),
            Language::De
        );
    }

    #[test]
    fn language_from_locale_falls_back_to_english() {
        assert_eq!(language_from_locale(None), Language::En);
        assert_eq!(
            language_from_locale(Some("en_US".to_string())),
            Language::En
        );
        assert_eq!(
            language_from_locale(Some("fr_FR".to_string())),
            Language::En
        );
        assert_eq!(language_from_locale(Some(String::new())), Language::En);
    }

    #[test]
    fn detect_system_language_does_not_panic() {
        // Whatever the test runner's actual locale is, this must return
        // *some* valid Language without panicking.
        let _ = detect_system_language();
    }

    #[test]
    fn strings_are_provided_for_every_language() {
        for language in [Language::En, Language::De] {
            let s = strings(language);
            assert!(!s.subtitle.is_empty());
            assert!(!s.run_checks.is_empty());
            assert!(!s.badge_detected.is_empty());
            assert!(!s.about_window_title.is_empty());
        }
    }

    #[test]
    fn verdict_summary_covers_every_verdict_and_language() {
        for verdict in [
            Verdict::LikelyVirtualMachine,
            Verdict::LikelyPhysicalMachine,
            Verdict::Uncertain,
        ] {
            for language in [Language::En, Language::De] {
                let summary = verdict_summary(verdict, 42.0, language);
                assert!(summary.contains("42"));
            }
        }
    }
}
