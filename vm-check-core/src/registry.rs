use crate::check::Check;

/// The single dispatch point selecting the platform-appropriate check list at
/// compile time. Besides `evidence.rs` and module wiring in `checks/mod.rs`,
/// this is the only place inside `vm-check-core` that needs to know which OS
/// it's built for.
pub fn all_checks() -> Vec<Check> {
    #[cfg(target_os = "linux")]
    {
        crate::checks::linux::all()
    }
    #[cfg(target_os = "windows")]
    {
        crate::checks::windows::all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_to_the_platform_check_list() {
        #[cfg(target_os = "linux")]
        assert_eq!(all_checks().len(), crate::checks::linux::all().len());
        #[cfg(target_os = "windows")]
        assert_eq!(all_checks().len(), crate::checks::windows::all().len());
    }
}
