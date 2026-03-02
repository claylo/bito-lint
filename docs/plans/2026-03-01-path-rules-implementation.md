# Path-Based Rules Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add path-based lint rules to the config file, a `lint` CLI subcommand, a `lint_file` MCP tool, and inline suppression directives so that both hooks and AI assistants can lint files according to project-specific rules.

**Architecture:** New `rules` module in bito-lint-core owns rule resolution (glob matching, specificity, accumulation). New `directives` module handles inline suppression parsing. A shared `lint` execution engine runs the resolved checks. CLI `lint` subcommand and MCP `lint_file` tool both call the same engine.

**Tech Stack:** `globset` for glob pattern matching, existing `figment` for config loading, existing `serde` for deserialization.

**Design doc:** `docs/plans/2026-03-01-path-rules-design.md`

---

### Task 1: Add `globset` Dependency

**Files:**
- Modify: `crates/bito-lint-core/Cargo.toml`

**Step 1: Add globset to dependencies**

Add under `[dependencies]` in `crates/bito-lint-core/Cargo.toml`:

```toml
globset = "0.4"
```

**Step 2: Verify it compiles**

Run: `cargo check -p bito-lint-core`
Expected: success

**Step 3: Commit**

```bash
git add crates/bito-lint-core/Cargo.toml Cargo.lock
git commit -m "chore: add globset dependency for path-based rules"
```

---

### Task 2: Config Structs for Rules

**Files:**
- Modify: `crates/bito-lint-core/src/config.rs` (add rule types, add `rules` field to `Config`)

**Step 1: Write failing test for rule deserialization**

Add to the `#[cfg(test)] mod tests` block in `config.rs`:

```rust
#[test]
fn rules_deserialize_from_yaml() {
    let yaml = r#"
rules:
  - paths: ["docs/**/*.md"]
    checks:
      analyze:
        max_grade: 8.0
  - paths: [".handoffs/*.md"]
    checks:
      tokens:
        budget: 2000
      completeness:
        template: handoff
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    let rules = config.rules.expect("rules should be present");
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].paths, vec!["docs/**/*.md"]);
    let analyze = rules[0].checks.analyze.as_ref().unwrap();
    assert_eq!(analyze.max_grade, Some(8.0));
    let tokens = rules[1].checks.tokens.as_ref().unwrap();
    assert_eq!(tokens.budget, Some(2000));
    let comp = rules[1].checks.completeness.as_ref().unwrap();
    assert_eq!(comp.template, "handoff");
}

#[test]
fn rules_default_to_none() {
    let config = Config::default();
    assert!(config.rules.is_none());
}

#[test]
fn empty_config_still_works_with_rules_field() {
    let yaml = "log_level: info\n";
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.rules.is_none());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p bito-lint-core rules_deserialize`
Expected: FAIL (field `rules` does not exist)

**Step 3: Add rule config structs and `rules` field**

Add these structs above the `Config` struct in `config.rs`. Import `tokens::Backend` at the top if not already imported.

```rust
/// Settings for the `analyze` check within a rule.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AnalyzeRuleConfig {
    /// Which of the 18 analysis checks to run. Omit for all.
    pub checks: Option<Vec<String>>,
    /// Which analysis checks to skip.
    pub exclude: Option<Vec<String>>,
    /// Maximum acceptable readability grade level.
    pub max_grade: Option<f64>,
    /// Maximum acceptable passive voice percentage.
    pub passive_max: Option<f64>,
    /// Minimum acceptable style score (0--100).
    pub style_min: Option<i32>,
    /// English dialect for spelling enforcement.
    pub dialect: Option<Dialect>,
}

/// Settings for the `readability` check within a rule.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ReadabilityRuleConfig {
    /// Maximum acceptable Flesch-Kincaid grade level.
    pub max_grade: Option<f64>,
}

/// Settings for the `grammar` check within a rule.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct GrammarRuleConfig {
    /// Maximum acceptable passive voice percentage (0--100).
    pub passive_max: Option<f64>,
}

/// Settings for the `completeness` check within a rule.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CompletenessRuleConfig {
    /// Template name (required): "adr", "handoff", "design-doc", or custom.
    pub template: String,
}

/// Settings for the `tokens` check within a rule.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct TokensRuleConfig {
    /// Maximum token budget. Omit for no limit.
    pub budget: Option<usize>,
    /// Tokenizer backend: "claude" (default) or "openai".
    pub tokenizer: Option<Backend>,
}

/// Checks to run for a path-based rule.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct RuleChecks {
    /// Run comprehensive writing analysis (18 checks).
    pub analyze: Option<AnalyzeRuleConfig>,
    /// Run standalone readability scoring (gate on grade level).
    pub readability: Option<ReadabilityRuleConfig>,
    /// Run standalone grammar checking (gate on passive voice).
    pub grammar: Option<GrammarRuleConfig>,
    /// Run completeness checking against a template.
    pub completeness: Option<CompletenessRuleConfig>,
    /// Run token counting (gate on budget).
    pub tokens: Option<TokensRuleConfig>,
}

/// A path-based lint rule.
///
/// Glob patterns in `paths` are relative to the project root.
/// All matching rules accumulate; when two rules configure the same
/// check, the more specific pattern's settings win.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Rule {
    /// Glob patterns to match file paths against.
    pub paths: Vec<String>,
    /// Checks to run on matched files.
    pub checks: RuleChecks,
}
```

Add the `rules` field to the `Config` struct:

```rust
/// Path-based lint rules.
///
/// Each rule maps glob patterns to checks with specific settings.
/// All matching rules accumulate; more specific patterns override
/// less specific ones when they configure the same check.
pub rules: Option<Vec<Rule>>,
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p bito-lint-core -- rules_ config`
Expected: all PASS

**Step 5: Commit**

```bash
git add crates/bito-lint-core/src/config.rs
git commit -m "feat: add rule config structs for path-based lint rules"
```

---

### Task 3: Rule Resolution Engine

**Files:**
- Create: `crates/bito-lint-core/src/rules.rs`
- Modify: `crates/bito-lint-core/src/lib.rs` (add `pub mod rules;`)

**Step 1: Write failing tests**

Create `crates/bito-lint-core/src/rules.rs` with tests first:

```rust
//! Rule resolution engine.
//!
//! Matches file paths against configured glob rules and accumulates
//! check configurations. When multiple rules match, all contribute
//! their checks. Conflicts (same check in multiple rules) are resolved
//! by specificity: the pattern with more literal (non-wildcard) path
//! segments wins. Ties go to the earlier rule.

use globset::{Glob, GlobMatcher};

use crate::config::{
    AnalyzeRuleConfig, CompletenessRuleConfig, GrammarRuleConfig,
    ReadabilityRuleConfig, Rule, RuleChecks, TokensRuleConfig,
};

/// Compiled rule set for efficient matching.
pub struct RuleSet {
    compiled: Vec<CompiledRule>,
}

/// A single rule with pre-compiled glob matchers.
struct CompiledRule {
    matchers: Vec<(GlobMatcher, usize)>, // (matcher, specificity)
    checks: RuleChecks,
}

/// Accumulated check configurations after rule resolution.
#[derive(Debug, Clone, Default)]
pub struct ResolvedChecks {
    pub analyze: Option<AnalyzeRuleConfig>,
    pub readability: Option<ReadabilityRuleConfig>,
    pub grammar: Option<GrammarRuleConfig>,
    pub completeness: Option<CompletenessRuleConfig>,
    pub tokens: Option<TokensRuleConfig>,
}

impl ResolvedChecks {
    /// Returns `true` if no checks are configured.
    pub fn is_empty(&self) -> bool {
        self.analyze.is_none()
            && self.readability.is_none()
            && self.grammar.is_none()
            && self.completeness.is_none()
            && self.tokens.is_none()
    }
}

/// Count literal (non-wildcard) path segments in a glob pattern.
///
/// `docs/decisions/*.md` → 2 (`docs`, `decisions`)
/// `docs/**/*.md` → 1 (`docs`)
/// `**/*.md` → 0
fn specificity(pattern: &str) -> usize {
    pattern
        .split('/')
        .filter(|seg| !seg.contains('*') && !seg.contains('?') && !seg.contains('['))
        .count()
}

impl RuleSet {
    /// Compile a list of rules into a `RuleSet`.
    ///
    /// Invalid glob patterns are silently skipped with a tracing warning.
    pub fn compile(rules: &[Rule]) -> Self {
        let compiled = rules
            .iter()
            .filter_map(|rule| {
                let matchers: Vec<(GlobMatcher, usize)> = rule
                    .paths
                    .iter()
                    .filter_map(|pattern| {
                        match Glob::new(pattern) {
                            Ok(glob) => Some((glob.compile_matcher(), specificity(pattern))),
                            Err(e) => {
                                tracing::warn!(pattern, error = %e, "skipping invalid glob pattern");
                                None
                            }
                        }
                    })
                    .collect();
                if matchers.is_empty() {
                    None
                } else {
                    Some(CompiledRule {
                        matchers,
                        checks: rule.checks.clone(),
                    })
                }
            })
            .collect();
        Self { compiled }
    }

    /// Resolve which checks apply to a file path.
    ///
    /// All matching rules contribute. When two rules configure the same
    /// check type, the one matched by the higher-specificity pattern wins.
    /// Ties go to the earlier rule (lower index).
    pub fn resolve(&self, file_path: &str) -> ResolvedChecks {
        let mut result = ResolvedChecks::default();

        // Track the specificity of the winning rule for each check type,
        // so higher-specificity matches override lower ones.
        let mut analyze_spec: Option<usize> = None;
        let mut readability_spec: Option<usize> = None;
        let mut grammar_spec: Option<usize> = None;
        let mut completeness_spec: Option<usize> = None;
        let mut tokens_spec: Option<usize> = None;

        for rule in &self.compiled {
            // Find the highest specificity among this rule's matching patterns.
            let max_spec = rule
                .matchers
                .iter()
                .filter(|(m, _)| m.is_match(file_path))
                .map(|(_, s)| *s)
                .max();

            let Some(spec) = max_spec else {
                continue; // Rule doesn't match this path.
            };

            // For each check type: adopt if no winner yet, or if this
            // match is more specific (strictly greater).
            if rule.checks.analyze.is_some()
                && analyze_spec.map_or(true, |prev| spec > prev)
            {
                result.analyze = rule.checks.analyze.clone();
                analyze_spec = Some(spec);
            }
            if rule.checks.readability.is_some()
                && readability_spec.map_or(true, |prev| spec > prev)
            {
                result.readability = rule.checks.readability.clone();
                readability_spec = Some(spec);
            }
            if rule.checks.grammar.is_some()
                && grammar_spec.map_or(true, |prev| spec > prev)
            {
                result.grammar = rule.checks.grammar.clone();
                grammar_spec = Some(spec);
            }
            if rule.checks.completeness.is_some()
                && completeness_spec.map_or(true, |prev| spec > prev)
            {
                result.completeness = rule.checks.completeness.clone();
                completeness_spec = Some(spec);
            }
            if rule.checks.tokens.is_some()
                && tokens_spec.map_or(true, |prev| spec > prev)
            {
                result.tokens = rule.checks.tokens.clone();
                tokens_spec = Some(spec);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rules(specs: &[(&[&str], RuleChecks)]) -> Vec<Rule> {
        specs
            .iter()
            .map(|(paths, checks)| Rule {
                paths: paths.iter().map(|s| (*s).to_string()).collect(),
                checks: checks.clone(),
            })
            .collect()
    }

    #[test]
    fn specificity_counts_literal_segments() {
        assert_eq!(specificity("**/*.md"), 0);
        assert_eq!(specificity("docs/**/*.md"), 1);
        assert_eq!(specificity("docs/decisions/*.md"), 2);
        assert_eq!(specificity("docs/decisions/important/*.md"), 3);
        assert_eq!(specificity("README.md"), 1);
    }

    #[test]
    fn no_rules_returns_empty() {
        let set = RuleSet::compile(&[]);
        let resolved = set.resolve("anything.md");
        assert!(resolved.is_empty());
    }

    #[test]
    fn no_match_returns_empty() {
        let rules = make_rules(&[(
            &["docs/**/*.md"],
            RuleChecks {
                analyze: Some(AnalyzeRuleConfig::default()),
                ..Default::default()
            },
        )]);
        let set = RuleSet::compile(&rules);
        let resolved = set.resolve("src/main.rs");
        assert!(resolved.is_empty());
    }

    #[test]
    fn single_match_returns_checks() {
        let rules = make_rules(&[(
            &["docs/**/*.md"],
            RuleChecks {
                analyze: Some(AnalyzeRuleConfig {
                    max_grade: Some(8.0),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )]);
        let set = RuleSet::compile(&rules);
        let resolved = set.resolve("docs/guide.md");
        assert!(resolved.analyze.is_some());
        assert_eq!(resolved.analyze.unwrap().max_grade, Some(8.0));
    }

    #[test]
    fn accumulates_different_checks_from_multiple_rules() {
        let rules = make_rules(&[
            (
                &["docs/**/*.md"],
                RuleChecks {
                    analyze: Some(AnalyzeRuleConfig {
                        max_grade: Some(8.0),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
            (
                &["docs/decisions/*.md"],
                RuleChecks {
                    completeness: Some(CompletenessRuleConfig {
                        template: "adr".to_string(),
                    }),
                    ..Default::default()
                },
            ),
        ]);
        let set = RuleSet::compile(&rules);
        let resolved = set.resolve("docs/decisions/001.md");
        // Gets analyze from general rule AND completeness from specific rule
        assert!(resolved.analyze.is_some());
        assert!(resolved.completeness.is_some());
        assert_eq!(resolved.completeness.unwrap().template, "adr");
    }

    #[test]
    fn specific_rule_overrides_general_for_same_check() {
        let rules = make_rules(&[
            (
                &["docs/**/*.md"],
                RuleChecks {
                    analyze: Some(AnalyzeRuleConfig {
                        max_grade: Some(8.0),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
            (
                &["docs/designs/*.md"],
                RuleChecks {
                    analyze: Some(AnalyzeRuleConfig {
                        max_grade: Some(12.0),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
        ]);
        let set = RuleSet::compile(&rules);
        let resolved = set.resolve("docs/designs/api.md");
        // More specific pattern wins
        assert_eq!(resolved.analyze.unwrap().max_grade, Some(12.0));
    }

    #[test]
    fn equal_specificity_earlier_rule_wins() {
        let rules = make_rules(&[
            (
                &["docs/*.md"],
                RuleChecks {
                    analyze: Some(AnalyzeRuleConfig {
                        max_grade: Some(8.0),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
            (
                &["docs/*.md"],
                RuleChecks {
                    analyze: Some(AnalyzeRuleConfig {
                        max_grade: Some(12.0),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
        ]);
        let set = RuleSet::compile(&rules);
        let resolved = set.resolve("docs/guide.md");
        // Equal specificity: first rule wins
        assert_eq!(resolved.analyze.unwrap().max_grade, Some(8.0));
    }

    #[test]
    fn multiple_paths_in_single_rule() {
        let rules = make_rules(&[(
            &["README.md", "docs/**/*.md"],
            RuleChecks {
                analyze: Some(AnalyzeRuleConfig::default()),
                ..Default::default()
            },
        )]);
        let set = RuleSet::compile(&rules);
        assert!(set.resolve("README.md").analyze.is_some());
        assert!(set.resolve("docs/guide.md").analyze.is_some());
        assert!(set.resolve("src/main.rs").analyze.is_none());
    }

    #[test]
    fn invalid_glob_skipped_gracefully() {
        let rules = make_rules(&[(
            &["[invalid", "docs/*.md"],
            RuleChecks {
                analyze: Some(AnalyzeRuleConfig::default()),
                ..Default::default()
            },
        )]);
        let set = RuleSet::compile(&rules);
        // Valid pattern still works
        assert!(set.resolve("docs/guide.md").analyze.is_some());
    }
}
```

**Step 2: Add module declaration**

In `crates/bito-lint-core/src/lib.rs`, add:

```rust
pub mod rules;
```

**Step 3: Run tests to verify they pass**

Run: `cargo test -p bito-lint-core rules::`
Expected: all PASS

**Step 4: Commit**

```bash
git add crates/bito-lint-core/src/rules.rs crates/bito-lint-core/src/lib.rs
git commit -m "feat: add rule resolution engine with specificity-based accumulation"
```

---

### Task 4: Inline Suppression Parser

**Files:**
- Create: `crates/bito-lint-core/src/directives.rs`
- Modify: `crates/bito-lint-core/src/lib.rs` (add `pub mod directives;`)

**Step 1: Create directives module with types and tests**

Create `crates/bito-lint-core/src/directives.rs`:

```rust
//! Inline suppression directives.
//!
//! Parses HTML comments in the form:
//! - `<!-- bito-lint disable check1,check2 -->` — suppress checks until re-enabled
//! - `<!-- bito-lint enable check1,check2 -->` — re-enable previously suppressed checks
//! - `<!-- bito-lint disable-next-line check1 -->` — suppress for the next line only
//!
//! Directives are parsed from the raw input before markdown stripping.

use std::collections::{HashMap, HashSet};

use regex::Regex;

/// Map of check names to their suppressed line ranges (1-indexed, inclusive).
///
/// A check present in this map with an empty vec means "file-level suppression"
/// (disable without a matching enable).
#[derive(Debug, Clone, Default)]
pub struct SuppressionMap {
    /// check name → list of suppressed line ranges (start, end) inclusive.
    suppressed: HashMap<String, Vec<(usize, usize)>>,
}

impl SuppressionMap {
    /// Returns `true` if the given check is suppressed at the given line.
    pub fn is_suppressed(&self, check: &str, line: usize) -> bool {
        match self.suppressed.get(check) {
            None => false,
            Some(ranges) => {
                if ranges.is_empty() {
                    // File-level suppression (no matching enable).
                    return true;
                }
                ranges.iter().any(|(start, end)| line >= *start && line <= *end)
            }
        }
    }

    /// Returns `true` if the given check is suppressed for the entire document.
    pub fn is_fully_suppressed(&self, check: &str) -> bool {
        matches!(self.suppressed.get(check), Some(ranges) if ranges.is_empty())
    }

    /// Returns `true` if no suppressions exist.
    pub fn is_empty(&self) -> bool {
        self.suppressed.is_empty()
    }

    /// All check names that have any suppression.
    pub fn suppressed_checks(&self) -> HashSet<&str> {
        self.suppressed.keys().map(String::as_str).collect()
    }
}

/// Parse suppression directives from raw input text.
///
/// Call this on the original text BEFORE markdown stripping.
pub fn parse_suppressions(input: &str) -> SuppressionMap {
    let re = Regex::new(
        r"<!--\s*bito-lint\s+(disable|enable|disable-next-line)\s+([\w,\s]+?)\s*-->"
    ).expect("directive regex should compile");

    let mut map = SuppressionMap::default();

    // Track open disable directives: check_name → line where disable started.
    let mut open: HashMap<String, usize> = HashMap::new();

    for (line_idx, line_text) in input.lines().enumerate() {
        let line_num = line_idx + 1; // 1-indexed

        for cap in re.captures_iter(line_text) {
            let action = &cap[1];
            let checks_str = &cap[2];
            let checks: Vec<String> = checks_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            match action {
                "disable" => {
                    for check in &checks {
                        open.insert(check.clone(), line_num);
                    }
                }
                "enable" => {
                    for check in &checks {
                        if let Some(start) = open.remove(check.as_str()) {
                            map.suppressed
                                .entry(check.clone())
                                .or_default()
                                .push((start, line_num));
                        }
                    }
                }
                "disable-next-line" => {
                    let next_line = line_num + 1;
                    for check in &checks {
                        map.suppressed
                            .entry(check.clone())
                            .or_default()
                            .push((next_line, next_line));
                    }
                }
                _ => {}
            }
        }
    }

    // Any unclosed disable → file-level suppression (empty ranges vec).
    for (check, _start) in open {
        map.suppressed.entry(check).or_default().clear();
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_directives_returns_empty() {
        let map = parse_suppressions("Just some text.\nNo directives here.");
        assert!(map.is_empty());
    }

    #[test]
    fn disable_enable_block() {
        let input = "\
Line 1.
<!-- bito-lint disable style -->
Line 3 suppressed.
Line 4 suppressed.
<!-- bito-lint enable style -->
Line 6 not suppressed.";
        let map = parse_suppressions(input);
        assert!(!map.is_suppressed("style", 1));
        assert!(map.is_suppressed("style", 2));
        assert!(map.is_suppressed("style", 3));
        assert!(map.is_suppressed("style", 4));
        assert!(map.is_suppressed("style", 5));
        assert!(!map.is_suppressed("style", 6));
    }

    #[test]
    fn disable_next_line() {
        let input = "\
Line 1.
<!-- bito-lint disable-next-line readability -->
Line 3 suppressed.
Line 4 not suppressed.";
        let map = parse_suppressions(input);
        assert!(!map.is_suppressed("readability", 2));
        assert!(map.is_suppressed("readability", 3));
        assert!(!map.is_suppressed("readability", 4));
    }

    #[test]
    fn multiple_checks_comma_separated() {
        let input = "<!-- bito-lint disable grammar,cliches -->\nSuppressed.\n<!-- bito-lint enable grammar,cliches -->";
        let map = parse_suppressions(input);
        assert!(map.is_suppressed("grammar", 2));
        assert!(map.is_suppressed("cliches", 2));
    }

    #[test]
    fn unclosed_disable_is_file_level() {
        let input = "<!-- bito-lint disable style -->\nRest of file.";
        let map = parse_suppressions(input);
        assert!(map.is_fully_suppressed("style"));
        assert!(map.is_suppressed("style", 1));
        assert!(map.is_suppressed("style", 100));
    }

    #[test]
    fn unrelated_check_not_affected() {
        let input = "<!-- bito-lint disable style -->\nText.\n<!-- bito-lint enable style -->";
        let map = parse_suppressions(input);
        assert!(!map.is_suppressed("grammar", 2));
    }

    #[test]
    fn multiple_regions_for_same_check() {
        let input = "\
<!-- bito-lint disable style -->
Region 1.
<!-- bito-lint enable style -->
Gap.
<!-- bito-lint disable style -->
Region 2.
<!-- bito-lint enable style -->";
        let map = parse_suppressions(input);
        assert!(map.is_suppressed("style", 2));
        assert!(!map.is_suppressed("style", 4));
        assert!(map.is_suppressed("style", 6));
    }
}
```

**Step 2: Add module declaration**

In `crates/bito-lint-core/src/lib.rs`, add:

```rust
pub mod directives;
```

**Step 3: Run tests**

Run: `cargo test -p bito-lint-core directives::`
Expected: all PASS

**Step 4: Commit**

```bash
git add crates/bito-lint-core/src/directives.rs crates/bito-lint-core/src/lib.rs
git commit -m "feat: add inline suppression directive parser"
```

---

### Task 5: Lint Execution Engine

This is the shared core that both the CLI `lint` command and MCP `lint_file` tool call.

**Files:**
- Create: `crates/bito-lint-core/src/lint.rs`
- Modify: `crates/bito-lint-core/src/lib.rs` (add `pub mod lint;`)

**Step 1: Write the lint module with report types and executor**

Create `crates/bito-lint-core/src/lint.rs`. This module:
- Takes `ResolvedChecks` + file content + project-wide `Config` defaults
- Runs each configured check
- Returns a combined `LintReport`

```rust
//! Lint execution engine.
//!
//! Runs the checks specified by [`ResolvedChecks`] against file content,
//! applying project-wide config defaults where rule-level settings are absent.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::analysis::{self, reports::FullAnalysisReport};
use crate::completeness::{self, CompletenessReport};
use crate::config::{
    AnalyzeRuleConfig, CompletenessRuleConfig, Config, Dialect, GrammarRuleConfig,
    ReadabilityRuleConfig, TokensRuleConfig,
};
use crate::directives;
use crate::grammar::{self, GrammarReport};
use crate::readability::{self, ReadabilityReport};
use crate::rules::ResolvedChecks;
use crate::tokens::{self, Backend, TokenReport};

/// Combined results from all checks run by the lint engine.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LintReport {
    /// The file that was linted.
    pub file: String,
    /// Full analysis report (18 checks), if `analyze` was configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analyze: Option<FullAnalysisReport>,
    /// Standalone readability report, if `readability` was configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readability: Option<ReadabilityReport>,
    /// Standalone grammar report, if `grammar` was configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar: Option<GrammarReport>,
    /// Completeness report, if `completeness` was configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completeness: Option<CompletenessReport>,
    /// Token count report, if `tokens` was configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenReport>,
    /// Overall pass/fail. `true` only if every check passes its thresholds.
    pub pass: bool,
}

/// Run all checks specified by `resolved` against `content`.
///
/// Settings cascade: rule-level overrides config-level defaults.
/// The `file_path` is used for reporting only.
/// The `config` provides project-wide defaults and custom templates.
pub fn run_lint(
    file_path: &str,
    content: &str,
    resolved: &ResolvedChecks,
    config: &Config,
) -> anyhow::Result<LintReport> {
    let strip_md = file_path.ends_with(".md");
    let _suppressions = directives::parse_suppressions(content);
    let mut pass = true;

    // --- analyze ---
    let analyze_report = if let Some(ref ac) = resolved.analyze {
        let max_grade = ac.max_grade.or(config.max_grade);
        let passive_max = ac.passive_max.or(config.passive_max_percent);
        let dialect = ac.dialect.or(config.dialect);
        // Resolve checks/exclude into final check list.
        let check_list = resolve_analyze_checks(ac)?;
        let checks_ref = check_list.as_deref();

        match analysis::run_full_analysis(content, strip_md, checks_ref, max_grade, passive_max, dialect) {
            Ok(report) => {
                // Check style gate.
                let style_min = ac.style_min.or(config.style_min_score);
                if let (Some(min), Some(ref st)) = (style_min, &report.style) {
                    if st.style_score < min {
                        pass = false;
                    }
                }
                Some(report)
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        None
    };

    // --- readability ---
    let readability_report = if let Some(ref rc) = resolved.readability {
        let max_grade = rc.max_grade.or(config.max_grade);
        match readability::check_readability(content, strip_md, max_grade) {
            Ok(report) => {
                if report.over_max {
                    pass = false;
                }
                Some(report)
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        None
    };

    // --- grammar ---
    let grammar_report = if let Some(ref gc) = resolved.grammar {
        let passive_max = gc.passive_max.or(config.passive_max_percent);
        match grammar::check_grammar_full(content, strip_md, passive_max) {
            Ok(report) => {
                if report.over_max {
                    pass = false;
                }
                Some(report)
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        None
    };

    // --- completeness ---
    let completeness_report = if let Some(ref cc) = resolved.completeness {
        let custom_templates = config.templates.as_ref();
        match completeness::check_completeness(content, &cc.template, custom_templates) {
            Ok(report) => {
                if !report.pass {
                    pass = false;
                }
                Some(report)
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        None
    };

    // --- tokens ---
    let tokens_report = if let Some(ref tc) = resolved.tokens {
        let backend = tc.tokenizer.or(config.tokenizer).unwrap_or_default();
        match tokens::count_tokens(content, tc.budget, backend) {
            Ok(report) => {
                if report.over_budget {
                    pass = false;
                }
                Some(report)
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        None
    };

    Ok(LintReport {
        file: file_path.to_string(),
        analyze: analyze_report,
        readability: readability_report,
        grammar: grammar_report,
        completeness: completeness_report,
        tokens: tokens_report,
        pass,
    })
}

/// Resolve analyze checks/exclude into final check list.
fn resolve_analyze_checks(ac: &AnalyzeRuleConfig) -> anyhow::Result<Option<Vec<String>>> {
    use std::collections::HashSet;
    use crate::analysis::ALL_CHECKS;

    match (&ac.checks, &ac.exclude) {
        (Some(checks), None) => Ok(Some(checks.clone())),
        (None, Some(exclude)) => {
            let excluded: HashSet<&str> = exclude.iter().map(String::as_str).collect();
            let remaining: Vec<String> = ALL_CHECKS
                .iter()
                .filter(|name| !excluded.contains(*name))
                .map(|s| (*s).to_string())
                .collect();
            Ok(Some(remaining))
        }
        (Some(_), Some(_)) => {
            anyhow::bail!("rule cannot specify both 'checks' and 'exclude' for analyze");
        }
        (None, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::rules::ResolvedChecks;

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn empty_resolved_checks_produces_empty_report() {
        let resolved = ResolvedChecks::default();
        let report = run_lint("test.md", "Some text.", &resolved, &default_config()).unwrap();
        assert!(report.pass);
        assert!(report.analyze.is_none());
        assert!(report.readability.is_none());
        assert!(report.tokens.is_none());
    }

    #[test]
    fn analyze_check_runs() {
        let resolved = ResolvedChecks {
            analyze: Some(AnalyzeRuleConfig::default()),
            ..Default::default()
        };
        let report = run_lint(
            "doc.md",
            "The cat sat on the mat. The dog ran fast.",
            &resolved,
            &default_config(),
        )
        .unwrap();
        assert!(report.analyze.is_some());
    }

    #[test]
    fn tokens_over_budget_fails() {
        let resolved = ResolvedChecks {
            tokens: Some(TokensRuleConfig {
                budget: Some(1), // impossibly small
                tokenizer: None,
            }),
            ..Default::default()
        };
        let report = run_lint(
            "doc.md",
            "The cat sat on the mat.",
            &resolved,
            &default_config(),
        )
        .unwrap();
        assert!(!report.pass);
        assert!(report.tokens.unwrap().over_budget);
    }

    #[test]
    fn completeness_missing_sections_fails() {
        let resolved = ResolvedChecks {
            completeness: Some(CompletenessRuleConfig {
                template: "handoff".to_string(),
            }),
            ..Default::default()
        };
        let report = run_lint(
            "doc.md",
            "## Where things stand\n\nDone.",
            &resolved,
            &default_config(),
        )
        .unwrap();
        assert!(!report.pass);
    }

    #[test]
    fn config_defaults_cascade_to_analyze() {
        let config = Config {
            max_grade: Some(1.0), // Very strict
            ..Default::default()
        };
        let resolved = ResolvedChecks {
            analyze: Some(AnalyzeRuleConfig::default()), // No rule-level override
            ..Default::default()
        };
        let report = run_lint(
            "doc.md",
            "The cat sat on the mat. The dog ran fast.",
            &resolved,
            &config,
        )
        .unwrap();
        // Analysis should run with max_grade from config
        assert!(report.analyze.is_some());
    }

    #[test]
    fn rule_settings_override_config() {
        let config = Config {
            max_grade: Some(1.0),
            ..Default::default()
        };
        let resolved = ResolvedChecks {
            readability: Some(ReadabilityRuleConfig {
                max_grade: Some(20.0), // Rule overrides config
            }),
            ..Default::default()
        };
        let report = run_lint(
            "doc.md",
            "The cat sat on the mat.",
            &resolved,
            &config,
        )
        .unwrap();
        // Should pass because rule max_grade is generous
        assert!(report.readability.is_some());
        assert!(report.pass);
    }
}
```

**Step 2: Add module declaration**

In `crates/bito-lint-core/src/lib.rs`, add:

```rust
pub mod lint;
```

**Step 3: Run tests**

Run: `cargo test -p bito-lint-core lint::`
Expected: all PASS

**Step 4: Commit**

```bash
git add crates/bito-lint-core/src/lint.rs crates/bito-lint-core/src/lib.rs
git commit -m "feat: add lint execution engine"
```

---

### Task 6: `lint` CLI Subcommand

**Files:**
- Create: `crates/bito-lint/src/commands/lint.rs`
- Modify: `crates/bito-lint/src/commands/mod.rs` (add `pub mod lint;`)
- Modify: `crates/bito-lint/src/lib.rs` (add `Lint` variant to `Commands`)
- Modify: `crates/bito-lint/src/main.rs` (add dispatch + pass `Config` to lint)

**Step 1: Create the lint command**

Create `crates/bito-lint/src/commands/lint.rs`:

```rust
//! Lint command --- run path-based quality checks on a file.
//!
//! Matches the file against configured `rules` in the config file,
//! resolves which checks apply, and runs them all. This is the
//! CLI counterpart of the `lint_file` MCP tool.

use anyhow::{Context, bail};
use camino::Utf8PathBuf;
use clap::Args;
use owo_colors::OwoColorize;
use tracing::{debug, instrument};

use bito_lint_core::config::Config;
use bito_lint_core::lint;
use bito_lint_core::rules::RuleSet;

use super::read_input_file;

/// Arguments for the `lint` subcommand.
#[derive(Args, Debug)]
pub struct LintArgs {
    /// File to lint.
    pub file: Utf8PathBuf,
}

/// Lint a file according to project rules.
#[instrument(name = "cmd_lint", skip_all, fields(file = %args.file))]
pub fn cmd_lint(
    args: LintArgs,
    global_json: bool,
    config: &Config,
    max_input_bytes: Option<usize>,
) -> anyhow::Result<()> {
    debug!(file = %args.file, "executing lint command");

    let rules = match config.rules {
        Some(ref rules) => rules,
        None => {
            if !global_json {
                println!("{} no rules configured", "SKIP:".dimmed());
            }
            return Ok(());
        }
    };

    let rule_set = RuleSet::compile(rules);
    let file_str = args.file.as_str();
    let resolved = rule_set.resolve(file_str);

    if resolved.is_empty() {
        debug!(file = %args.file, "no rules match this file");
        if !global_json {
            println!("{} no rules match {}", "SKIP:".dimmed(), args.file);
        }
        return Ok(());
    }

    let content = read_input_file(&args.file, max_input_bytes)?;

    let report = lint::run_lint(file_str, &content, &resolved, config)
        .with_context(|| format!("failed to lint {}", args.file))?;

    if global_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    // Text output
    println!("{}", args.file.bold());

    if let Some(ref a) = report.analyze {
        if let Some(ref st) = a.style {
            let score_str = if st.style_score >= 80 {
                format!("{}", st.style_score).green().to_string()
            } else if st.style_score >= 60 {
                format!("{}", st.style_score).yellow().to_string()
            } else {
                format!("{}", st.style_score).red().to_string()
            };
            println!("  {} style {}/100", "analyze:".cyan(), score_str);
        }
        if let Some(ref r) = a.readability {
            println!("  {} grade {:.1}", "analyze:".cyan(), r.grade);
        }
    }

    if let Some(ref r) = report.readability {
        let status = if r.over_max { "FAIL".red() } else { "PASS".green() };
        println!("  {} {} grade {:.1}", "readability:".cyan(), status, r.grade);
    }

    if let Some(ref g) = report.grammar {
        let status = if g.over_max { "FAIL".red() } else { "PASS".green() };
        println!(
            "  {} {} {:.1}% passive",
            "grammar:".cyan(),
            status,
            g.passive_percentage,
        );
    }

    if let Some(ref c) = report.completeness {
        let status = if c.pass { "PASS".green() } else { "FAIL".red() };
        println!("  {} {} ({})", "completeness:".cyan(), status, c.template);
    }

    if let Some(ref t) = report.tokens {
        let status = if t.over_budget { "FAIL".red() } else { "PASS".green() };
        if let Some(budget) = t.budget {
            println!("  {} {} {}/{}", "tokens:".cyan(), status, t.count, budget);
        } else {
            println!("  {} {}", "tokens:".cyan(), t.count);
        }
    }

    if !report.pass {
        bail!("{} failed lint checks", args.file);
    }

    Ok(())
}
```

**Step 2: Wire into CLI**

In `crates/bito-lint/src/commands/mod.rs`, add:

```rust
pub mod lint;
```

In `crates/bito-lint/src/lib.rs`, add to `Commands` enum:

```rust
/// Lint a file according to project rules
Lint(commands::lint::LintArgs),
```

In `crates/bito-lint/src/main.rs`, add to the `match command` block:

```rust
Commands::Lint(args) => commands::lint::cmd_lint(args, cli.json, &config, max_input),
```

**Step 3: Verify it compiles**

Run: `cargo check -p bito-lint`
Expected: success

**Step 4: Add CLI integration tests**

Add to `crates/bito-lint/tests/cli.rs`:

```rust
// =============================================================================
// Lint Command
// =============================================================================

#[test]
fn lint_no_rules_skips() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "The cat sat on the mat.").unwrap();
    cmd()
        .args(["lint", tmp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("no rules"));
}

#[test]
fn lint_help_shows_usage() {
    cmd()
        .args(["lint", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Lint a file"));
}
```

**Step 5: Run all tests**

Run: `cargo test`
Expected: all PASS

**Step 6: Commit**

```bash
git add crates/bito-lint/src/commands/lint.rs crates/bito-lint/src/commands/mod.rs \
       crates/bito-lint/src/lib.rs crates/bito-lint/src/main.rs \
       crates/bito-lint/tests/cli.rs
git commit -m "feat: add 'lint' CLI subcommand for path-based rules"
```

---

### Task 7: `lint_file` MCP Tool

**Files:**
- Modify: `crates/bito-lint/src/server.rs` (add `lint_file` tool, expand `ProjectServer` to hold `Config`)
- Modify: `crates/bito-lint/src/commands/serve.rs` (pass `Config` to server)
- Modify: `crates/bito-lint/src/main.rs` (pass config to serve command)

**Step 1: Expand `ProjectServer` to hold `Config`**

In `server.rs`, change:

```rust
#[derive(Clone)]
pub struct ProjectServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
    max_input_bytes: Option<usize>,
    config: bito_lint_core::Config,
}
```

Update `new()`:

```rust
pub fn new() -> Self {
    Self {
        tool_router: Self::tool_router(),
        max_input_bytes: Some(core::DEFAULT_MAX_INPUT_BYTES),
        config: bito_lint_core::Config::default(),
    }
}
```

Add builder method:

```rust
pub fn with_config(mut self, config: bito_lint_core::Config) -> Self {
    self.config = config;
    self
}
```

**Step 2: Add `LintFileParams` and `lint_file` tool**

```rust
/// Parameters for the `lint_file` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LintFileParams {
    /// File path (relative to project root) for rule matching.
    pub file_path: String,
    /// The file contents to lint.
    pub text: String,
}
```

Add tool method inside the `#[tool_router] impl ProjectServer` block:

```rust
/// Lint a file according to project rules.
#[tool(
    description = "Lint a file according to configured project rules. Matches the file path against rules in the config, runs all applicable checks (analysis, readability, grammar, completeness, tokens), and returns combined results with pass/fail status."
)]
#[tracing::instrument(skip(self, params), fields(otel.kind = "server", file = %params.file_path))]
fn lint_file(
    &self,
    #[allow(unused_variables)] Parameters(params): Parameters<LintFileParams>,
) -> Result<CallToolResult, McpError> {
    tracing::debug!(tool = "lint_file", file = %params.file_path, "executing MCP tool");
    self.validate_input(&params.text)?;

    let rules = self.config.rules.as_deref().unwrap_or_default();
    let rule_set = bito_lint_core::rules::RuleSet::compile(rules);
    let resolved = rule_set.resolve(&params.file_path);

    if resolved.is_empty() {
        return Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "file": params.file_path,
                "matched": false,
                "message": "no rules match this file path"
            })
            .to_string(),
        )]));
    }

    let report =
        bito_lint_core::lint::run_lint(&params.file_path, &params.text, &resolved, &self.config)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?;

    tracing::info!(tool = "lint_file", pass = report.pass, "MCP tool completed");
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
```

**Step 3: Update serve command to pass Config**

In `crates/bito-lint/src/commands/serve.rs`, change signature:

```rust
pub async fn cmd_serve(
    _args: ServeArgs,
    max_input_bytes: Option<usize>,
    config: bito_lint_core::Config,
) -> Result<()> {
    tracing::info!("starting MCP server on stdio");

    let server = ProjectServer::new()
        .with_max_input_bytes(max_input_bytes)
        .with_config(config);
    let service = server.serve(rmcp::transport::stdio()).await?;
    // ...
}
```

In `main.rs`, update the `Serve` dispatch to pass config:

```rust
Commands::Serve(args) => {
    let rt = tokio::runtime::Runtime::new()
        .context("failed to create async runtime for MCP server")?;
    rt.block_on(commands::serve::cmd_serve(args, max_input, config))
}
```

Note: `config` must be moved into serve since it's consumed. Handle ownership by cloning or restructuring the match. If `config` is used after the match, clone it for serve:

```rust
Commands::Serve(args) => {
    let rt = tokio::runtime::Runtime::new()
        .context("failed to create async runtime for MCP server")?;
    rt.block_on(commands::serve::cmd_serve(args, max_input, config.clone()))
}
```

But since `Serve` is the last arm and `config` is not used after, a move may work. Check the borrow checker and adjust.

**Step 4: Add tests**

In server.rs test module:

```rust
#[test]
fn lint_file_no_rules_returns_no_match() {
    let server = ProjectServer::new(); // No rules in default config
    let params = Parameters(LintFileParams {
        file_path: "docs/guide.md".to_string(),
        text: "The cat sat on the mat.".to_string(),
    });

    let result = server.lint_file(params).expect("lint_file should succeed");
    assert!(!result.is_error.unwrap_or(false));

    let text = extract_text(&result).expect("should have text content");
    let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(json["matched"], false);
}

#[test]
fn lint_file_with_rules_runs_checks() {
    use bito_lint_core::config::{Rule, RuleChecks, ReadabilityRuleConfig};

    let config = bito_lint_core::Config {
        rules: Some(vec![Rule {
            paths: vec!["docs/**/*.md".to_string()],
            checks: RuleChecks {
                readability: Some(ReadabilityRuleConfig { max_grade: Some(20.0) }),
                ..Default::default()
            },
        }]),
        ..Default::default()
    };

    let server = ProjectServer::new().with_config(config);
    let params = Parameters(LintFileParams {
        file_path: "docs/guide.md".to_string(),
        text: "The cat sat on the mat. The dog ran fast.".to_string(),
    });

    let result = server.lint_file(params).expect("lint_file should succeed");
    assert!(!result.is_error.unwrap_or(false));

    let text = extract_text(&result).expect("should have text content");
    let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert!(json["pass"].as_bool().unwrap());
    assert!(json["readability"].is_object());
}
```

**Step 5: Run all tests**

Run: `cargo test`
Expected: all PASS. Watch the `mcp_tool_schemas_fit_token_budget` test --- the new tool adds schema tokens. If it exceeds 4000, trim tool descriptions.

**Step 6: Commit**

```bash
git add crates/bito-lint/src/server.rs crates/bito-lint/src/commands/serve.rs \
       crates/bito-lint/src/main.rs
git commit -m "feat: add lint_file MCP tool with config-aware rule resolution"
```

---

### Task 8: Documentation --- When to Use What

**Files:**
- Modify: `docs/README.md` (replace placeholder with real docs)

**Step 1: Write command comparison and rules documentation**

Replace the placeholder `docs/README.md` with proper documentation covering:

- **Commands overview:** What each command does and when to use it
- **`lint` vs `analyze`:** `lint` is config-driven (rules determine what runs for a file path), `analyze` is ad-hoc (you choose what to run). Use `lint` in CI/hooks, `analyze` for interactive exploration.
- **Quick-gate commands** (`readability`, `grammar`, `completeness`, `tokens`): Single-purpose pass/fail gates. Use directly when you only need one check with a threshold.
- **Rules configuration:** Full example config with rules, explanation of accumulation and specificity.
- **Inline suppressions:** Syntax reference with examples.
- **Config file reference:** All config fields with descriptions.

Keep it concise and scannable. Refer to `--help` output for flag details rather than duplicating them.

**Step 2: Verify links and formatting**

Review the doc reads correctly.

**Step 3: Commit**

```bash
git add docs/README.md
git commit -m "docs: add command guide, rules reference, and config documentation"
```

---

### Task 9: End-to-End Integration Test

**Files:**
- Modify: `crates/bito-lint/tests/cli.rs`

**Step 1: Write an integration test using a config file with rules**

```rust
#[test]
fn lint_with_config_rules_runs_checks() {
    let dir = tempfile::tempdir().unwrap();

    // Create a config file with rules
    let config_path = dir.path().join(".bito-lint.yaml");
    std::fs::write(
        &config_path,
        r#"
rules:
  - paths: ["docs/**/*.md"]
    checks:
      readability:
        max_grade: 20
"#,
    )
    .unwrap();

    // Create the file to lint (matching path)
    let docs_dir = dir.path().join("docs");
    std::fs::create_dir_all(&docs_dir).unwrap();
    let file_path = docs_dir.join("guide.md");
    std::fs::write(&file_path, "The cat sat on the mat. The dog ran fast.").unwrap();

    cmd()
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "--config",
            config_path.to_str().unwrap(),
            "lint",
            "docs/guide.md",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("readability"));
}

#[test]
fn lint_no_match_skips_cleanly() {
    let dir = tempfile::tempdir().unwrap();

    let config_path = dir.path().join(".bito-lint.yaml");
    std::fs::write(
        &config_path,
        "rules:\n  - paths: [\"docs/**/*.md\"]\n    checks:\n      readability:\n        max_grade: 20\n",
    )
    .unwrap();

    let file_path = dir.path().join("random.txt");
    std::fs::write(&file_path, "Some text.").unwrap();

    cmd()
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "lint",
            file_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("no rules match"));
}
```

**Step 2: Run**

Run: `cargo test --test cli lint_with_config`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/bito-lint/tests/cli.rs
git commit -m "test: add end-to-end integration tests for lint command with rules"
```

---

### Task 10: Run Full Suite and Clippy

**Step 1: Full test suite**

Run: `cargo nextest run`
Expected: all PASS

**Step 2: Clippy**

Run: `cargo clippy --all-targets`
Expected: no warnings

**Step 3: Final commit (if any fixups needed)**

```bash
git commit -m "chore: clippy and test fixes"
```
