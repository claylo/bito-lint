//! Tokens command — count tokens in a file.

use anyhow::{Context, bail};
use camino::Utf8PathBuf;
use clap::Args;
use owo_colors::OwoColorize;
use tracing::{debug, instrument};

use bito_lint_core::tokens;

/// Arguments for the `tokens` subcommand.
#[derive(Args, Debug)]
pub struct TokensArgs {
    /// File to analyze.
    pub file: Utf8PathBuf,

    /// Maximum token budget.
    #[arg(long)]
    pub budget: Option<usize>,
}

/// Count tokens in a file and optionally check against a budget.
#[instrument(name = "cmd_tokens", skip_all, fields(file = %args.file))]
pub fn cmd_tokens(
    args: TokensArgs,
    global_json: bool,
    config_budget: Option<usize>,
) -> anyhow::Result<()> {
    debug!(file = %args.file, budget = ?args.budget, "executing tokens command");

    let content = std::fs::read_to_string(args.file.as_std_path())
        .with_context(|| format!("failed to read {}", args.file))?;

    let budget = args.budget.or(config_budget);
    let report = tokens::count_tokens(&content, budget)
        .with_context(|| format!("failed to count tokens in {}", args.file))?;

    if global_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.over_budget {
        let max = report.budget.unwrap_or(0);
        bail!(
            "{} is {} tokens (budget: {}). Compress.",
            args.file,
            report.count.red(),
            max,
        );
    } else if let Some(max) = report.budget {
        println!(
            "{} {} is {} tokens (budget: {max})",
            "PASS:".green(),
            args.file,
            report.count,
        );
    } else {
        println!("{}", report.count);
    }

    Ok(())
}
