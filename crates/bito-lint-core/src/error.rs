//! Error types for bito-lint-core.

use thiserror::Error;

/// Errors that can occur when working with configuration.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Failed to deserialize configuration.
    #[error("invalid configuration: {0}")]
    Deserialize(#[from] Box<figment::Error>),

    /// Configuration file not found after searching all locations.
    #[error("no configuration file found")]
    NotFound,
}

/// Result type alias using [`ConfigError`].
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Errors that can occur during text analysis.
#[derive(Error, Debug)]
pub enum AnalysisError {
    /// The tokenizer could not be initialized.
    #[error("tokenizer initialization failed: {0}")]
    TokenizerInit(String),

    /// The input text is empty or has no scorable content.
    #[error("no scorable text in input")]
    EmptyInput,

    /// An unknown template name was provided.
    #[error("unknown template: {name}. Use: {available}")]
    UnknownTemplate {
        /// The template name that was requested.
        name: String,
        /// Comma-separated list of available template names.
        available: String,
    },
}

/// Result type alias using [`AnalysisError`].
pub type AnalysisResult<T> = Result<T, AnalysisError>;
