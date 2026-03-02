# Path-Based Rules and Inline Suppressions

## Problem

bito-lint's quality gates are configured globally (config file) or per-invocation (CLI flags). Projects need different checks for different file types: ADRs need completeness templates, design docs need readability ceilings, handoffs need token budgets. Today this requires a shell hook script with case/esac pattern matching, duplicating logic that the tool itself should own.

The MCP server can't benefit from these path-based rules at all --- it accepts all settings as explicit parameters per call.

## Design Decisions

- **Rule syntax:** ordered array of rule objects in config (YAML/TOML/JSON)
- **Overlap:** all matching rules accumulate; when two rules configure the same check, the more specific rule wins (specificity = literal path segment count; ties broken by rule order, earlier wins)
- **Inline suppression:** HTML comments (`<!-- bito-lint disable style -->`) parsed before markdown stripping
- **New CLI subcommand:** `bito-lint lint <file>` --- matches rules, runs configured checks
- **New MCP tool:** `lint_file` --- same behavior, accepts file path + text

## Config Schema

```yaml
rules:
  - paths: [".handoffs/*.md"]
    checks:
      tokens:
        budget: 2000
      completeness:
        template: handoff

  - paths: ["docs/decisions/*.md"]
    checks:
      completeness:
        template: adr

  - paths: ["docs/designs/*.md"]
    checks:
      completeness:
        template: design-doc
      readability:
        max_grade: 12

  - paths: ["README.md", "docs/**/*.md"]
    checks:
      analyze:
        max_grade: 8
```

### Rule Accumulation Example

Given the rules above, `docs/decisions/001.md` matches both `docs/decisions/*.md` and `docs/**/*.md`. It gets:

- `completeness` (template: adr) --- from the specific rule
- `analyze` (max_grade: 8) --- from the general rule

No duplication needed.

### Check Config Structs

Each check type carries only its relevant settings. Omitted fields inherit project-wide config defaults.

| Check | Fields |
|---|---|
| `analyze` | `checks`, `exclude`, `max_grade`, `passive_max`, `style_min`, `dialect` |
| `readability` | `max_grade` |
| `grammar` | `passive_max` |
| `completeness` | `template` (required) |
| `tokens` | `budget`, `tokenizer` |

## Inline Suppressions

HTML comment directives, parsed before markdown stripping:

```markdown
<!-- bito-lint disable style -->
This paragraph's style issues are suppressed.
<!-- bito-lint enable style -->

<!-- bito-lint disable-next-line readability -->
This one complex sentence won't flag readability.

<!-- bito-lint disable grammar,cliches -->
Multiple checks suppressed at once.
<!-- bito-lint enable grammar,cliches -->
```

### Architecture

1. New `directives` module parses HTML comments from raw input
2. Produces a `SuppressionMap` --- maps check names to suppressed line ranges
3. Suppression map passed into analysis functions
4. Line-level findings (grammar issues, cliches) filtered against the map
5. Document-level scores (style, readability) skip computation when fully suppressed

## New CLI Subcommand: `lint`

```
bito-lint lint <file>         # match rules, run configured checks
bito-lint lint <file> --json  # JSON output
```

- Matches file path against `rules` config
- No matching rule = clean exit (nothing to check)
- Runs all resolved checks, returns combined output
- Honors inline suppressions
- Exit code: 0 = all pass, 1 = any fail thresholds

## New MCP Tool: `lint_file`

```json
{
  "name": "lint_file",
  "description": "Lint a file according to project rules.",
  "parameters": {
    "file_path": "string (required)",
    "text": "string (required)"
  }
}
```

The MCP server receives the full `Config` at init (currently only `max_input_bytes`). The `lint_file` tool uses the config's `rules` and project-wide defaults to resolve checks.

## Rule Resolution Engine

New `rules` module in `bito-lint-core`:

```
RuleSet
  rules: Vec<Rule>
  fn resolve(&self, file_path: &str) -> ResolvedChecks

ResolvedChecks
  analyze: Option<AnalyzeCheckConfig>
  readability: Option<ReadabilityCheckConfig>
  grammar: Option<GrammarCheckConfig>
  completeness: Option<CompletenessCheckConfig>
  tokens: Option<TokensCheckConfig>
```

**Specificity heuristic:** count literal (non-wildcard) path segments in the matching pattern. More literal segments = more specific = wins conflicts. Example: `docs/decisions/*.md` (2 literal) beats `docs/**/*.md` (1 literal).

**Glob matching:** `glob` crate for pattern matching. Paths in config are relative to project root.

## Interaction with Hooks

The hook script and the MCP tool serve different enforcement boundaries:

- **Hook:** automatic guardrail, fires on file writes whether the AI remembers or not
- **MCP tool:** lets the AI be proactive about checking before it writes

Both read the same `rules` config. After this feature ships, the hook simplifies to:

```bash
bito-lint lint "$FILE_PATH"
```

## Implementation Sequence

1. Config structs (`Rule`, `RuleChecks`, check configs) + serde + tests
2. Rule resolution engine (`RuleSet`, specificity, accumulation) + tests
3. Inline suppression parser (`directives` module) + tests
4. Wire suppressions into analysis pipeline
5. `lint` CLI subcommand
6. `lint_file` MCP tool (expand `ProjectServer` to hold `Config`)
7. Integration tests + doc updates
