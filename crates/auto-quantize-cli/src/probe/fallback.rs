//! Conservative fallback probe for platforms without a real backend yet.
//!
//! Real macOS (`sysctl`/`system_profiler`) and Windows (`GlobalMemoryStatusEx`/
//! DXGI) probing are tracked in `docs/BACKLOG.md` (stories 1.3, 1.4). Until
//! then this returns an honestly-unknown profile rather than a guess, so the
//! CLI still runs everywhere without pretending to know the hardware.

use auto_quantize_core::HardwareProfile;

pub fn probe() -> HardwareProfile {
    HardwareProfile {
        vram_bytes: None,
        ram_bytes: 0,
        ram_free_bytes: 0,
        bandwidth_gbps: None,
    }
}
