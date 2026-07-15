//! Scores [`QuantOption`]s against a [`HardwareProfile`] and picks a winner.
//!
//! v1 policy: prefer the largest quant that fits entirely within the
//! accelerator budget, after reserving headroom for context/KV cache.
//! Reserving a fixed fraction rather than a fixed byte count means the
//! headroom scales sensibly across small (8 GB) and large (80 GB) budgets
//! alike; the exact overhead model (context-length-aware) is tracked in the
//! backlog.

use crate::architecture::ModelArchitecture;
use crate::hardware::HardwareProfile;
use crate::quant::QuantOption;

/// Fraction of the accelerator budget reserved for context and KV cache
/// rather than model weights. Used as a fallback when no [`ContextConfig`]
/// (and therefore no exact KV-cache size) is available for a repo.
const DEFAULT_HEADROOM_FRACTION: f64 = 0.15;

/// Context length + model shape needed to compute an exact KV-cache
/// headroom (docs/BACKLOG.md 1.6), rather than the flat
/// [`DEFAULT_HEADROOM_FRACTION`] placeholder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContextConfig {
    pub context_length: u32,
    pub architecture: ModelArchitecture,
}

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
    recommend_with_context(hardware, options, extra_reserve_bytes, prefer, None)
}

/// Like [`recommend_with_options`], additionally taking an optional
/// [`ContextConfig`] (docs/BACKLOG.md 1.6). When `context` is `Some`, headroom
/// is the *exact* KV-cache size for that context length and model shape
/// rather than the flat [`DEFAULT_HEADROOM_FRACTION`] placeholder, and the
/// reason names the context length when it's the limiting factor.
pub fn recommend_with_context(
    hardware: &HardwareProfile,
    options: &[QuantOption],
    extra_reserve_bytes: u64,
    prefer: Preference,
    context: Option<ContextConfig>,
) -> Option<Recommendation> {
    if options.is_empty() {
        return None;
    }

    let budget = hardware
        .accelerator_budget_bytes()
        .saturating_sub(extra_reserve_bytes);

    let (usable_budget, headroom_phrase) = match context {
        Some(cfg) => {
            let kv_bytes = cfg.architecture.kv_cache_bytes(cfg.context_length);
            let usable = budget.saturating_sub(kv_bytes);
            let phrase = format!(
                "after reserving {:.2} GB of KV cache for a {}-token context",
                kv_bytes as f64 / 1e9,
                cfg.context_length
            );
            (usable, phrase)
        }
        None => {
            let usable = (budget as f64 * (1.0 - DEFAULT_HEADROOM_FRACTION)) as u64;
            (usable, "for context + KV cache".to_string())
        }
    };

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
                    "smallest available build still exceeds budget by {:.1} GB {headroom_phrase}; expect partial offload or swapping",
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
                    "fits entirely within budget with {headroom_gb:.1} GB headroom {headroom_phrase}; picked one size below the largest fit for extra speed margin"
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
                    "fits entirely within budget with {headroom_gb:.1} GB headroom {headroom_phrase}"
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
    fn extra_reserve_changes_which_quant_is_recommended() {
        let options = vec![
            QuantOption::new("Q4_K_M", gb(4)),
            QuantOption::new("Q5_K_M", gb(6)),
            QuantOption::new("Q6_K", gb(7)),
        ];
        let before = recommend_with_options(&hw(12), &options, 0, Preference::Quality).unwrap();
        assert_eq!(before.quant.name, "Q6_K");

        // Reserve 4 extra GB: 12 - 4 = 8 GB * 0.85 = 6.8 GB usable -> Q6_K no longer fits.
        let after = recommend_with_options(&hw(12), &options, gb(4), Preference::Quality).unwrap();
        assert_eq!(after.quant.name, "Q5_K_M");
        assert_ne!(before.quant.name, after.quant.name);
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

    fn arch(num_layers: u32, hidden_size: u32) -> ModelArchitecture {
        ModelArchitecture {
            num_layers,
            hidden_size,
        }
    }

    #[test]
    fn small_context_fits_with_room_to_spare() {
        let options = vec![
            QuantOption::new("Q6_K", gb(7)),
            QuantOption::new("Q8_0", gb(9)),
        ];
        // KV cache for 512 tokens with this tiny shape is a few KB — negligible
        // next to a 12 GB budget, so the largest quant still fits.
        let context = ContextConfig {
            context_length: 512,
            architecture: arch(8, 512),
        };
        let rec = recommend_with_context(&hw(12), &options, 0, Preference::Quality, Some(context))
            .unwrap();
        assert_eq!(rec.quant.name, "Q8_0");
        assert!(rec.fits_fully);
    }

    #[test]
    fn large_context_pushes_a_previously_fitting_quant_down_a_size() {
        let options = vec![
            QuantOption::new("Q6_K", gb(7)),
            QuantOption::new("Q8_0", gb(9)),
        ];
        // 2 (K+V) * 40 layers * 5120 hidden * 2 bytes/elem * 131072 tokens
        // ≈ 107.4 GB of KV cache — far larger than the 12 GB budget, so even
        // the smallest quant no longer fits and the picker falls back.
        let context = ContextConfig {
            context_length: 131_072,
            architecture: arch(40, 5120),
        };
        let rec = recommend_with_context(&hw(12), &options, 0, Preference::Quality, Some(context))
            .unwrap();
        assert!(!rec.fits_fully);
        assert_eq!(rec.quant.name, "Q6_K");
    }

    #[test]
    fn moderate_context_pushes_the_largest_fit_down_one_size() {
        let options = vec![
            QuantOption::new("Q4_K_M", gb(4)),
            QuantOption::new("Q5_K_M", gb(6)),
            QuantOption::new("Q6_K", gb(7)),
        ];
        // Small architecture, big-enough context that its KV cache (a bit
        // over 1 GB) eats into the budget without wiping it out entirely.
        let small_context = ContextConfig {
            context_length: 4096,
            architecture: arch(32, 4096),
        };
        let with_small_context = recommend_with_context(
            &hw(12),
            &options,
            0,
            Preference::Quality,
            Some(small_context),
        )
        .unwrap();
        assert_eq!(with_small_context.quant.name, "Q6_K");

        let large_context = ContextConfig {
            context_length: 10_240,
            architecture: arch(32, 4096),
        };
        let with_large_context = recommend_with_context(
            &hw(12),
            &options,
            0,
            Preference::Quality,
            Some(large_context),
        )
        .unwrap();
        assert_eq!(with_large_context.quant.name, "Q5_K_M");
        assert_ne!(with_small_context.quant.name, with_large_context.quant.name);
    }

    #[test]
    fn reason_names_the_context_length_when_it_is_the_limiting_factor() {
        let options = vec![QuantOption::new("Q8_0", gb(9))];
        let context = ContextConfig {
            context_length: 32_768,
            architecture: arch(32, 4096),
        };
        let rec = recommend_with_context(&hw(12), &options, 0, Preference::Quality, Some(context))
            .unwrap();
        assert!(rec.reason.contains("32768"));
        assert!(rec.reason.contains("KV cache"));
    }

    #[test]
    fn no_context_config_matches_recommend_with_options() {
        let options = vec![QuantOption::new("Q5_K_M", gb(6))];
        let without = recommend_with_options(&hw(12), &options, 0, Preference::Quality).unwrap();
        let with_none =
            recommend_with_context(&hw(12), &options, 0, Preference::Quality, None).unwrap();
        assert_eq!(without, with_none);
    }
}
