//! Concrete virtualization-detection heuristics, grouped by which platform
//! they apply to. `linux`/`windows` each expose an `all()` returning that
//! platform's full, ordered [`Check`](crate::check::Check) list (order
//! determines CLI/GUI display order); `common` holds OS-agnostic
//! classification helpers shared by both.

pub mod common;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;
