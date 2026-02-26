Date: 2026-02-09
Reviewer: Codex (gpt-5.3-codex high)

**Findings**

1. **High** — Unknown `checks` values are silently ignored, which can accidentally (or intentionally) disable quality gates.
   `crates/bito-lint-core/src/analysis/mod.rs:89` builds a `HashSet` from user-provided names but never validates against
   `ALL_CHECKS`, and the CLI/MCP both pass user input straight through (`crates/bito-lint/src/commands/analyze.rs:52`,
   `crates/bito-lint/src/server.rs:290`). A typo like `--checks readablity` returns success with an empty report (`{}`), so
   CI can pass while running zero intended checks.
   Recommended fix: validate requested names and return an error listing unknown values and valid options.
2. **Medium-High** — npm `postinstall` fallback downloads binaries outside npm’s integrity model and without basic network
   safety limits.
   The package always runs a postinstall script (`npm/bito-lint/package.json:14`) that manually fetches a tarball (`npm/
   bito-lint/install.js:35`, `npm/bito-lint/install.js:37`) and extracts it with a custom parser (`npm/bito-lint/
   install.js:69`). The downloader follows redirects recursively and has no timeout, size cap, or status-code
   validation (`npm/bito-lint/install.js:55`), which creates supply-chain and DoS risk (especially in CI/proxy
   environments).
   Recommended fix: avoid manual download when possible; otherwise add integrity verification (hash), redirect limit,
   timeout, content-length cap, and strict status handling.
3. **Medium** — npm CLI wrapper can report success when the Rust binary was terminated by a signal.
   `npm/bito-lint/cli.js:9` exits with `code ?? 0` on close, but ignores the `signal` parameter. If the child is killed
   (e.g., `SIGINT`, `SIGTERM`, OOM kill), `code` is `null`, so the wrapper exits `0`, which can mask failures in scripts/CI.
   Recommended fix: handle `(code, signal)` and exit non-zero on signal (or re-emit the signal).
4. **Medium** — `max_input_bytes = null` cannot disable the limit as documented.
   The config field is `Option<usize>` with docs saying null/omit disables (`crates/bito-lint-core/src/config.rs:110`),
   but `main` always replaces `None` with the default limit (`crates/bito-lint/src/main.rs:69`). `null` and omitted both
   deserialize to `None`, so disabling is impossible.
   Recommended fix: use a tri-state config representation (e.g., `Option<Option<usize>>` via custom deserialization or
   an enum).
5. **Low-Medium** — `info`/`doctor` can report the wrong config file path (and search past repo boundaries).
   `ConfigLoader::new()` defaults to a `.git` boundary (`crates/bito-lint-core/src/config.rs:172`), but the public helper
   used by diagnostics explicitly disables that boundary (`crates/bito-lint-core/src/config.rs:342`, `crates/bito-lint-
   core/src/config.rs:345`). `info` and `doctor` both use this helper (`crates/bito-lint/src/commands/info.rs:69`, `crates/
   bito-lint/src/commands/doctor.rs:75`), so they can claim a config was found that the actual loader would not use.
   Recommended fix: track/report the actual loaded source path from `ConfigLoader` instead of re-discovering separately.

**Open Questions / Assumptions**

- I assumed `--checks` should fail closed on unknown names (safer for CI/policy gates) rather than silently ignore them.
- I treated the npm fallback installer as production-relevant; if it’s only for local dev packaging experiments,
  severity is lower but the risks remain.

What Changed

- No code changes (review only).
- Ran the project checks and validated one finding with a live CLI repro (--checks readablity returned {} with exit
  code 0).

How To Run Relevant Checks

- just check
- Repro for finding #1: `cargo run -q -p bito-lint -- analyze README.md --checks readablity --json`

Follow-up Notes

1. Add regression tests for invalid --checks in CLI and MCP paths.
2. Add npm wrapper/install tests (signal propagation + download failure/redirect behavior).
