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
