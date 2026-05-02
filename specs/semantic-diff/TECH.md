# Semantic Diff Review — Technical Design

## Architecture Overview

The feature adds a new module `crates/languages/src/semantic_diff.rs` that computes entity-level changes between two versions of a file. This result flows into the existing code review pipeline alongside (not replacing) the line-based diff.

```
┌─────────────────────────────────────────────────────────┐
│ FileDiffAndContent                                      │
│  .file_diff (existing line-based diff)                  │
│  .content_at_head (existing base content string)        │
│  .entity_diff: Option<EntityDiff>  ← NEW               │
│     .entities: Vec<EntityChange>                        │
└─────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────┐
│ code_review_view.rs                                     │
│  - File header: entity summary + stats                  │
│  - Future: entity sidebar, entity-scoped context        │
└─────────────────────────────────────────────────────────┘
```

## New Types

### `Entity` — A named code entity extracted from one version of a file

```rust
/// A named code entity extracted from a file via tree-sitter.
#[derive(Debug, Clone)]
pub struct Entity {
    /// The entity name (e.g. "validateToken", "Config")
    pub name: String,
    /// The entity type prefix (e.g. "fn", "struct", "class", "def")
    pub type_prefix: Option<String>,
    /// 0-indexed start line (inclusive)
    pub start_line: usize,
    /// 0-indexed end line (exclusive)
    pub end_line: usize,
    /// Hash of the entity body (content between start_line and end_line,
    /// with leading whitespace normalized) for structural comparison.
    pub body_hash: u64,
}
```

### `EntityChange` — A matched entity pair across two file versions

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityChangeKind {
    Unchanged,
    Modified,
    Renamed { old_name: String },
    Added,
    Deleted,
    Moved, // same entity, different position
}

#[derive(Debug, Clone)]
pub struct EntityChange {
    pub kind: EntityChangeKind,
    /// The entity in the current version (None for Deleted)
    pub current: Option<Entity>,
    /// The entity in the base version (None for Added)
    pub base: Option<Entity>,
}
```

### `EntityDiff` — The complete entity-level diff for one file

```rust
#[derive(Debug, Clone, Default)]
pub struct EntityDiff {
    pub changes: Vec<EntityChange>,
}
```

## Entity Extraction

### Approach: Extend existing `identifiers.scm` queries

The existing `identifiers.scm` queries capture entity **names** via `@definition.*` captures. We need the **full span** (start line + end line) of each entity. Rather than creating separate query files, we extend the existing queries to also capture the entity's full node.

The existing `get_symbols()` function in `crates/ai/src/index/file_outline/native.rs` already iterates query captures. We add a parallel `extract_entities()` function in `crates/languages/src/semantic_diff.rs` that:

1. Gets the `Language` for the file via `language_by_filename()`
2. Parses the content with `Parser::new()` + `language.grammar`
3. Runs `language.symbols_query` against the tree
4. For each `@definition.*` capture, extracts:
   - `name`: the captured text (existing behavior)
   - `type_prefix`: from the capture name (existing behavior, e.g. `"fn"` from `"definition.fn"`)
   - `start_line` / `end_line`: from `cap.node.range()` (the name node) — **BUT** we need the _parent_ node's range for the full entity span
5. Walks up from the captured name node to its parent to get the full entity body range
6. Computes `body_hash` from the parent node's text

### Full-span extraction strategy

The `@definition.fn` capture targets the _name_ identifier inside a function definition. To get the full entity span, we walk up to the parent node (e.g., `function_item` in Rust, `function_definition` in Python). This works because tree-sitter's AST always has the definition node as the parent of the name identifier.

For languages without `identifiers.scm` queries, `extract_entities()` returns an empty vec and the caller falls back to line-based diff.

### Parent-node heuristic

```rust
fn entity_parent_node(node: Node) -> Option<Node> {
    let parent = node.parent()?;
    // Skip "wrapper" nodes like (identifier) inside (function_item)
    // We want the definition-level node, which is the direct parent
    // of the @definition.* capture.
    Some(parent)
}
```

This is a simple `node.parent()` call because the `@definition.*` captures are always on the _name_ child, and the parent is the definition node.

## Entity Matching Algorithm

Three-phase matching, mirroring Sem's approach:

### Phase 1: Exact name match

For each entity in `base` and `current` with the same name + type_prefix, pair them. If multiple entities share a name (overloaded methods), pair by closest line proximity.

### Phase 2: Structural hash match (renames)

For remaining unmatched entities, group by `body_hash`. Entities with identical body hashes but different names are classified as `Renamed`. If multiple candidates match, prefer the one closest in line position.

### Phase 3: Fuzzy similarity

For remaining unmatched entities, compute token-overlap similarity using a cheap set-intersection approach (split body text into whitespace-delimited tokens, compute Jaccard index). Pair entities with >80% overlap. These are classified as `Modified` (with a low-confidence flag for future UI hints).

### Remaining unmatched

- Entities only in `current` → `Added`
- Entities only in `base` → `Deleted`

### Moved detection

If an exact-name-matched entity has the same body hash but its line position differs significantly (offset > 3 lines from expected position based on original ordering), classify as `Moved` instead of `Unchanged`.

## Body Hash Computation

```rust
fn compute_body_hash(content: &str, start_line: usize, end_line: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    
    let body: String = content.lines()
        .skip(start_line)
        .take(end_line - start_line)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    hasher.finish()
}
```

Trimming + empty-line removal ensures formatting-only changes produce the same hash. This is intentional — we want `Unchanged` classification for `cargo fmt` / `prettier` runs.

## Formatting-Only Detection

An entity is classified as `Unchanged` when:
- Exact name match (Phase 1) succeeded
- `body_hash` of current entity == `body_hash` of base entity (whitespace-normalized)

This means formatting-only changes within an entity produce `Unchanged`. The raw line-based diff still shows the exact line changes; the entity classification is additive metadata.

## Integration Points

### 1. `crates/languages/src/semantic_diff.rs` (NEW, ~600 lines)

Public API:
```rust
/// Extract entities from file content using tree-sitter.
pub fn extract_entities(content: &str, path: &Path) -> Option<Vec<Entity>>

/// Match entities across two file versions and classify changes.
pub fn diff_entities(base_entities: &[Entity], current_entities: &[Entity]) -> EntityDiff

/// Convenience: extract + diff in one call.
pub fn compute_entity_diff(base_content: &str, current_content: &str, path: &Path) -> Option<EntityDiff>
```

### 2. `app/src/code_review/diff_state.rs`

Add `entity_diff: Option<EntityDiff>` to `FileDiffAndContent`:
```rust
pub struct FileDiffAndContent {
    pub file_diff: FileDiff,
    pub content_at_head: Option<String>,
    pub entity_diff: Option<EntityDiff>,  // NEW
}
```

Compute it alongside the existing diff parsing when `FeatureFlag::SemanticDiff` is enabled:
```rust
let entity_diff = if FeatureFlag::SemanticDiff.is_enabled() {
    content_at_head.as_ref().and_then(|base| {
        languages::semantic_diff::compute_entity_diff(base, &current_content, &path)
    })
} else {
    None
};
```

### 3. `app/src/code_review/code_review_view.rs`

In the file header section, render entity summary when `entity_diff` is available:
- Show entity change pills: `fn foo (modified)`, `bar → baz (renamed)`, `struct X (added)`
- Augment stats line with entity counts

### 4. `crates/languages/grammars/*/identifiers.scm`

No changes needed for the 18 languages that already have queries. The entity extraction uses the existing `@definition.*` captures and walks up to the parent node for full spans.

For the 14 missing languages (ruby, php, swift, kotlin, dockerfile, hcl, html, json, lua, powershell, sql, toml, vue, xml, yaml), entity extraction returns `None` and the diff falls back to line-based. Adding queries for these languages is a separate follow-up.

## Dependencies

| Dep | Status | Purpose |
|---|---|---|
| `arborium` (tree-sitter) | Already in workspace | Parsing + query execution |
| `strsim` | Already in workspace | Fuzzy similarity in Phase 3 matching |
| `std::hash::DefaultHasher` | std | Body hash computation |
| `languages` crate | Existing | Language detection, grammar access |

**No new crate dependencies required.**

## Feature Flag

Add `SemanticDiff` to `FeatureFlag` enum in `crates/warp_features/src/lib.rs`. Add to `DOGFOOD_FLAGS` for dev testing.

No `Cargo.toml` feature needed — all code is runtime-gated behind `FeatureFlag::SemanticDiff.is_enabled()`.

## Sequencing

1. **Step 1:** Add `SemanticDiff` feature flag
2. **Step 2:** Implement `semantic_diff.rs` module with `extract_entities()`, `diff_entities()`, `compute_entity_diff()`
3. **Step 3:** Wire into `FileDiffAndContent` in `diff_state.rs`
4. **Step 4:** Render entity summary in `code_review_view.rs` file header
5. **Step 5:** Unit tests for entity matching (rename, modify, add, delete, move, formatting-only)
6. **Step 6:** Verify WASM compilation with feature flag disabled

## Risks

| Risk | Mitigation |
|---|---|
| Tree-sitter parse time on large files | Cap extraction at existing file outline size limits (is_file_parsable). Fall back silently on timeout. |
| Body hash collisions (different bodies, same hash) | DefaultHasher has good distribution. Collisions are non-critical — they'd cause a false "Renamed" classification, which is visually obvious. |
| Parent-node heuristic fails for some language grammars | Fall back to using the name node's line range. This gives a partial entity span rather than a wrong one. |
| Memory overhead from parsing both versions | Tree-sitter trees are dropped immediately after entity extraction. Only `Vec<Entity>` is retained. |
