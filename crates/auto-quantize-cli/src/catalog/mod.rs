//! HuggingFace GGUF quant catalog lookup.

mod architecture;
mod fetch;
mod parse;

pub use architecture::fetch_architecture;
pub use fetch::{fetch_quants, CatalogError};
pub use parse::QuantFile;
