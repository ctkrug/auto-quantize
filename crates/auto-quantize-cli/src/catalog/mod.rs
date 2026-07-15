//! HuggingFace GGUF quant catalog lookup.

mod fetch;
mod parse;

pub use fetch::{fetch_quants, CatalogError};
pub use parse::QuantFile;
