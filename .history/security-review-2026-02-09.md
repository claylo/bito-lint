# Security Code Review - bito-lint

Date: 2026-02-09
Reviewer: Codex (GPT-5)
Scope: Full repository review (`crates/*`, `xtask/*`, config/security docs)

## Executive Summary

- No critical remote-code-execution or data-exfiltration paths were found.
- 1 medium-risk and 1 low-risk issue were identified, both centered on resource/path hardening.
- Dependency checks passed with no known advisories.

## Methodology

- Static code review of runtime entry points and trust boundaries:
  - CLI commands and file handling
  - MCP server request handling
  - Config loading and env overrides
  - Logging/output path selection
- Dependency and policy audit:
  - `cargo deny check advisories bans licenses sources`

## Findings

### 1) Medium: Unbounded input size enables memory/CPU exhaustion (DoS)

- Severity: Medium
- CWE: CWE-400 (Uncontrolled Resource Consumption)
- Affected code:
  - `crates/bito-lint/src/server.rs:41`
  - `crates/bito-lint/src/server.rs:52`
  - `crates/bito-lint/src/server.rs:64`
  - `crates/bito-lint/src/server.rs:73`
  - `crates/bito-lint/src/server.rs:85`
  - `crates/bito-lint/src/commands/analyze.rs:43`
  - `crates/bito-lint/src/commands/tokens.rs:31`
  - `crates/bito-lint/src/commands/readability.rs:31`
  - `crates/bito-lint/src/commands/grammar.rs:31`
  - `crates/bito-lint/src/commands/completeness.rs:31`
  - `crates/bito-lint-core/src/tokens.rs:38`

#### Details

- MCP tools accept unbounded `String` payloads for `text` and pass them directly into analysis/tokenization paths.
- CLI commands load entire files into memory with `std::fs::read_to_string` and perform full-text analysis.
- For very large input, this can cause high memory consumption and long CPU-bound execution (especially tokenization and multi-check analysis), resulting in local denial-of-service.

#### Exploitability

- Practical for any untrusted caller of the MCP server (or automation that forwards untrusted content).
- Practical for local/CI usage when very large files are passed intentionally or accidentally.

#### Recommended Remediation

- Enforce a max input size before analysis:
  - MCP: reject payloads over a configured limit (for example, 1-5 MB), returning structured invalid-params errors.
  - CLI: preflight file metadata (`metadata.len()`) and fail early on oversized files.
- Add optional execution guards:
  - Per-request timeout (MCP).
  - Maximum token threshold before deeper analysis.
- Add tests for oversized input handling in both CLI and MCP tool paths.

### 2) Low: Log path overrides allow uncontrolled append target (symlink/path abuse)

- Severity: Low
- CWE: CWE-59 (Improper Link Resolution Before File Access), CWE-73 (External Control of File Name or Path)
- Affected code:
  - `crates/bito-lint/src/observability.rs:323`
  - `crates/bito-lint/src/observability.rs:385`
  - `crates/bito-lint/src/observability.rs:404`
  - `crates/bito-lint/src/observability.rs:409`

#### Details

- `BITO_LINT_LOG_PATH`, `BITO_LINT_LOG_DIR`, and config `log_dir` can select arbitrary write targets.
- The logger opens files with `OpenOptions::create(true).append(true)` and does not prevent symlink traversal or enforce owner/permission checks.
- In elevated or multi-tenant execution contexts, this could be abused for unintended file append/tampering.

#### Exploitability

- Low in normal single-user CLI usage.
- Higher impact if the binary runs with elevated privileges and attacker-controlled environment/config.

#### Recommended Remediation

- Restrict log destinations to an allowlisted app directory by default; require explicit unsafe flag to write elsewhere.
- Add checks before open:
  - Reject symlink paths (`symlink_metadata` + `file_type().is_symlink()`).
  - Require target parent ownership/permissions in sensitive modes.
- Consider `create_new(true)` for first-write setup paths and safer rotation strategy where feasible.

## Dependency Audit Result

Command run:

```bash
cargo deny check advisories bans licenses sources
```

Result:

- `advisories ok, bans ok, licenses ok, sources ok`
- Non-blocking policy warnings exist in `deny.toml` (duplicate/unmatched skip entries), but no known security advisories were reported.

## Notes

- `#![deny(unsafe_code)]` is enforced in runtime crates, which reduces memory safety risk.
- No shell command injection path was found in runtime CLI/MCP code.
