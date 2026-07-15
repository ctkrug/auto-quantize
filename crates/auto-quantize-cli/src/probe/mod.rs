//! Platform hardware probing: dispatches to a real per-OS backend.

#[cfg(not(target_os = "linux"))]
mod fallback;
#[cfg(target_os = "linux")]
mod linux;

use auto_quantize_core::HardwareProfile;

/// Probe the local machine's memory resources.
pub fn probe() -> HardwareProfile {
    #[cfg(target_os = "linux")]
    {
        linux::probe()
    }
    #[cfg(not(target_os = "linux"))]
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
