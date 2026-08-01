//! Platform-independent core of vm-check: a registry of virtualization-detection
//! [`checks`], the [`Check`]/[`CheckResult`] types they produce, and
//! [`Report`] scoring to turn a batch of results into a [`Verdict`].
//!
//! Platform-specific evidence gathering lives behind the [`evidence::Ctx`]
//! trait, so checks themselves stay testable without touching the real OS;
//! see the `evidence::testing` mocks used throughout `checks`.

pub mod check;
pub mod checks;
pub mod evidence;
pub mod registry;
pub mod report;

pub use check::{Check, CheckResult, Language, Privilege, Signal};
pub use registry::all_checks;
pub use report::{Report, Verdict};
