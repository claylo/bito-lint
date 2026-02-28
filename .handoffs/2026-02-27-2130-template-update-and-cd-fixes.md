# Handoff: Template Update v1.0.0-beta.6 + CD Pipeline Fixes

**Date:** 2026-02-27
**Repo:** `~/source/claylo/bito-lint`
**Branch:** `main` (template-update was merged as PR #14)
**State:** Yellow — merge conflicts resolved, CD pipeline fixes staged but not yet committed/tagged

## Current State

### Done
- **Template merge conflicts resolved** (PR #14, commit `188a631`): 12 files with merge conflicts from claylo-rs template update (beta.5 -> beta.6) were resolved. The pattern: accept template API changes (ConfigSources), keep bito-lint-specific commands and features.
- **Key API change**: `ConfigLoader::load()` now returns `(Config, ConfigSources)` instead of just `Config`. All callers updated. `find_project_config()` removed in favor of `ConfigSources::primary_file()`.
- **214 tests passing**, clippy clean, fmt clean after the merge.
- **v0.1.6 tagged and pushed**, but the CD run failed due to YAML syntax errors in cd.yml.
- **crates.io publish succeeded** (v0.1.6 is live on crates.io) because it didn't gate on builds.

### Staged but not committed
These fixes are staged on `main` atop the `v0.1.6` tag:

1. **Homebrew formula extracted to standalone file** (`.github/formula.rb.tmpl`): The inline heredoc in cd.yml broke YAML parsing because heredoc content dropped to 0-indentation inside a `|` block, causing `class BitoLint < Formula` to be parsed as YAML (the `<` is invalid). Replaced with a `.rb.tmpl` file + `sed` substitution.

2. **npm README heredoc replaced with printf**: Same YAML-heredoc problem. Replaced `cat > README.md <<EOF` with `printf '%s\n'` lines.

3. **All publish jobs now gate on build success**: `publish-crates-io`, `publish-deb`, and `publish-rpm` previously only needed `generate-changelog`. Now all need `[generate-changelog, publish-binaries]`.

4. **rustfmt fixes**: Template code came in with lines longer than rustfmt's limit. 4 files reformatted.

## Next Steps

1. **Commit the staged fixes** — the fmt fixes + cd.yml fixes + formula template
2. **Re-tag v0.1.6**: Delete old tag, create new one on the fix commit, push:
   ```bash
   git tag -d v0.1.6
   git push origin :refs/tags/v0.1.6
   git tag v0.1.6
   git push origin v0.1.6
   ```
3. **Verify CD pipeline completes** — all builds, then all publish steps
4. **crates.io is already published** — the publish-crates-io step will skip (already at v0.1.6)
5. **Backport CD fixes to claylo-rs template** — the heredoc-in-YAML and missing build gates are template bugs that affect all repos using claylo-rs

## Key Files

| File | Why |
|------|-----|
| `.github/workflows/cd.yml` | Fixed: heredoc removal, build gates, npm README |
| `.github/formula.rb.tmpl` | New: standalone Homebrew formula template with `__PLACEHOLDER__` markers |
| `crates/bito-lint-core/src/config.rs` | ConfigLoader returns `(Config, ConfigSources)` now |
| `crates/bito-lint/src/commands/doctor.rs` | Takes `config + sources + cwd` (bito-lint needs dialect from config) |
| `crates/bito-lint/src/commands/info.rs` | Takes `config + sources` (template pattern) |
| `crates/bito-lint/src/main.rs` | Updated command dispatch for new signatures |

## Gotchas

- **v0.1.6 is on crates.io but nowhere else.** Binaries, Homebrew, npm, deb, rpm all failed because the CD pipeline broke before reaching them.
- **The tag must be moved, not bumped.** crates.io already has v0.1.6 so you can't publish v0.1.7 for what's essentially a CI fix. Move the tag to include the fix.
- **`just test` fails in Claude's sandbox** because TMPDIR is set to `/tmp/claude/` which doesn't exist. Use `dangerouslyDisableSandbox: true` for test/clippy/fmt commands.
- **The Homebrew `publish-homebrew` job does a sparse checkout** to get `.github/formula.rb.tmpl`. This was added because the job previously had no checkout step (the heredoc was self-contained).
- **Two heredoc patterns broke YAML**: `<<RUBY` (formula) and `<<EOF` (npm README). Any future heredocs in workflow `run: |` blocks will have the same problem if their content has less indentation than the block's first line.

## What Worked / Didn't Work

- **Worked:** `sed` with `__PLACEHOLDER__` markers for the formula template. Clean, readable, YAML-safe.
- **Worked:** `printf '%s\n'` for the short npm README. No temp files, no indentation issues.
- **Worked:** Resolving merge conflicts by accepting template API changes everywhere, layering bito-lint features on top. Clean pattern.
- **Didn't work:** Inline heredocs in GitHub Actions YAML `|` blocks. The YAML parser doesn't know about shell heredocs — it just sees content that dropped below the block's indentation level.
- **Didn't work:** Ruby YAML parser for validation initially — needed `dangerouslyDisableSandbox` due to system Ruby location.

## Commands

```sh
cd ~/source/claylo/bito-lint
git status                    # Should show staged changes
git diff --cached --stat      # Review what's staged
just check                    # Full suite before committing
# Then commit, re-tag, push (see Next Steps above)
```
