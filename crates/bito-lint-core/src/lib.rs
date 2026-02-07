//! Core library for bito-lint.
//!
//! This crate provides the foundational types and functionality used by the
//! `bito-lint` CLI and any downstream consumers.
//!
//! # Modules
//!
//! - [`config`] - Configuration loading and management
//! - [`error`] - Error types and result aliases
//!
//! # Quick Start
//!
//! ```no_run
//! use bito_lint_core::{Config, ConfigLoader};
//!
//! let config = ConfigLoader::new()
//!     .with_user_config(true)
//!     .load()
//!     .expect("Failed to load configuration");
//!
//! println!("Log level: {:?}", config.log_level);
//! ```
#![deny(unsafe_code)]

pub mod config;

pub mod error;

pub use config::{Config, ConfigLoader, LogLevel};

pub use error::{ConfigError, ConfigResult};
