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

/// Tie-breaking direction among quants that both fit the budget
/// (docs/BACKLOG.md 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
    /// Pick the largest fitting quant (best quality per byte). Default.
    Quality,
    /// Pick one size step below the largest fitting quant, trading quality
    /// for extra headroom (and therefore speed/safety margin).
    Speed,
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
    recommend_with_options(hardware, options, 0, Preference::Quality)
}

/// Like [`recommend`], with power-user overrides: `extra_reserve_bytes` is
/// subtracted from the accelerator budget before headroom is applied (e.g.
/// `--reserve-vram`), and `prefer` breaks ties between fitting quants.
pub fn recommend_with_options(
    hardware: &HardwareProfile,
    options: &[QuantOption],
    extra_reserve_bytes: u64,
    prefer: Preference,
) -> Option<Recommendation> {
    if options.is_empty() {
        return None;
    }

    let budget = hardware
        .accelerator_budget_bytes()
        .saturating_sub(extra_reserve_bytes);
    let usable_budget = (budget as f64 * (1.0 - DEFAULT_HEADROOM_FRACTION)) as u64;

    let mut sorted: Vec<&QuantOption> = options.iter().collect();
    sorted.sort_by_key(|q| q.size_bytes);

    let fitting: Vec<&&QuantOption> = sorted
        .iter()
        .filter(|q| q.size_bytes <= usable_budget)
        .collect();

    match (prefer, fitting.as_slice()) {
        (_, []) => {
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
        (Preference::Speed, [.., second_largest, _largest]) => {
            let winner = **second_largest;
            let headroom_gb = (usable_budget - winner.size_bytes) as f64 / 1e9;
            Some(Recommendation {
                quant: winner.clone(),
                reason: format!(
                    "fits entirely within budget with {:.1} GB headroom for context + KV cache; picked one size below the largest fit for extra speed margin",
                    headroom_gb
                ),
                fits_fully: true,
            })
        }
        (_, [.., largest]) => {
            let winner = **largest;
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

    #[test]
    fn extra_reserve_shrinks_the_usable_budget() {
        let options = vec![QuantOption::new("Q8_0", gb(9))];
        // 12 GB budget - 4 GB reserved = 8 GB * 0.85 = 6.8 GB usable -> no longer fits.
        let rec = recommend_with_options(&hw(12), &options, gb(4), Preference::Quality).unwrap();
        assert_eq!(rec.quant.name, "Q8_0");
        assert!(!rec.fits_fully);
    }

    #[test]
    fn extra_reserve_of_zero_matches_recommend() {
        let options = vec![
            QuantOption::new("Q4_K_M", gb(4)),
            QuantOption::new("Q5_K_M", gb(6)),
        ];
        let plain = recommend(&hw(12), &options).unwrap();
        let explicit = recommend_with_options(&hw(12), &options, 0, Preference::Quality).unwrap();
        assert_eq!(plain, explicit);
    }

    #[test]
    fn prefer_speed_picks_one_size_below_the_largest_fit() {
        let options = vec![
            QuantOption::new("Q4_K_M", gb(4)),
            QuantOption::new("Q5_K_M", gb(6)),
            QuantOption::new("Q6_K", gb(7)),
        ];
        // Both Q5_K_M and Q6_K fit the 12 GB budget's 10.2 GB usable portion.
        let quality = recommend_with_options(&hw(12), &options, 0, Preference::Quality).unwrap();
        assert_eq!(quality.quant.name, "Q6_K");

        let speed = recommend_with_options(&hw(12), &options, 0, Preference::Speed).unwrap();
        assert_eq!(speed.quant.name, "Q5_K_M");
    }

    #[test]
    fn prefer_speed_falls_back_to_only_fitting_option() {
        let options = vec![QuantOption::new("Q4_K_M", gb(4))];
        let speed = recommend_with_options(&hw(12), &options, 0, Preference::Speed).unwrap();
        assert_eq!(speed.quant.name, "Q4_K_M");
    }
}
