//! Lint execution engine.
//!
//! Runs the checks specified by [`ResolvedChecks`] against file content,
//! applying project-wide config defaults where rule-level settings are absent.

use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::analysis::{self, ALL_CHECKS, FullAnalysisReport};
use crate::completeness::{self, CompletenessReport};
use crate::config::{AnalyzeRuleConfig, Config};
use crate::directives;
use crate::error::{AnalysisError, AnalysisResult};
use crate::grammar::{self, GrammarReport};
use crate::readability::{self, ReadabilityReport};
use crate::rules::ResolvedChecks;
use crate::tokens::{self, TokenReport};

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
) -> AnalysisResult<LintReport> {
    let strip_md = file_path.ends_with(".md");
    let suppressions = directives::parse_suppressions(content);
    let mut pass = true;

    // --- analyze ---
    let analyze_report = if let Some(ref ac) = resolved.analyze {
        // Filter out fully-suppressed sub-checks from the analyze check list.
        let mut check_list = resolve_analyze_checks(ac)?;
        if !suppressions.is_empty()
            && let Some(ref mut checks) = check_list
        {
            checks.retain(|c| !suppressions.is_fully_suppressed(c));
        }
        // If all requested checks were suppressed, skip analyze entirely.
        if check_list.as_ref().is_some_and(|c| c.is_empty()) {
            None
        } else {
            let max_grade = ac.max_grade.or(config.max_grade);
            let passive_max = ac.passive_max.or(config.passive_max_percent);
            let dialect = ac.dialect.or(config.dialect);
            let checks_ref = check_list.as_deref();

            let report = analysis::run_full_analysis(
                content, strip_md, checks_ref, max_grade, passive_max, dialect,
            )?;
            let style_min = ac.style_min.or(config.style_min_score);
            if let (Some(min), Some(st)) = (style_min, &report.style)
                && st.style_score < min
            {
                pass = false;
            }
            Some(report)
        }
    } else {
        None
    };

    // --- readability ---
    let readability_report = if let Some(ref rc) = resolved.readability
        && !suppressions.is_fully_suppressed("readability")
    {
        let max_grade = rc.max_grade.or(config.max_grade);
        let report = readability::check_readability(content, strip_md, max_grade)?;
        if report.over_max {
            pass = false;
        }
        Some(report)
    } else {
        None
    };

    // --- grammar ---
    let grammar_report = if let Some(ref gc) = resolved.grammar
        && !suppressions.is_fully_suppressed("grammar")
    {
        let passive_max = gc.passive_max.or(config.passive_max_percent);
        let report = grammar::check_grammar_full(content, strip_md, passive_max)?;
        if report.over_max {
            pass = false;
        }
        Some(report)
    } else {
        None
    };

    // --- completeness ---
    let completeness_report = if let Some(ref cc) = resolved.completeness
        && !suppressions.is_fully_suppressed("completeness")
    {
        let custom_templates = config.templates.as_ref();
        let report = completeness::check_completeness(content, &cc.template, custom_templates)?;
        if !report.pass {
            pass = false;
        }
        Some(report)
    } else {
        None
    };

    // --- tokens ---
    let tokens_report = if let Some(ref tc) = resolved.tokens
        && !suppressions.is_fully_suppressed("tokens")
    {
        let backend = tc.tokenizer.or(config.tokenizer).unwrap_or_default();
        let report = tokens::count_tokens(content, tc.budget, backend)?;
        if report.over_budget {
            pass = false;
        }
        Some(report)
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
fn resolve_analyze_checks(ac: &AnalyzeRuleConfig) -> AnalysisResult<Option<Vec<String>>> {
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
        (Some(_), Some(_)) => Err(AnalysisError::ConflictingConfig(
            "rule cannot specify both 'checks' and 'exclude' for analyze".to_string(),
        )),
        (None, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AnalyzeRuleConfig, CompletenessRuleConfig, Config, ReadabilityRuleConfig,
        TokensRuleConfig,
    };
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
                budget: Some(1),
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
            max_grade: Some(1.0),
            ..Default::default()
        };
        let resolved = ResolvedChecks {
            analyze: Some(AnalyzeRuleConfig::default()),
            ..Default::default()
        };
        let report = run_lint(
            "doc.md",
            "The cat sat on the mat. The dog ran fast.",
            &resolved,
            &config,
        )
        .unwrap();
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
                max_grade: Some(20.0),
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
        assert!(report.readability.is_some());
        assert!(report.pass);
    }

    #[test]
    fn analyze_checks_and_exclude_conflict_errors() {
        let resolved = ResolvedChecks {
            analyze: Some(AnalyzeRuleConfig {
                checks: Some(vec!["readability".to_string()]),
                exclude: Some(vec!["style".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = run_lint(
            "doc.md",
            "The cat sat on the mat.",
            &resolved,
            &default_config(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("cannot specify both"));
    }

    #[test]
    fn file_level_suppression_skips_check() {
        let resolved = ResolvedChecks {
            readability: Some(ReadabilityRuleConfig {
                max_grade: Some(1.0), // would fail
            }),
            ..Default::default()
        };
        // Unclosed disable → file-level suppression
        let content = "<!-- bito-lint disable readability -->\nThe cat sat on the mat.";
        let report = run_lint("doc.md", content, &resolved, &default_config()).unwrap();
        // Readability check should be skipped entirely
        assert!(report.readability.is_none());
        assert!(report.pass);
    }

    #[test]
    fn suppression_does_not_affect_other_checks() {
        let resolved = ResolvedChecks {
            readability: Some(ReadabilityRuleConfig {
                max_grade: Some(20.0),
            }),
            tokens: Some(TokensRuleConfig {
                budget: Some(1_000_000),
                tokenizer: None,
            }),
            ..Default::default()
        };
        // Suppress readability only — tokens should still run
        let content = "<!-- bito-lint disable readability -->\nThe cat sat on the mat.";
        let report = run_lint("doc.md", content, &resolved, &default_config()).unwrap();
        assert!(report.readability.is_none());
        assert!(report.tokens.is_some());
    }
}
