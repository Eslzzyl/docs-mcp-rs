//! Core module containing error types, configuration, and domain types.

pub mod error;
pub mod config;
pub mod types;

pub use error::{Error, Result};
pub use config::Config;
pub use types::*;
