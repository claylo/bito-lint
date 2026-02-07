//! Analyze command — comprehensive writing analysis.

use anyhow::{Context, bail};
use camino::Utf8PathBuf;
use clap::Args;
use owo_colors::OwoColorize;
use tracing::{debug, instrument};

use bito_lint_core::analysis;

/// Arguments for the `analyze` subcommand.
#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// File to analyze.
    pub file: Utf8PathBuf,

    /// Checks to run (comma-separated). Omit for all checks.
    #[arg(long, value_delimiter = ',')]
    pub checks: Option<Vec<String>>,

    /// Minimum acceptable style score (0–100).
    #[arg(long)]
    pub style_min: Option<i32>,
}

/// Run comprehensive writing analysis on a file.
#[instrument(name = "cmd_analyze", skip_all, fields(file = %args.file))]
pub fn cmd_analyze(
    args: AnalyzeArgs,
    global_json: bool,
    config_style_min: Option<i32>,
    config_max_grade: Option<f64>,
    config_passive_max: Option<f64>,
) -> anyhow::Result<()> {
    debug!(file = %args.file, checks = ?args.checks, "executing analyze command");

    let content = std::fs::read_to_string(args.file.as_std_path())
        .with_context(|| format!("failed to read {}", args.file))?;

    let strip_md = args.file.extension() == Some("md");
    let style_min = args.style_min.or(config_style_min);

    let checks_ref = args.checks.as_deref();
    let report = analysis::run_full_analysis(
        &content,
        strip_md,
        checks_ref,
        config_max_grade,
        config_passive_max,
    )
    .with_context(|| format!("failed to analyze {}", args.file))?;

    if global_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    // Text output — section by section
    println!("{}", args.file.bold());

    if let Some(ref r) = report.readability {
        println!(
            "\n  {} Grade {:.1}, {} sentences, {} words",
            "Readability:".cyan(),
            r.grade,
            r.sentences,
            r.words,
        );
    }

    if let Some(ref g) = report.grammar {
        println!(
            "\n  {} {} issues, {} passive ({:.1}%)",
            "Grammar:".cyan(),
            g.issues.len(),
            g.passive_count,
            g.passive_percentage,
        );
    }

    if let Some(ref s) = report.sticky_sentences {
        println!(
            "\n  {} Glue index {:.1}%, {} sticky sentences",
            "Sticky:".cyan(),
            s.overall_glue_index,
            s.sticky_count,
        );
    }

    if let Some(ref p) = report.pacing {
        println!(
            "\n  {} Fast {:.0}% / Medium {:.0}% / Slow {:.0}%",
            "Pacing:".cyan(),
            p.fast_percentage,
            p.medium_percentage,
            p.slow_percentage,
        );
    }

    if let Some(ref sl) = report.sentence_length {
        println!(
            "\n  {} Avg {:.1} words, variety {:.1}/10",
            "Length:".cyan(),
            sl.avg_length,
            sl.variety_score,
        );
    }

    if let Some(ref t) = report.transitions {
        println!(
            "\n  {} {:.0}% of sentences, {} unique",
            "Transitions:".cyan(),
            t.transition_percentage,
            t.unique_transitions,
        );
    }

    if let Some(ref o) = report.overused_words
        && !o.overused_words.is_empty()
    {
        let top: Vec<_> = o
            .overused_words
            .iter()
            .take(5)
            .map(|w| format!("\"{}\" ({:.1}%)", w.word, w.frequency))
            .collect();
        println!("\n  {} {}", "Overused:".cyan(), top.join(", "),);
    }

    if let Some(ref d) = report.diction
        && d.total_vague > 0
    {
        println!("\n  {} {} vague words", "Diction:".cyan(), d.total_vague,);
    }

    if let Some(ref c) = report.cliches
        && c.total_cliches > 0
    {
        println!(
            "\n  {} {} clichés found",
            "Clichés:".yellow(),
            c.total_cliches,
        );
    }

    if let Some(ref j) = report.jargon
        && j.total_jargon > 0
    {
        println!("\n  {} {} jargon terms", "Jargon:".yellow(), j.total_jargon,);
    }

    if let Some(ref st) = report.style {
        let score_str = if st.style_score >= 80 {
            format!("{}", st.style_score).green().to_string()
        } else if st.style_score >= 60 {
            format!("{}", st.style_score).yellow().to_string()
        } else {
            format!("{}", st.style_score).red().to_string()
        };
        println!(
            "\n  {} Score {}/100, {} adverbs, {} hidden verbs",
            "Style:".cyan(),
            score_str,
            st.adverb_count,
            st.hidden_verbs.len(),
        );
    }

    // Check style score gate
    if let (Some(min), Some(st)) = (style_min, &report.style)
        && st.style_score < min
    {
        bail!(
            "{} style score {} is below minimum {} — improve writing quality.",
            args.file,
            st.style_score,
            min,
        );
    }

    Ok(())
}
