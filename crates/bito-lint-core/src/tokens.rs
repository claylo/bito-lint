//! Pluggable token counting with multiple backends.
//!
//! Two backends are available:
//!
//! - **Claude** (default): Uses ctoc's 38,360 API-verified Claude 3+ tokens
//!   with greedy longest-match via `aho-corasick`. Table-aware: decomposes
//!   markdown tables so pipe boundaries are respected, preventing undercounts.
//!   Overcounts by ~4% compared to the real Claude tokenizer — safe for budget
//!   enforcement.
//! - **OpenAI**: Uses `bpe-openai` for exact cl100k_base BPE encoding.
//!
//! For exact Claude counts, use the Anthropic `count_tokens` API.

use std::ops::Range;
use std::sync::LazyLock;

use aho_corasick::AhoCorasick;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
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

/// The 38,360 API-verified Claude 3+ token strings from ctoc.
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

/// Raw greedy longest-match token count (no markdown awareness).
///
/// Walks the input left-to-right, greedily matching the longest known token
/// at each position. Unmatched bytes are counted as one token each
/// (conservative — these are characters not in the known vocab).
///
/// Use [`count_claude`] instead for markdown-aware counting that handles
/// table boundaries correctly.
fn count_claude_raw(text: &str) -> usize {
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

/// Find byte ranges of markdown tables in the input.
///
/// Uses pulldown-cmark's offset iterator to locate `Table` start/end events,
/// returning the byte ranges that enclose each table.
fn find_table_ranges(text: &str) -> Vec<Range<usize>> {
    let parser = Parser::new_ext(text, Options::ENABLE_TABLES).into_offset_iter();
    let mut ranges = Vec::new();
    let mut table_start: Option<usize> = None;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Table(_)) => {
                table_start = Some(range.start);
            }
            Event::End(TagEnd::Table) => {
                if let Some(start) = table_start.take() {
                    ranges.push(start..range.end);
                }
            }
            _ => {}
        }
    }
    ranges
}

/// Count Claude tokens in a markdown table fragment.
///
/// Splits each line on `|` to prevent the Aho-Corasick automaton from
/// matching tokens that span across cell boundaries. Each `|` is counted
/// as one token, and cell contents are tokenized individually via
/// [`count_claude_raw`].
fn count_claude_table(table_text: &str) -> usize {
    let mut count: usize = 0;
    for line in table_text.split('\n') {
        let pipes = line.bytes().filter(|&b| b == b'|').count();
        count += pipes;
        for segment in line.split('|') {
            count += count_claude_raw(segment);
        }
    }
    count
}

/// Count tokens using the Claude backend (table-aware greedy longest-match).
///
/// For text without markdown tables, delegates directly to [`count_claude_raw`].
/// When tables are detected, decomposes them so that pipe (`|`) boundaries
/// are respected — preventing the automaton from matching tokens that span
/// across cells, which would undercount.
fn count_claude(text: &str) -> usize {
    // Fast path: no pipe character means no tables possible.
    if !text.contains('|') {
        return count_claude_raw(text);
    }

    let table_ranges = find_table_ranges(text);
    if table_ranges.is_empty() {
        return count_claude_raw(text);
    }

    let mut count: usize = 0;
    let mut pos: usize = 0;

    for range in &table_ranges {
        // Non-table text before this table.
        if range.start > pos {
            count += count_claude_raw(&text[pos..range.start]);
        }
        // Table region — cell-aware counting.
        count += count_claude_table(&text[range.start..range.end]);
        pos = range.end;
    }

    // Trailing non-table text.
    if pos < text.len() {
        count += count_claude_raw(&text[pos..]);
    }

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

    // -----------------------------------------------------------------------
    // Table-aware tokenization
    // -----------------------------------------------------------------------

    #[test]
    fn table_counts_at_least_raw() {
        let table = "| Name | Age |\n|------|-----|\n| Alice | 30 |\n| Bob | 25 |\n";
        let raw = count_claude_raw(table);
        let aware = count_claude(table);
        assert!(
            aware >= raw,
            "table-aware ({aware}) should be >= raw ({raw})"
        );
    }

    #[test]
    fn no_table_matches_raw() {
        let text = "The quick brown fox jumps over the lazy dog.";
        assert_eq!(count_claude(text), count_claude_raw(text));
    }

    #[test]
    fn pipe_in_non_table_unchanged() {
        let text = "Use the || operator for logical OR.";
        // No markdown table structure, so raw path is used.
        assert_eq!(count_claude(text), count_claude_raw(text));
    }

    #[test]
    fn mixed_table_and_prose() {
        let text = "Some prose before the table.\n\n\
                    | Col A | Col B |\n\
                    |-------|-------|\n\
                    | x     | y     |\n\n\
                    Some prose after the table.";
        let aware = count_claude(text);
        assert!(aware > 0, "should produce a positive count");
        // Table-aware should be >= raw because table decomposition only adds.
        let raw = count_claude_raw(text);
        assert!(
            aware >= raw,
            "table-aware ({aware}) should be >= raw ({raw})"
        );
    }

    #[test]
    fn empty_table_cells() {
        let table = "| | |\n|---|---|\n| | |\n";
        let count = count_claude(table);
        // At minimum: pipes are counted as tokens.
        assert!(count > 0, "empty-cell table should still produce tokens");
    }

    #[test]
    fn find_table_ranges_finds_one_table() {
        let text = "Hello\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nGoodbye\n";
        let ranges = find_table_ranges(text);
        assert_eq!(ranges.len(), 1, "should find exactly one table");
        let table_slice = &text[ranges[0].clone()];
        assert!(
            table_slice.contains("| A |"),
            "range should contain the table header"
        );
    }

    #[test]
    fn claude_overcounts_vs_openai_with_tables() {
        let text = "# Report\n\n\
                    | Metric | Value |\n\
                    |--------|-------|\n\
                    | CPU    | 85%   |\n\
                    | Memory | 4 GB  |\n\n\
                    Overall performance is satisfactory.";
        let claude = count_tokens(text, None, Backend::Claude).unwrap();
        let openai = count_tokens(text, None, Backend::Openai).unwrap();
        assert!(
            claude.count >= openai.count,
            "Claude ({}) should overcount vs OpenAI ({}) even with tables",
            claude.count,
            openai.count
        );
    }
}
