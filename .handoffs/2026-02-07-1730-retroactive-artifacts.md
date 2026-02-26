# Handoff: Retroactive Development Artifacts for bito-lint

**Date:** 2026-02-07
**Repo:** `~/source/claylo/bito-lint`
**State:** Green — 173 tests passing, `just check` clean

## Current State

bito-lint is a fully functional writing analysis CLI + MCP server. All 5 implementation phases are complete and committed (`92a095d`). The old `crates/bito-lint/` was stripped from the building-in-the-open repo (`f3b4572`). Test coverage was improved with 35 new unit tests across 7 analysis modules.

The problem: bito-lint was built *without* using the building-in-the-open plugin's own artifact workflows. The repo has zero handoffs, zero ADRs, zero design docs, an empty CHANGELOG, and a README that still describes template scaffolding rather than the actual tool. We need to retroactively create these artifacts.

## Next Steps — Task List

### 1. Copy the extraction handoff

Copy `~/source/claylo/building-in-the-open/.handoffs/2026-02-07-134655-bito-lint-extraction.md` into `~/source/claylo/bito-lint/.handoffs/`. This is the project's origin story — why bito-lint was extracted to its own repo.

### 2. Adapt the plan into a design doc

The 5-phase implementation plan at `~/.claude/plans/crispy-beaming-toast.md` is essentially a design doc. Adapt it into `docs/designs/2026-02-07-writing-analysis-engine.md` in the bito-lint repo. Clean up plan-specific language ("Phase 1", "Phase 2" execution details) into a proper design doc format: Overview, Architecture, Key Decisions, CLI Surface, MCP Tools, What We Kept/Replaced/Added, and Cross-cutting Concerns. Mark status as **Implemented**. Use the Technical Writer persona voice from `~/source/claylo/building-in-the-open/personas/technical-writer.md`.

### 3. Extract ADRs from key decisions

The plan documents 8 key decisions. Extract each into a MADR 4.0.0 ADR in `docs/decisions/`. Template at `~/source/claylo/building-in-the-open/templates/adr.md`. Decisions to capture:

1. **Pure functions over god-object** — no TextAnalyzer monolith, callers compose
2. **Drop position tracking from reports** — semantic data only (sentence numbers, counts, percentages)
3. **LazyLock over lazy_static** — std-only, no external dependency
4. **pulldown-cmark for markdown** — proper CommonMark AST vs regex stripping
5. **Every capability as both CLI and MCP tool** — added in same phase
6. **MCP server as optional feature flag** — `default = ["mcp"]`, CLI works without async deps
7. **Result types over process::exit** — main() converts errors to exit codes
8. **Two-crate workspace** — bito-lint-core (library, pure) + bito-lint (binary, presentation)

Create a `docs/decisions/README.md` index linking all ADRs.

### 4. Write a current-state handoff

Write a handoff for the current state of the project. Include:
- All 5 phases complete, 173 tests, MCP feature-flagged
- CLI surface: tokens, readability, completeness, grammar, analyze, doctor, info, serve
- Remaining work: dialect support, MCP context budget audit, real-world calibration
- Landmines: README doesn't describe actual tool yet (task 6 fixes this), manpages need regeneration after README update

### 5. Update CHANGELOG

Add a v0.1.0 entry to `CHANGELOG.md` covering everything built. Group by: Added (commands, MCP tools, analysis features, dictionaries, config). Use conventional changelog format already scaffolded in the file.

### 6. Update README

The current README describes template scaffolding (generic install instructions, placeholder features). Rewrite to describe the actual tool: what bito-lint does, the 5 command types, MCP server mode, analysis features, configuration, development setup. Use the Doc Writer persona voice from `~/source/claylo/building-in-the-open/personas/doc-writer.md` for the usage sections and Marketing Copywriter persona from `~/source/claylo/building-in-the-open/personas/marketing-copywriter.md` for the intro.

### 7. Commit all artifacts

Stage and commit with a descriptive message. Do NOT push.

## Key Files

| File | Why |
|------|-----|
| `~/.claude/plans/crispy-beaming-toast.md` | The 5-phase plan — source material for design doc and ADRs |
| `~/source/claylo/building-in-the-open/.handoffs/2026-02-07-134655-bito-lint-extraction.md` | Origin handoff to copy |
| `~/source/claylo/building-in-the-open/personas/technical-writer.md` | Voice for ADRs and design doc |
| `~/source/claylo/building-in-the-open/personas/doc-writer.md` | Voice for README usage sections |
| `~/source/claylo/building-in-the-open/personas/marketing-copywriter.md` | Voice for README intro |
| `~/source/claylo/building-in-the-open/templates/adr.md` | MADR 4.0.0 template for ADRs |
| `~/source/claylo/bito-lint/crates/bito-lint/src/lib.rs` | Commands enum — CLI surface reference |
| `~/source/claylo/bito-lint/crates/bito-lint/src/server.rs` | MCP tools — tool surface reference |
| `~/source/claylo/bito-lint/crates/bito-lint-core/src/lib.rs` | Core module structure |
| `~/source/claylo/bito-lint/CHANGELOG.md` | Currently empty, needs v0.1.0 entry |
| `~/source/claylo/bito-lint/README.md` | Currently template scaffolding, needs full rewrite |

## Gotchas

- **Persona files are in the other repo.** Read them before writing — they have calibration examples showing right/wrong voice.
- **ADR numbering starts at 0001.** The building-in-the-open repo already has 0001-0006; bito-lint gets its own numbering starting at 0001.
- **MADR 4.0.0 format** — not Nygard format. See the template.
- **Don't overwrite this handoff.** It's a point-in-time snapshot. Write new ones with new timestamps.
- **The plan file has stale "Phase N" framing.** The design doc should describe the architecture as-built, not the build sequence.
- **jq not python** for any JSON pretty-printing. Clay was very clear about this.

## What Worked / Didn't Work

- **Worked:** Pure functions in core, thin CLI/MCP wrappers. Easy to test (173 tests), easy to feature-flag MCP.
- **Worked:** Edition 2024 let-chains for collapsible-if patterns. Cleaner than nested if-let.
- **Worked:** `just test` (nextest) — 10x faster than cargo test. Always use it.
- **Didn't work:** Using `python3 -m json.tool` for JSON output. Use `jq`. The permission prompt for python blocked for hours.
- **Didn't work:** Writing test sentences that were "just barely" over a threshold. Be generous — write sentences clearly over/under the target values.

## Commands

```sh
cd ~/source/claylo/bito-lint
just check          # Full suite: fmt + clippy + deny + test + doc-test
just test           # Fast tests only (nextest)
just cov            # Coverage report → target/llvm-cov/
git log --oneline   # Two commits: scaffolding + full implementation
```
