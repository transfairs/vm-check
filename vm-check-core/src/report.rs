//! Turns a batch of [`CheckResult`]s into a single weighted [`Report::confidence`]
//! score and a [`Verdict`].

use crate::check::{CheckResult, Signal};
use serde::Serialize;

/// The overall conclusion drawn from a [`Report`]'s confidence score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// Confidence at or above `VM_THRESHOLD`.
    LikelyVirtualMachine,
    /// Confidence at or below `PHYSICAL_THRESHOLD`.
    LikelyPhysicalMachine,
    /// Confidence strictly between the two thresholds, or no conclusive
    /// (non-[`Signal::Inconclusive`]) results at all.
    Uncertain,
}

// Asymmetric on purpose: a real machine can incidentally trip one or two
// low-weight virtualization checks (e.g. a MAC OUI reused by a physical NIC
// vendor), but genuine VMs tend to trip almost everything applicable to the
// platform. Requiring 66% keeps a couple of false positives from flipping
// the verdict, while 15% is lenient enough that a handful of skipped/failed
// checks on real hardware doesn't accidentally read as "uncertain".
const VM_THRESHOLD: f32 = 0.66;
const PHYSICAL_THRESHOLD: f32 = 0.15;

/// A batch of check results together with the scoring derived from them.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub results: Vec<CheckResult>,
}

impl Report {
    pub fn new(results: Vec<CheckResult>) -> Self {
        Self { results }
    }

    /// Weighted fraction of *conclusive* checks that detected virtualization.
    /// Inconclusive checks are excluded from both numerator and denominator so
    /// that skipped/unavailable checks don't dilute the signal either way.
    pub fn confidence(&self) -> f32 {
        let mut detected_weight = 0.0;
        let mut total_weight = 0.0;
        for result in &self.results {
            match result.signal {
                Signal::Detected => {
                    detected_weight += result.weight;
                    total_weight += result.weight;
                }
                Signal::NotDetected => {
                    total_weight += result.weight;
                }
                Signal::Inconclusive(_) => {}
            }
        }
        if total_weight == 0.0 {
            0.0
        } else {
            detected_weight / total_weight
        }
    }

    /// The [`Verdict`] implied by [`confidence`](Self::confidence) against
    /// `VM_THRESHOLD`/`PHYSICAL_THRESHOLD`, or [`Verdict::Uncertain`] if every
    /// result is [`Signal::Inconclusive`].
    pub fn verdict(&self) -> Verdict {
        let has_conclusive_result = self
            .results
            .iter()
            .any(|r| !matches!(r.signal, Signal::Inconclusive(_)));
        if !has_conclusive_result {
            return Verdict::Uncertain;
        }
        let confidence = self.confidence();
        if confidence >= VM_THRESHOLD {
            Verdict::LikelyVirtualMachine
        } else if confidence <= PHYSICAL_THRESHOLD {
            Verdict::LikelyPhysicalMachine
        } else {
            Verdict::Uncertain
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::Signal;

    fn result(signal: Signal, weight: f32) -> CheckResult {
        CheckResult {
            id: "test",
            name: "test",
            signal,
            weight,
            detail: None,
        }
    }

    #[test]
    fn all_detected_is_likely_vm() {
        let report = Report::new(vec![
            result(Signal::Detected, 1.0),
            result(Signal::Detected, 0.5),
        ]);
        assert_eq!(report.confidence(), 1.0);
        assert_eq!(report.verdict(), Verdict::LikelyVirtualMachine);
    }

    #[test]
    fn all_not_detected_is_likely_physical() {
        let report = Report::new(vec![
            result(Signal::NotDetected, 1.0),
            result(Signal::NotDetected, 0.5),
        ]);
        assert_eq!(report.confidence(), 0.0);
        assert_eq!(report.verdict(), Verdict::LikelyPhysicalMachine);
    }

    #[test]
    fn mixed_signals_are_uncertain() {
        let report = Report::new(vec![
            result(Signal::Detected, 1.0),
            result(Signal::NotDetected, 1.0),
        ]);
        assert_eq!(report.confidence(), 0.5);
        assert_eq!(report.verdict(), Verdict::Uncertain);
    }

    #[test]
    fn inconclusive_results_are_excluded_entirely() {
        let report = Report::new(vec![
            result(Signal::Detected, 1.0),
            result(Signal::Inconclusive("skipped"), 100.0),
        ]);
        assert_eq!(report.confidence(), 1.0);
        assert_eq!(report.verdict(), Verdict::LikelyVirtualMachine);
    }

    #[test]
    fn no_conclusive_results_is_uncertain_not_divide_by_zero() {
        let report = Report::new(vec![result(Signal::Inconclusive("skipped"), 1.0)]);
        assert_eq!(report.confidence(), 0.0);
        assert_eq!(report.verdict(), Verdict::Uncertain);
    }
}
