//! Command implementations.

pub mod analyze;
pub mod completeness;
pub mod doctor;
pub mod grammar;
pub mod info;
pub mod readability;
#[cfg(feature = "mcp")]
pub mod serve;
pub mod tokens;
