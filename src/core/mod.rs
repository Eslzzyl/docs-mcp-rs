//! Core module containing error types, configuration, and domain types.

pub mod config;
pub mod embedding;
pub mod error;
pub mod types;

pub use config::Config;
pub use embedding::*;
pub use error::{Error, Result};
pub use types::*;
