//! Optional model shape metadata used to size a transformer's KV cache
//! exactly, when it's known, instead of guessing at a flat headroom fraction.

use serde::{Deserialize, Serialize};

/// The shape parameters needed to size a transformer's KV cache for a given
/// context length.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelArchitecture {
    /// Number of transformer layers (a.k.a. `num_hidden_layers`).
    pub num_layers: u32,
    /// Model hidden size (a.k.a. `hidden_size` / `d_model`).
    pub hidden_size: u32,
}

impl ModelArchitecture {
    /// Bytes needed to hold the K and V caches for `context_length` tokens.
    ///
    /// Assumes fp16 cache storage and full multi-head attention (no
    /// grouped-query-attention head-count reduction) — a conservative upper
    /// bound to use when the exact KV head count isn't known.
    pub fn kv_cache_bytes(&self, context_length: u32) -> u64 {
        const KV_TENSORS: u64 = 2; // one cache each for K and V
        const BYTES_PER_ELEMENT: u64 = 2; // fp16

        KV_TENSORS
            * self.num_layers as u64
            * self.hidden_size as u64
            * BYTES_PER_ELEMENT
            * context_length as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_cache_bytes_scales_with_context_length() {
        let arch = ModelArchitecture {
            num_layers: 32,
            hidden_size: 4096,
        };
        let small = arch.kv_cache_bytes(2048);
        let large = arch.kv_cache_bytes(8192);
        assert_eq!(large, small * 4);
    }

    #[test]
    fn kv_cache_bytes_matches_known_formula() {
        // 2 (K+V) * 2 layers * 4 hidden * 2 bytes * 10 tokens = 320 bytes.
        let arch = ModelArchitecture {
            num_layers: 2,
            hidden_size: 4,
        };
        assert_eq!(arch.kv_cache_bytes(10), 320);
    }

    #[test]
    fn zero_context_length_needs_no_cache() {
        let arch = ModelArchitecture {
            num_layers: 32,
            hidden_size: 4096,
        };
        assert_eq!(arch.kv_cache_bytes(0), 0);
    }

    #[test]
    fn zero_layers_needs_no_cache() {
        let arch = ModelArchitecture {
            num_layers: 0,
            hidden_size: 4096,
        };
        assert_eq!(arch.kv_cache_bytes(4096), 0);
    }

    #[test]
    fn extreme_shape_saturates_instead_of_overflowing() {
        // A malformed or hostile config.json isn't bounds-checked beyond
        // "nonzero" (catalog::architecture::RawConfig::into_architecture),
        // so u32::MAX layers/hidden_size/context must not panic or wrap.
        let arch = ModelArchitecture {
            num_layers: u32::MAX,
            hidden_size: u32::MAX,
        };
        assert_eq!(arch.kv_cache_bytes(u32::MAX), u64::MAX);
    }
}
