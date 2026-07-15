//! Core hardware-probing and quant-fit decision logic for auto-quantize.
//!
//! This crate has no CLI or network dependencies of its own: it takes a
//! [`HardwareProfile`] and a list of [`QuantOption`]s and returns a
//! [`Recommendation`]. The CLI crate is responsible for producing the
//! hardware profile (via platform probes) and the quant list (via the
//! HuggingFace API).

pub mod decision;
pub mod hardware;
pub mod quant;

pub use decision::{recommend, recommend_with_options, Preference, Recommendation};
pub use hardware::HardwareProfile;
pub use quant::QuantOption;
