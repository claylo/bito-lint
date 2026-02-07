//! Spelling and formatting consistency analysis.

use crate::word_lists::{HYPHEN_PATTERNS, US_UK_PAIRS};

use super::reports::ConsistencyReport;

/// Check for inconsistent spelling (US/UK) and hyphenation.
#[tracing::instrument(skip_all)]
pub fn analyze_consistency(text: &str) -> ConsistencyReport {
    let lower = text.to_lowercase();
    let mut issues = Vec::new();

    // US vs UK spelling
    for (us, uk) in US_UK_PAIRS.iter() {
        let has_us = lower.contains(us);
        let has_uk = lower.contains(uk);
        if has_us && has_uk {
            issues.push(format!(
                "Mixed US/UK spelling: both \"{us}\" and \"{uk}\" found"
            ));
        }
    }

    // Hyphenation variants
    for (joined, hyphenated) in HYPHEN_PATTERNS.iter() {
        let has_joined = lower.contains(joined);
        let has_hyphenated = lower.contains(hyphenated);
        if has_joined && has_hyphenated {
            issues.push(format!(
                "Inconsistent hyphenation: both \"{joined}\" and \"{hyphenated}\" found"
            ));
        }
    }

    let total = issues.len();
    ConsistencyReport {
        total_issues: total,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_issues_consistent_text() {
        let report = analyze_consistency("The color of the system is organized well.");
        assert_eq!(report.total_issues, 0);
    }

    #[test]
    fn detects_us_uk_mixing() {
        let report = analyze_consistency("The color was nice but the colour was better.");
        assert_eq!(report.total_issues, 1);
        assert!(report.issues[0].contains("color"));
        assert!(report.issues[0].contains("colour"));
    }

    #[test]
    fn detects_hyphenation_inconsistency() {
        let report = analyze_consistency("Send an email to the e-mail address.");
        assert_eq!(report.total_issues, 1);
        assert!(report.issues[0].contains("email"));
        assert!(report.issues[0].contains("e-mail"));
    }

    #[test]
    fn detects_both_issue_types() {
        let report = analyze_consistency(
            "The color and colour of the email and e-mail are different.",
        );
        assert_eq!(report.total_issues, 2);
    }

    #[test]
    fn case_insensitive() {
        let report = analyze_consistency("The COLOR was nice but the Colour was better.");
        assert_eq!(report.total_issues, 1);
    }
}
