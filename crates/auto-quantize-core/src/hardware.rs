//! Describes the machine auto-quantize is running on.
//!
//! Population of a real [`HardwareProfile`] (platform-specific VRAM/RAM/
//! bandwidth probing) lives in the CLI crate's `probe` module; this type is
//! deliberately probe-agnostic so the decision engine can be tested with
//! fixed profiles.

use serde::{Deserialize, Serialize};

/// A snapshot of the memory and bandwidth resources available for running a
/// quantized model.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// Dedicated or unified GPU memory available to the model, in bytes.
    /// `None` when no GPU/accelerator was detected (CPU-only inference).
    pub vram_bytes: Option<u64>,
    /// Total system RAM, in bytes.
    pub ram_bytes: u64,
    /// Free system RAM at probe time, in bytes.
    pub ram_free_bytes: u64,
    /// Estimated effective memory bandwidth, in gigabytes per second, if it
    /// could be determined for this platform.
    pub bandwidth_gbps: Option<f64>,
}

impl HardwareProfile {
    /// The memory budget available for a model that must run entirely on
    /// the accelerator: VRAM if present, otherwise free system RAM.
    pub fn accelerator_budget_bytes(&self) -> u64 {
        self.vram_bytes.unwrap_or(self.ram_free_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_prefers_vram_when_present() {
        let hw = HardwareProfile {
            vram_bytes: Some(8_000_000_000),
            ram_bytes: 32_000_000_000,
            ram_free_bytes: 20_000_000_000,
            bandwidth_gbps: None,
        };
        assert_eq!(hw.accelerator_budget_bytes(), 8_000_000_000);
    }

    #[test]
    fn budget_falls_back_to_free_ram_without_vram() {
        let hw = HardwareProfile {
            vram_bytes: None,
            ram_bytes: 32_000_000_000,
            ram_free_bytes: 20_000_000_000,
            bandwidth_gbps: None,
        };
        assert_eq!(hw.accelerator_budget_bytes(), 20_000_000_000);
    }
}
