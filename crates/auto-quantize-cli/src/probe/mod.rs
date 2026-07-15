//! Platform hardware probing: dispatches to a real per-OS backend.

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod fallback;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use auto_quantize_core::HardwareProfile;

/// Probe the local machine's memory resources.
pub fn probe() -> HardwareProfile {
    #[cfg(target_os = "linux")]
    {
        linux::probe()
    }
    #[cfg(target_os = "macos")]
    {
        macos::probe()
    }
    #[cfg(target_os = "windows")]
    {
        windows::probe()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        fallback::probe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// docs/BACKLOG.md 1.1: hardware probing (excluding HTTP calls) must
    /// complete in well under a second.
    #[test]
    fn probe_completes_in_under_one_second() {
        let start = Instant::now();
        let _ = probe();
        assert!(
            start.elapsed().as_secs_f64() < 1.0,
            "probe() took {:?}, expected < 1s",
            start.elapsed()
        );
    }
}
