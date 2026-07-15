//! A single candidate quantization of a model, as offered by a GGUF repo.

use serde::{Deserialize, Serialize};

/// One quantized build of a model (e.g. `Q4_K_M`, `Q5_K_M`, `Q8_0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantOption {
    /// The quant label as it appears in the GGUF filename (e.g. `"Q5_K_M"`).
    pub name: String,
    /// On-disk size of this build, in bytes.
    pub size_bytes: u64,
}

impl QuantOption {
    pub fn new(name: impl Into<String>, size_bytes: u64) -> Self {
        Self {
            name: name.into(),
            size_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_owned_and_borrowed_strings() {
        let from_str = QuantOption::new("Q4_K_M", 4_000_000_000);
        let from_string = QuantOption::new(String::from("Q4_K_M"), 4_000_000_000);
        assert_eq!(from_str, from_string);
    }
}
