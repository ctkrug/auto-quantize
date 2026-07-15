//! Conservative fallback probe for platforms without a real backend.
//!
//! Linux, macOS, and Windows all have real backends now (`probe::linux`,
//! `probe::macos`, `probe::windows`); this covers everything else (BSDs,
//! other Unixes, ...) with an honestly-unknown profile rather than a guess,
//! so the CLI still runs everywhere without pretending to know the hardware.

use auto_quantize_core::HardwareProfile;

pub fn probe() -> HardwareProfile {
    HardwareProfile {
        vram_bytes: None,
        ram_bytes: 0,
        ram_free_bytes: 0,
        bandwidth_gbps: None,
    }
}
