//! Token counting using tiktoken cl100k_base.
//!
//! Counts tokens as an approximation of Claude context usage.
//! Claude uses its own tokenizer, so counts here are estimates.
//! For exact counts, use the Anthropic token counting API.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{AnalysisError, AnalysisResult};

/// Result of counting tokens in a text.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TokenReport {
    /// Number of tokens in the text.
    pub count: usize,
    /// Token budget (if provided).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<usize>,
    /// Whether the count exceeds the budget.
    pub over_budget: bool,
}

/// Count tokens in text using the cl100k_base tokenizer.
///
/// This is an approximation — Claude uses its own tokenizer. The cl100k_base
/// encoding is close enough for budget enforcement.
///
/// # Arguments
///
/// * `text` — The text to tokenize.
/// * `budget` — Optional maximum token count. If provided, `over_budget`
///   in the report indicates whether the text exceeds it.
#[tracing::instrument(skip(text), fields(text_len = text.len()))]
pub fn count_tokens(text: &str, budget: Option<usize>) -> AnalysisResult<TokenReport> {
    let bpe =
        tiktoken_rs::cl100k_base().map_err(|e| AnalysisError::TokenizerInit(e.to_string()))?;
    let count = bpe.encode_ordinary(text).len();
    let over_budget = budget.is_some_and(|max| count > max);

    Ok(TokenReport {
        count,
        budget,
        over_budget,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_tokens_in_simple_text() {
        let report = count_tokens("Hello, world!", None).unwrap();
        assert!(report.count > 0);
        assert!(!report.over_budget);
        assert!(report.budget.is_none());
    }

    #[test]
    fn detects_over_budget() {
        let report = count_tokens("Hello, world! This is a test.", Some(1)).unwrap();
        assert!(report.over_budget);
        assert_eq!(report.budget, Some(1));
    }

    #[test]
    fn within_budget() {
        let report = count_tokens("Hi", Some(100)).unwrap();
        assert!(!report.over_budget);
    }

    #[test]
    fn empty_text_returns_zero() {
        let report = count_tokens("", None).unwrap();
        assert_eq!(report.count, 0);
    }
}
