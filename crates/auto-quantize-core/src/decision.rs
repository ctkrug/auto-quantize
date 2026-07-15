//! Scores [`QuantOption`]s against a [`HardwareProfile`] and picks a winner.
//!
//! v1 policy: prefer the largest quant that fits entirely within the
//! accelerator budget, after reserving headroom for context/KV cache.
//! Reserving a fixed fraction rather than a fixed byte count means the
//! headroom scales sensibly across small (8 GB) and large (80 GB) budgets
//! alike; the exact overhead model (context-length-aware) is tracked in the
//! backlog.

use crate::hardware::HardwareProfile;
use crate::quant::QuantOption;

/// Fraction of the accelerator budget reserved for context and KV cache
/// rather than model weights.
const DEFAULT_HEADROOM_FRACTION: f64 = 0.15;

/// The chosen quant, plus the human-readable reasoning behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct Recommendation {
    pub quant: QuantOption,
    pub reason: String,
    /// True if the winning quant fits fully within the accelerator budget;
    /// false if every option was too large and this is the smallest
    /// available fallback (which will partially offload or swap).
    pub fits_fully: bool,
}

/// Pick the best-fitting quant for `hardware` out of `options`.
///
/// Returns `None` if `options` is empty.
///
/// ```
/// use auto_quantize_core::{recommend, HardwareProfile, QuantOption};
///
/// let hardware = HardwareProfile {
///     vram_bytes: Some(12_000_000_000),
///     ram_bytes: 32_000_000_000,
///     ram_free_bytes: 18_000_000_000,
///     bandwidth_gbps: Some(200.0),
/// };
/// let options = vec![
///     QuantOption::new("Q4_K_M", 4_000_000_000),
///     QuantOption::new("Q5_K_M", 6_000_000_000),
/// ];
///
/// let rec = recommend(&hardware, &options).unwrap();
/// assert_eq!(rec.quant.name, "Q5_K_M");
/// assert!(rec.fits_fully);
/// ```
pub fn recommend(hardware: &HardwareProfile, options: &[QuantOption]) -> Option<Recommendation> {
    if options.is_empty() {
        return None;
    }

    let budget = hardware.accelerator_budget_bytes();
    let usable_budget = (budget as f64 * (1.0 - DEFAULT_HEADROOM_FRACTION)) as u64;

    let mut sorted: Vec<&QuantOption> = options.iter().collect();
    sorted.sort_by_key(|q| q.size_bytes);

    let best_fit = sorted
        .iter()
        .rev()
        .find(|q| q.size_bytes <= usable_budget)
        .copied();

    match best_fit {
        Some(winner) => {
            let headroom_gb = (usable_budget - winner.size_bytes) as f64 / 1e9;
            Some(Recommendation {
                quant: winner.clone(),
                reason: format!(
                    "fits entirely within budget with {:.1} GB headroom for context + KV cache",
                    headroom_gb
                ),
                fits_fully: true,
            })
        }
        None => {
            // Nothing fits: fall back to the smallest option and say so.
            let smallest = sorted[0];
            let shortfall_gb = (smallest.size_bytes.saturating_sub(usable_budget)) as f64 / 1e9;
            Some(Recommendation {
                quant: smallest.clone(),
                reason: format!(
                    "smallest available build still exceeds budget by {:.1} GB; expect partial offload or swapping",
                    shortfall_gb
                ),
                fits_fully: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gb(n: u64) -> u64 {
        n * 1_000_000_000
    }

    fn hw(vram_gb: u64) -> HardwareProfile {
        HardwareProfile {
            vram_bytes: Some(gb(vram_gb)),
            ram_bytes: gb(32),
            ram_free_bytes: gb(18),
            bandwidth_gbps: Some(200.0),
        }
    }

    #[test]
    fn picks_largest_quant_that_fits() {
        let options = vec![
            QuantOption::new("Q4_K_M", gb(4)),
            QuantOption::new("Q5_K_M", gb(6)),
            QuantOption::new("Q6_K", gb(7)),
            QuantOption::new("Q8_0", gb(9)),
        ];
        // 12 GB budget * 0.85 headroom = 10.2 GB usable -> Q8_0 (9GB) fits.
        let rec = recommend(&hw(12), &options).unwrap();
        assert_eq!(rec.quant.name, "Q8_0");
        assert!(rec.fits_fully);
    }

    #[test]
    fn falls_back_to_smallest_when_nothing_fits() {
        let options = vec![
            QuantOption::new("Q4_K_M", gb(20)),
            QuantOption::new("Q5_K_M", gb(24)),
        ];
        let rec = recommend(&hw(8), &options).unwrap();
        assert_eq!(rec.quant.name, "Q4_K_M");
        assert!(!rec.fits_fully);
    }

    #[test]
    fn empty_options_returns_none() {
        assert!(recommend(&hw(12), &[]).is_none());
    }
}
