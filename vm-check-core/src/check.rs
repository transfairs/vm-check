use serde::{Deserialize, Serialize};

/// A UI language a [`Check`]'s name/description can be localized into.
/// Add a variant here and matching [`Translation`] entries on the relevant
/// checks to support another language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    En,
    De,
}

/// A translated name/description for a [`Check`], looked up by
/// [`Check::localized_name`]/[`Check::localized_description`].
#[derive(Debug, Clone, Copy)]
pub struct Translation {
    pub language: Language,
    pub name: &'static str,
    pub description: &'static str,
}

/// What a single check concluded.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Signal {
    /// Evidence of virtualization was found.
    Detected,
    /// The check ran successfully and found no evidence of virtualization.
    NotDetected,
    /// The check could not produce a result (missing tool, no permission, not applicable).
    Inconclusive(&'static str),
}

/// Whether a check needs elevated privileges to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Privilege {
    /// Runs fine as an ordinary user.
    None,
    /// Needs root/administrator rights (e.g. `dmidecode`, `dmesg`); callers
    /// decide whether to run these at all, see [`Privilege`] usage in the
    /// CLI/GUI's "include privileged checks" option.
    Elevated,
}

/// The outcome of running a single [`Check`], carrying its own copy of the
/// check's static metadata (`id`/`name`/`weight`) so a result is
/// self-contained and doesn't need the originating `Check` kept around.
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    /// Stable, machine-readable identifier matching [`Check::id`]; used to
    /// look up localized text or deduplicate results, unlike `name` which is
    /// for display and may be localized.
    pub id: &'static str,
    /// Display name, matching [`Check::name`] at the time this check ran.
    pub name: &'static str,
    pub signal: Signal,
    /// How much this result should count toward [`crate::report::Report::confidence`].
    pub weight: f32,
    /// Extra detail for the CLI's `--verbose` output, e.g. the matched string;
    /// not currently surfaced in the GUI.
    pub detail: Option<String>,
}

/// A single virtualization-detection heuristic.
///
/// `run` is a plain fn pointer rather than `Box<dyn Fn>`: checks are stateless
/// and only need the evidence context passed in, so there is nothing to capture.
pub struct Check {
    /// Stable, machine-readable identifier, e.g. `"cpuinfo_hypervisor_flag"`.
    pub id: &'static str,
    /// English display name; see [`Check::localized_name`] for other languages.
    pub name: &'static str,
    /// English one-line explanation of what the check inspects and why;
    /// see [`Check::localized_description`] for other languages.
    pub description: &'static str,
    /// How much this check should count toward [`crate::report::Report::confidence`]
    /// relative to the other checks that ran.
    pub weight: f32,
    pub privilege: Privilege,
    pub run: fn(crate::evidence::Ctx) -> CheckResult,
    /// Non-English translations of `name`/`description`. Empty means
    /// English-only; [`Check::localized_name`]/[`Check::localized_description`]
    /// fall back to the English fields for any language not listed here.
    pub translations: &'static [Translation],
}

impl Check {
    /// `name` translated into `language`, falling back to English.
    pub fn localized_name(&self, language: Language) -> &'static str {
        self.translation(language).map_or(self.name, |t| t.name)
    }

    /// `description` translated into `language`, falling back to English.
    pub fn localized_description(&self, language: Language) -> &'static str {
        self.translation(language)
            .map_or(self.description, |t| t.description)
    }

    fn translation(&self, language: Language) -> Option<&Translation> {
        if language == Language::En {
            return None;
        }
        self.translations.iter().find(|t| t.language == language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_with_de_translation() -> Check {
        Check {
            id: "test_check",
            name: "Test check",
            description: "Test description",
            weight: 1.0,
            privilege: Privilege::None,
            run: |_| unreachable!("translation tests never run a check"),
            translations: &[Translation {
                language: Language::De,
                name: "Testprüfung",
                description: "Testbeschreibung",
            }],
        }
    }

    #[test]
    fn english_uses_the_untranslated_fields() {
        let check = check_with_de_translation();
        assert_eq!(check.localized_name(Language::En), "Test check");
        assert_eq!(
            check.localized_description(Language::En),
            "Test description"
        );
    }

    #[test]
    fn known_language_uses_its_translation() {
        let check = check_with_de_translation();
        assert_eq!(check.localized_name(Language::De), "Testprüfung");
        assert_eq!(
            check.localized_description(Language::De),
            "Testbeschreibung"
        );
    }

    #[test]
    fn missing_translation_falls_back_to_english() {
        let check = Check {
            id: "untranslated_check",
            name: "Untranslated check",
            description: "Untranslated description",
            weight: 1.0,
            privilege: Privilege::None,
            run: |_| unreachable!("translation tests never run a check"),
            translations: &[],
        };
        assert_eq!(check.localized_name(Language::De), "Untranslated check");
        assert_eq!(
            check.localized_description(Language::De),
            "Untranslated description"
        );
    }
}
