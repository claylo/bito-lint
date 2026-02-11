//! Pluggable token counting with multiple backends.
//!
//! Two backends are available:
//!
//! - **Claude** (default): Uses ctoc's 36,495 API-verified Claude 3+ tokens
//!   with greedy longest-match via `aho-corasick`. Overcounts by ~4% compared
//!   to the real Claude tokenizer — safe for budget enforcement.
//! - **OpenAI**: Uses `bpe-openai` for exact cl100k_base BPE encoding.
//!
//! For exact Claude counts, use the Anthropic `count_tokens` API.

use std::sync::LazyLock;

use aho_corasick::AhoCorasick;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::AnalysisResult;

/// Tokenizer backend for token counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Backend {
    /// Claude 3+ (ctoc verified vocab, greedy longest-match). Overcounts ~4%.
    #[default]
    Claude,
    /// OpenAI cl100k_base (exact BPE encoding via bpe-openai).
    #[cfg_attr(feature = "clap", value(name = "openai"))]
    Openai,
}

impl Backend {
    /// Returns the backend name as a lowercase string slice.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Openai => "openai",
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Claude backend: greedy longest-match on ctoc's verified vocabulary
// ---------------------------------------------------------------------------

/// The 36,495 API-verified Claude 3+ token strings from ctoc.
static CLAUDE_VOCAB_JSON: &str = include_str!("claude_vocab.json");

/// Pre-built Aho-Corasick automaton for greedy longest-match tokenization.
static CLAUDE_AUTOMATON: LazyLock<AhoCorasick> = LazyLock::new(|| {
    let vocab: Vec<String> =
        serde_json::from_str(CLAUDE_VOCAB_JSON).expect("embedded claude_vocab.json is valid");
    AhoCorasick::builder()
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build(&vocab)
        .expect("aho-corasick build should succeed for verified vocab")
});

/// Count tokens using the Claude backend (greedy longest-match).
///
/// Walks the input left-to-right, greedily matching the longest known token
/// at each position. Unmatched bytes are counted as one token each
/// (conservative — these are characters not in the known vocab).
fn count_claude(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let mut count: usize = 0;
    let mut pos: usize = 0;
    let bytes = text.as_bytes();

    for mat in CLAUDE_AUTOMATON.find_iter(text) {
        // Count any unmatched bytes before this match as individual tokens.
        let gap = mat.start() - pos;
        count += gap;
        // Count the matched token.
        count += 1;
        pos = mat.end();
    }

    // Count any trailing unmatched bytes.
    count += bytes.len() - pos;
    count
}

// ---------------------------------------------------------------------------
// OpenAI backend: exact cl100k_base via bpe-openai
// ---------------------------------------------------------------------------

/// Count tokens using the OpenAI cl100k_base backend (exact BPE).
fn count_openai(text: &str) -> usize {
    let tokenizer = bpe_openai::cl100k_base();
    tokenizer.count(text)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

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
    /// Which tokenizer backend produced this count.
    pub tokenizer: String,
}

/// Count tokens in text using the specified backend.
///
/// # Arguments
///
/// * `text` — The text to tokenize.
/// * `budget` — Optional maximum token count. If provided, `over_budget`
///   in the report indicates whether the text exceeds it.
/// * `backend` — Which tokenizer to use.
#[tracing::instrument(skip(text), fields(text_len = text.len(), backend = %backend))]
pub fn count_tokens(
    text: &str,
    budget: Option<usize>,
    backend: Backend,
) -> AnalysisResult<TokenReport> {
    let count = match backend {
        Backend::Claude => count_claude(text),
        Backend::Openai => count_openai(text),
    };
    let over_budget = budget.is_some_and(|max| count > max);

    Ok(TokenReport {
        count,
        budget,
        over_budget,
        tokenizer: backend.as_str().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_backend_counts_tokens() {
        let report = count_tokens("Hello, world!", None, Backend::Claude).unwrap();
        assert!(report.count > 0);
        assert_eq!(report.tokenizer, "claude");
    }

    #[test]
    fn openai_backend_counts_tokens() {
        let report = count_tokens("Hello, world!", None, Backend::Openai).unwrap();
        assert!(report.count > 0);
        assert_eq!(report.tokenizer, "openai");
    }

    #[test]
    fn claude_overcounts_vs_openai() {
        let text = "The quick brown fox jumps over the lazy dog. \
                    This is a longer passage of English text that should \
                    demonstrate the conservative overcounting behavior of \
                    the Claude tokenizer backend compared to OpenAI's exact \
                    cl100k_base encoding.";
        let claude = count_tokens(text, None, Backend::Claude).unwrap();
        let openai = count_tokens(text, None, Backend::Openai).unwrap();
        assert!(
            claude.count >= openai.count,
            "Claude ({}) should overcount vs OpenAI ({})",
            claude.count,
            openai.count
        );
    }

    #[test]
    fn backend_default_is_claude() {
        assert_eq!(Backend::default(), Backend::Claude);
    }

    #[test]
    fn backend_display_and_as_str() {
        assert_eq!(Backend::Claude.as_str(), "claude");
        assert_eq!(Backend::Openai.as_str(), "openai");
        assert_eq!(format!("{}", Backend::Claude), "claude");
        assert_eq!(format!("{}", Backend::Openai), "openai");
    }

    #[test]
    fn detects_over_budget() {
        let report =
            count_tokens("Hello, world! This is a test.", Some(1), Backend::default()).unwrap();
        assert!(report.over_budget);
        assert_eq!(report.budget, Some(1));
    }

    #[test]
    fn within_budget() {
        let report = count_tokens("Hi", Some(100), Backend::default()).unwrap();
        assert!(!report.over_budget);
    }

    #[test]
    fn empty_text_returns_zero() {
        let report = count_tokens("", None, Backend::Claude).unwrap();
        assert_eq!(report.count, 0);
        let report = count_tokens("", None, Backend::Openai).unwrap();
        assert_eq!(report.count, 0);
    }

    #[test]
    fn backend_serde_roundtrip() {
        let json = serde_json::to_string(&Backend::Claude).unwrap();
        assert_eq!(json, "\"claude\"");
        let back: Backend = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Backend::Claude);

        let json = serde_json::to_string(&Backend::Openai).unwrap();
        assert_eq!(json, "\"openai\"");
        let back: Backend = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Backend::Openai);
    }
}
