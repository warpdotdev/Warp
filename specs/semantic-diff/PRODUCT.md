# Semantic Diff Review

## Summary

Add entity-level (semantic) diff understanding to the Code Review panel, so diffs are presented in terms of code entities (functions, classes, structs, methods) rather than raw lines.

## Problem

Today, Warp's code review panel shows diffs identically to `git diff`: line-level hunks with ±4 lines of context. This doesn't understand code structure:

- A one-line change inside a 30-line function shows only the changed line + 4 neighbors — you can't see the whole function without manually expanding.
- A renamed function shows as "5 deletions, 5 additions" rather than "renamed."
- Formatting-only changes (cargo fmt, prettier, gofmt) are indistinguishable from logic changes.
- Reviewers must mentally reconstruct which entity they're looking at on every hunk.

This is a quality-of-life issue, not a blocker — but it makes code review meaningfully slower and more fatiguing on large changesets.

## User-Facing Behavior

### Phase 1: SemanticDiff — Entity sidebar + classification (this PR)

**Entity extraction & matching:** When a file is opened in the code review panel, Warp parses both the base and current content using its existing tree-sitter infrastructure. It extracts named entities (functions, classes, structs, methods, etc.) and matches them across the two versions using a three-phase algorithm: exact name → structural hash → fuzzy similarity.

**Entity-level change classification:** Each matched pair is classified as one of:
- `Unchanged` — same name, same body (formatting-only differences are collapsed)
- `Modified` — same name, different body
- `Renamed` — different name, same body structure
- `Added` — entity exists only in current
- `Deleted` — entity exists only in base
- `Moved` — same entity, different position within the file

**Entity sidebar in file header:** The file diff header shows a compact summary of entity-level changes, e.g.:
```
fn validateToken (modified) · fn processOrder → processRequest (renamed) · struct Config (added)
```

**Entity-aware stats:** File-level `+N -M` stats are augmented with entity counts:
```
+12 -5 (3 modified, 1 renamed, 1 added)
```

**Formatting-aware collapse:** Entities classified as `Unchanged` (same body ignoring whitespace) are automatically collapsed in the diff view. The reviewer can expand them if needed.

### Phase 2: SemanticDiffContext — Entity-scoped context (future)

Instead of ±4 lines around each hunk, the entire entity that contains a change is shown. Unchanged entities between changes stay collapsed. This replaces the fixed-context model with a code-structure-aware one.

### Phase 3: SemanticDiffRenames — Rename/move awareness (future)

Renamed and moved entities are visually indicated in the diff gutter with arrows or annotations. The diff view shows the entity once with rename markers rather than as separate addition + deletion.

### Phase 4: SemanticDiffAIContext — AI integration (future)

When attaching diffs to the AI agent, entire changed entities (with names and types) are sent instead of raw line hunks.

## Feature Flags

| Flag | Purpose | Default |
|---|---|---|
| `SemanticDiff` | Gates all entity-level diff features | Disabled (dogfood) |

Sub-features will be added as separate flags when the phases above are implemented.

## Edge Cases

- **Unsupported languages:** Files without `identifiers.scm` queries fall back to standard line-based diff. No error, no UI change.
- **Parse failures:** If tree-sitter fails to parse either version, fall back to line-based diff silently.
- **Binary/non-text files:** No entity extraction attempted. Standard diff behavior.
- **Very large files:** Entity extraction is capped at the same file size limits already used for file outlines.
- **Multiple entities with same name:** All same-named entities are matched by structural hash after the exact name phase.
- **Nested entities** (methods inside classes): Both the outer and inner entity are reported. The inner entity inherits the outer entity's classification context.

## Success Criteria

1. For any of the 18 languages with `identifiers.scm` queries, opening a diff in code review shows entity-level classifications in the file header.
2. Renamed functions are classified as "renamed" rather than "addition + deletion."
3. Formatting-only changes within an entity are classified as "unchanged" and collapsed.
4. Languages without `identifiers.scm` queries show no change from current behavior.
5. No measurable regression in code review panel load time for small-to-medium files (<10K lines).
6. WASM builds compile and run correctly with the feature flag enabled.

## Out of Scope

- Cross-file rename/move tracking (requires petgraph, future work)
- Semantic merge conflict resolution
- Side-by-side diff view (separate feature)
- Replacing the line-based diff algorithm itself (entity classification is additive)
