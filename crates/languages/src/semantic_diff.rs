//! Entity-level (semantic) diff for code review.
//!
//! This module extracts named code entities (functions, classes, structs, etc.) from two
//! versions of a file using tree-sitter and classifies the changes between them.
//!
//! The matching algorithm runs in three phases:
//!   1. Exact name match — entities with the same name and type are paired.
//!   2. Structural hash match — entities with the same whitespace-normalized body hash
//!      but different names are classified as renames.
//!   3. Fuzzy similarity — remaining entities with >80% token overlap are paired.
//!
//! Unmatched base entities are "deleted"; unmatched current entities are "added".

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use arborium::tree_sitter::{Node, Parser, QueryCursor, StreamingIterator, TextProvider, Tree};

/// Minimal text provider for tree-sitter query execution.
/// Replicated here to avoid a cyclic dependency on `syntax_tree`.
struct TextSlice<'a>(&'a [u8]);

impl<'a> TextProvider<TextSlice<'a>> for TextSlice<'a> {
    type I = std::iter::Once<TextSlice<'a>>;

    fn text(&mut self, node: Node) -> Self::I {
        let range = node.byte_range();
        std::iter::once(TextSlice(self.0.get(range).unwrap_or_default()))
    }
}

impl AsRef<[u8]> for TextSlice<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

// ── Public types ───────────────────────────────────────────────────────────

/// A named code entity extracted from a file via tree-sitter.
#[derive(Debug, Clone)]
pub struct Entity {
    /// The entity name (e.g. "validateToken", "Config").
    pub name: String,
    /// The entity type prefix (e.g. "fn", "struct", "class", "def").
    pub type_prefix: Option<String>,
    /// 0-indexed start line (inclusive).
    pub start_line: usize,
    /// 0-indexed end line (exclusive).
    pub end_line: usize,
    /// Hash of the entity body (whitespace-normalized) for structural comparison.
    pub body_hash: u64,
}

/// How an entity changed between base and current versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityChangeKind {
    /// Same name, same body (whitespace-normalized). Formatting-only.
    Unchanged,
    /// Same name, different body.
    Modified,
    /// Different name, same body structure.
    Renamed { old_name: String },
    /// Entity only exists in the current version.
    Added,
    /// Entity only exists in the base version.
    Deleted,
    /// Same entity, different position within the file.
    Moved,
}

/// A matched entity pair across two file versions.
#[derive(Debug, Clone)]
pub struct EntityChange {
    pub kind: EntityChangeKind,
    /// The entity in the current version (`None` for `Deleted`).
    pub current: Option<Entity>,
    /// The entity in the base version (`None` for `Added`).
    pub base: Option<Entity>,
}

/// The complete entity-level diff for one file.
#[derive(Debug, Clone, Default)]
pub struct EntityDiff {
    pub changes: Vec<EntityChange>,
}

impl EntityDiff {
    /// Returns only the non-unchanged changes.
    pub fn significant_changes(&self) -> Vec<&EntityChange> {
        self.changes
            .iter()
            .filter(|c| c.kind != EntityChangeKind::Unchanged)
            .collect()
    }
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Extract named entities from file content using tree-sitter.
///
/// Returns `None` if the language is unsupported or parsing fails.
pub fn extract_entities(content: &str, path: &Path) -> Option<Vec<Entity>> {
    let language = crate::language_by_filename(path)?;
    let symbols_query = language.symbols_query.as_ref()?;

    let mut parser = Parser::new();
    parser.set_language(&language.grammar).ok()?;
    let tree = parser.parse(content, None)?;

    Some(extract_entities_from_tree(&tree, content, symbols_query))
}

/// Match entities across two file versions and classify changes.
pub fn diff_entities(base_entities: &[Entity], current_entities: &[Entity]) -> EntityDiff {
    let changes = match_entities(base_entities, current_entities);
    EntityDiff { changes }
}

/// Convenience: extract + diff in one call.
///
/// Returns `None` if entity extraction fails for either version (unsupported language, parse error).
pub fn compute_entity_diff(
    base_content: &str,
    current_content: &str,
    path: &Path,
) -> Option<EntityDiff> {
    let base_entities = extract_entities(base_content, path)?;
    let current_entities = extract_entities(current_content, path)?;
    // If neither version has entities, don't produce a diff.
    if base_entities.is_empty() && current_entities.is_empty() {
        return None;
    }
    Some(diff_entities(&base_entities, &current_entities))
}

// ── Entity extraction ──────────────────────────────────────────────────────

fn extract_entities_from_tree(
    tree: &Tree,
    content: &str,
    query: &arborium::tree_sitter::Query,
) -> Vec<Entity> {
    let mut cursor = QueryCursor::new();
    let capture_names = query.capture_names();
    let mut captures = cursor.captures(query, tree.root_node(), TextSlice(content.as_bytes()));

    let mut entities = Vec::new();

    while let Some(matches) = captures.next() {
        for cap in matches.0.captures {
            let capture_name = capture_names.get(cap.index as usize);
            let Some(name) = capture_name else { continue };

            // Skip non-definition captures (comments, references, etc.).
            if *name == "comment" || *name == "ignore" || *name == "reference" {
                continue;
            }

            // Only process @definition and @definition.* captures.
            // Bare @definition is used by 18+ languages (C, C++, C#, Java, JS, etc.)
            // for functions/methods. Dotted forms like @definition.class provide
            // a type prefix for the entity pill.
            if !name.starts_with("definition") {
                continue;
            }
            // Must be exactly "definition" or "definition.<suffix>" (dot-separated).
            if *name != "definition" && !name.starts_with("definition.") {
                continue;
            }
            let type_prefix = name.strip_prefix("definition.").map(String::from);

            // The captured node is typically the _name_ identifier.
            // Walk up to the parent to get the full entity span.
            let entity_node = entity_body_node(cap.node);
            let start_line = entity_node.start_position().row;
            let end_line = entity_node.end_position().row; // 0-indexed, inclusive from tree-sitter
            let end_line_exclusive = end_line + 1; // make exclusive

            let name_byte_start = cap.node.start_byte();
            let name_byte_end = cap.node.end_byte();
            let name_text = &content[cap.node.byte_range()];
            let body_hash = compute_body_hash(
                content,
                start_line,
                end_line_exclusive,
                name_byte_start,
                name_byte_end,
            );

            entities.push(Entity {
                name: name_text.to_string(),
                type_prefix,
                start_line,
                end_line: end_line_exclusive,
                body_hash,
            });
        }
    }

    // Sort by start line for deterministic matching.
    entities.sort_by_key(|e| e.start_line);
    entities
}

/// Walk up from the captured name node to get the full entity body node.
///
/// The `@definition.*` captures target name identifiers (e.g., `(identifier)` inside
/// `(function_item)`). We need the parent node for the full span and body hash.
fn entity_body_node(name_node: Node) -> Node {
    // Try the parent first — it's usually the definition node.
    if let Some(parent) = name_node.parent() {
        // If the parent is just a wrapper (e.g., `name:` field in a `struct_item`),
        // we want the grandparent. But typically the direct parent IS the definition node.
        // Check if the parent's byte range is strictly larger than the name's.
        if parent.end_byte() > name_node.end_byte() || parent.start_byte() < name_node.start_byte()
        {
            return parent;
        }
    }
    // Fallback: use the name node itself (partial span is better than wrong span).
    name_node
}

/// Compute a hash of the entity body with whitespace normalization.
///
/// Masks the name token's byte range (replaces with spaces) so that renames
/// produce the same hash. Trims each line, removes empty lines, and joins.
/// This ensures formatting-only changes (cargo fmt, prettier) produce the same
/// hash, and single-line entities (e.g., `fn foo() { 1 }`) are hashed correctly.
fn compute_body_hash(
    content: &str,
    start_line: usize,
    end_line: usize,
    name_byte_start: usize,
    name_byte_end: usize,
) -> u64 {
    let start_byte = content
        .lines()
        .nth(start_line)
        .map(|l| l.as_ptr() as usize - content.as_ptr() as usize)
        .unwrap_or(0);
    let end_byte = content
        .lines()
        .nth(end_line)
        .map(|l| l.as_ptr() as usize - content.as_ptr() as usize)
        .unwrap_or(content.len());

    // Extract entity body text, masking the name token.
    let entity_text = &content[start_byte..end_byte];
    let masked: String = if name_byte_start >= start_byte && name_byte_end <= end_byte {
        let local_start = name_byte_start - start_byte;
        let local_end = name_byte_end - start_byte;
        let mut chars: Vec<char> = entity_text.chars().collect();
        // Count byte offset within the char vec to find the masking region.
        let mut byte_pos = 0usize;
        let mut char_start = None;
        let mut char_end = chars.len();
        for (i, &ch) in chars.iter().enumerate() {
            if byte_pos >= local_start && char_start.is_none() {
                char_start = Some(i);
            }
            if byte_pos >= local_end && char_end == chars.len() {
                char_end = i;
                break;
            }
            byte_pos += ch.len_utf8();
        }
        let char_start = char_start.unwrap_or(0);
        let char_end = char_end.min(chars.len());
        for ch in &mut chars[char_start..char_end] {
            if !ch.is_whitespace() {
                *ch = ' ';
            }
        }
        chars.into_iter().collect()
    } else {
        // Name range outside entity body — hash the full body.
        entity_text.to_string()
    };

    let normalized: String = masked
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    hasher.finish()
}

// ── Entity matching ────────────────────────────────────────────────────────

fn match_entities(base: &[Entity], current: &[Entity]) -> Vec<EntityChange> {
    let mut base_matched = vec![false; base.len()];
    let mut current_matched = vec![false; current.len()];
    let mut changes = Vec::new();

    // ── Phase 1: Exact name match ──────────────────────────────────────────
    for (ci, cur) in current.iter().enumerate() {
        if current_matched[ci] {
            continue;
        }
        // Find the best unmatched base entity with the same name and type.
        let best_bi = find_best_match(
            base,
            &base_matched,
            |b| b.name == cur.name && b.type_prefix == cur.type_prefix,
            cur.start_line,
        );

        if let Some(bi) = best_bi {
            base_matched[bi] = true;
            current_matched[ci] = true;

            let base_ent = &base[bi];
            let kind = if base_ent.body_hash == cur.body_hash {
                // Same body — check if moved (position shifted significantly).
                if position_shifted(base_ent, cur, base, current, bi, ci) {
                    EntityChangeKind::Moved
                } else {
                    EntityChangeKind::Unchanged
                }
            } else {
                EntityChangeKind::Modified
            };

            changes.push(EntityChange {
                kind,
                current: Some(cur.clone()),
                base: Some(base_ent.clone()),
            });
        }
    }

    // ── Phase 2: Structural hash match (renames) ─────────────────────────
    for (ci, cur) in current.iter().enumerate() {
        if current_matched[ci] {
            continue;
        }
        let best_bi = find_best_match(
            base,
            &base_matched,
            |b| b.body_hash == cur.body_hash && b.type_prefix == cur.type_prefix,
            cur.start_line,
        );

        if let Some(bi) = best_bi {
            base_matched[bi] = true;
            current_matched[ci] = true;

            changes.push(EntityChange {
                kind: EntityChangeKind::Renamed {
                    old_name: base[bi].name.clone(),
                },
                current: Some(cur.clone()),
                base: Some(base[bi].clone()),
            });
        }
    }

    // ── Phase 3: Fuzzy similarity (>80% token overlap) ───────────────────
    for (ci, cur) in current.iter().enumerate() {
        if current_matched[ci] {
            continue;
        }
        let best_bi = find_best_match_fuzzy(base, &base_matched, cur);

        if let Some(bi) = best_bi {
            base_matched[bi] = true;
            current_matched[ci] = true;

            changes.push(EntityChange {
                kind: EntityChangeKind::Modified,
                current: Some(cur.clone()),
                base: Some(base[bi].clone()),
            });
        }
    }

    // ── Remaining: Added / Deleted ────────────────────────────────────────
    for (bi, ent) in base.iter().enumerate() {
        if !base_matched[bi] {
            changes.push(EntityChange {
                kind: EntityChangeKind::Deleted,
                current: None,
                base: Some(ent.clone()),
            });
        }
    }
    for (ci, ent) in current.iter().enumerate() {
        if !current_matched[ci] {
            changes.push(EntityChange {
                kind: EntityChangeKind::Added,
                current: Some(ent.clone()),
                base: None,
            });
        }
    }

    // Sort by current line (then base line for deletions) for stable display.
    changes.sort_by(|a, b| {
        let a_line = a
            .current
            .as_ref()
            .map(|e| e.start_line)
            .unwrap_or(usize::MAX);
        let b_line = b
            .current
            .as_ref()
            .map(|e| e.start_line)
            .unwrap_or(usize::MAX);
        a_line.cmp(&b_line)
    });

    changes
}

/// Find the best unmatched entity matching a predicate, preferring closest line position.
fn find_best_match(
    entities: &[Entity],
    matched: &[bool],
    predicate: impl Fn(&Entity) -> bool,
    near_line: usize,
) -> Option<usize> {
    let mut best_idx: Option<usize> = None;
    let mut best_dist = usize::MAX;

    for (i, ent) in entities.iter().enumerate() {
        if matched[i] || !predicate(ent) {
            continue;
        }
        let dist = (ent.start_line as isize - near_line as isize).unsigned_abs();
        if dist < best_dist {
            best_dist = dist;
            best_idx = Some(i);
        }
    }

    best_idx
}

/// Find the best unmatched base entity with >80% token overlap with `cur`.
fn find_best_match_fuzzy(base: &[Entity], base_matched: &[bool], cur: &Entity) -> Option<usize> {
    let cur_tokens = tokenize_entity_name(&cur.name);
    if cur_tokens.is_empty() {
        return None;
    }

    let mut best_idx: Option<usize> = None;
    let mut best_similarity = 0.0f64;

    for (bi, base_ent) in base.iter().enumerate() {
        if base_matched[bi] || base_ent.type_prefix != cur.type_prefix {
            continue;
        }
        let base_tokens = tokenize_entity_name(&base_ent.name);
        if base_tokens.is_empty() {
            continue;
        }

        let similarity = jaccard_index(&cur_tokens, &base_tokens);
        if similarity > 0.8 && similarity > best_similarity {
            best_similarity = similarity;
            best_idx = Some(bi);
        }
    }

    best_idx
}

/// Heuristic: did this entity move significantly within the file?
///
/// Compares the entity's neighbors in the sorted entity list rather than
/// absolute line numbers. Inserting or deleting lines above an entity
/// shifts its absolute line number but preserves its adjacent neighbors,
/// so this avoids false `Moved` classifications from insertions/deletions.
///
/// An entity is classified as moved when its immediate neighbors in the
/// entity ordering have changed (i.e., it swapped position with another
/// existing entity).
fn position_shifted(
    base_ent: &Entity,
    cur_ent: &Entity,
    base: &[Entity],
    current: &[Entity],
    bi: usize,
    ci: usize,
) -> bool {
    // Collect same-type entities into indexed lists.
    let base_same: Vec<(usize, &Entity)> = base
        .iter()
        .enumerate()
        .filter(|(_, e)| e.type_prefix == base_ent.type_prefix)
        .collect();
    let current_same: Vec<(usize, &Entity)> = current
        .iter()
        .enumerate()
        .filter(|(_, e)| e.type_prefix == cur_ent.type_prefix)
        .collect();

    // Find this entity's ordinal position among same-type entities.
    let base_ord = base_same
        .iter()
        .position(|(i, e)| *i == bi && e.start_line == base_ent.start_line)
        .unwrap_or(0);
    let current_ord = current_same
        .iter()
        .position(|(i, e)| *i == ci && e.start_line == cur_ent.start_line)
        .unwrap_or(0);

    // Check neighbors: the entity before and after in the ordering.
    // If both neighbors are different entities (by name), the entity moved.
    // If at least one neighbor is the same, it's still in roughly the same spot.
    let base_before = base_ord
        .checked_sub(1)
        .and_then(|o| base_same.get(o))
        .map(|(_, e)| &e.name);
    let base_after = base_same.get(base_ord + 1).map(|(_, e)| &e.name);
    let cur_before = current_ord
        .checked_sub(1)
        .and_then(|o| current_same.get(o))
        .map(|(_, e)| &e.name);
    let cur_after = current_same.get(current_ord + 1).map(|(_, e)| &e.name);

    // Both neighbors changed → entity moved.
    // If at least one neighbor is the same (or the entity is at a boundary
    // in both versions), it hasn't meaningfully moved.
    let before_changed = base_before != cur_before;
    let after_changed = base_after != cur_after;
    before_changed && after_changed
}

/// Split an entity name into tokens for fuzzy matching.
/// E.g., "validateToken" → {"validate", "token"}, "process_order" → {"process", "order"}.
fn tokenize_entity_name(name: &str) -> Vec<String> {
    // Split on common delimiters: underscores, camelCase boundaries.
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
        } else if ch.is_uppercase() && !current.is_empty() {
            tokens.push(current.to_lowercase());
            current.clear();
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }

    tokens
}

/// Compute Jaccard index between two token sets.
fn jaccard_index(a: &[String], b: &[String]) -> f64 {
    use std::collections::HashSet;

    let set_a: HashSet<_> = a.iter().collect();
    let set_b: HashSet<_> = b.iter().collect();

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

// ── Syntax-token diff (difftastic-style) ─────────────────────────────────
//
// Instead of diffing lines, we diff syntax tokens extracted from tree-sitter.
// This produces semantic hunks: a renamed function shows as a single replacement
// (old name → new name), not a deletion + insertion of the whole function body.
//
// Algorithm:
//   1. Parse both files with tree-sitter.
//   2. Walk both trees depth-first, collecting leaf tokens (kind + text + line).
//   3. Hash each token for efficient comparison.
//   4. LCS diff the two token sequences (using the same `similar` algorithm
//      that Warp's line diff uses, but on tokens instead of lines).
//   5. Convert the token-level diff to line-level hunks.
//   6. Within replacement hunks, do word-level diff for inline highlights.

/// A syntax token extracted from a tree-sitter leaf node.
#[derive(Debug, Clone)]
struct SyntaxToken {
    /// Tree-sitter node kind (e.g., "identifier", "string_literal", "+").
    /// Used for future classification (e.g., treating comments differently).
    _kind: String,
    /// The text content of the token (kept for debugging, not used in diff computation).
    _text: String,
    /// 0-indexed line where this token starts.
    line: usize,
    /// 0-indexed column where this token starts.
    /// Kept for potential future use (column-aware diff rendering).
    _col: usize,
    /// Pre-computed hash for efficient LCS comparison.
    hash: u64,
}

impl SyntaxToken {
    fn new(kind: &str, text: &str, line: usize, col: usize) -> Self {
        let mut hasher = DefaultHasher::new();
        // Hash kind + text so that `x` (identifier) and `"x"` (string) differ.
        kind.hash(&mut hasher);
        text.hash(&mut hasher);
        Self {
            _kind: kind.to_string(),
            _text: text.to_string(),
            line,
            _col: col,
            hash: hasher.finish(),
        }
    }
}

impl PartialEq for SyntaxToken {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for SyntaxToken {}

impl std::hash::Hash for SyntaxToken {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

/// Walk a tree-sitter tree depth-first and collect all leaf tokens.
///
/// Comment and string nodes are treated as opaque: the entire text is emitted
/// as a single token. This prevents fragmented word-level changes within
/// comments/strings and produces clean deletion+addition blocks.
fn collect_tokens(node: Node, source: &str, tokens: &mut Vec<SyntaxToken>) {
    let kind = node.kind();
    let is_comment = kind.contains("comment");
    let is_string = kind.contains("string") || kind.contains("char");

    if is_comment || is_string || node.child_count() == 0 {
        let text = &source[node.byte_range()];
        if !text.is_empty() {
            tokens.push(SyntaxToken::new(
                node.kind(),
                text,
                node.start_position().row,
                node.start_position().column,
            ));
        }
    } else {
        // Recurse into children.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_tokens(child, source, tokens);
        }
    }
}

/// Parse a file and extract its syntax tokens.
///
/// Returns `None` if the language is unsupported or parsing fails.
fn extract_syntax_tokens(content: &str, path: &Path) -> Option<Vec<SyntaxToken>> {
    let language = crate::language_by_filename(path)?;

    let mut parser = Parser::new();
    parser.set_language(&language.grammar).ok()?;
    let tree = parser.parse(content, None)?;

    let mut tokens = Vec::new();
    collect_tokens(tree.root_node(), content, &mut tokens);
    Some(tokens)
}

/// Result of a syntax-token diff: line-level hunks with change classification.
///
/// This is designed to be converted into Warp's `DiffStatus` / `ChangeType`
/// types, replacing the line-based `similar::TextDiff` output.
#[derive(Debug, Clone, Default)]
pub struct SyntaxDiff {
    /// Hunks representing the diff between base and current.
    pub hunks: Vec<SyntaxHunk>,
}

/// A single diff hunk at the line level, produced from the syntax-token diff.
#[derive(Debug, Clone)]
pub enum SyntaxHunk {
    /// Lines that exist in both versions (unchanged).
    Equal {
        base_start: usize,
        base_end: usize,
        current_start: usize,
        current_end: usize,
    },
    /// Lines added in the current version.
    Addition { start: usize, end: usize },
    /// Lines deleted from the base version.
    Deletion {
        start: usize,
        end: usize,
        /// The line in the current version where this deletion is anchored
        /// (the line immediately after the deleted section).
        anchor: usize,
    },
    /// Lines changed between base and current.
    Replacement {
        base_start: usize,
        base_end: usize,
        current_start: usize,
        current_end: usize,
        /// Word-level highlights within the current (added) lines.
        /// Each range is (char_offset_start, char_offset_end) within the
        /// concatenated current-side hunk text.
        insertion_highlights: Vec<std::ops::Range<usize>>,
        /// Word-level highlights within the base (removed) lines.
        /// Each range is (char_offset_start, char_offset_end) within the
        /// concatenated base-side hunk text.
        deletion_highlights: Vec<std::ops::Range<usize>>,
    },
}

/// Compute a syntax-token diff between two file versions.
///
/// This is the difftastic-style semantic diff: it diffs the **tokens** of the
/// tree-sitter parse, not the lines. The result is then mapped back to line-level
/// hunks so it can feed into Warp's existing rendering pipeline.
///
/// Returns `None` if the language is unsupported or parsing fails.
/// Returns `Some(SyntaxDiff)` with empty hunks if both files are identical.
pub fn compute_syntax_diff(
    base_content: &str,
    current_content: &str,
    path: &Path,
) -> Option<SyntaxDiff> {
    let base_tokens = extract_syntax_tokens(base_content, path)?;
    let current_tokens = extract_syntax_tokens(current_content, path)?;

    // Skip if either file has no tokens (empty or parse failure).
    if base_tokens.is_empty() && current_tokens.is_empty() {
        return Some(SyntaxDiff::default());
    }

    // LCS diff on the token sequences.
    // We use `similar` (same crate Warp already uses for line diff) but on
    // token hashes serialized as lines. Each hash becomes one "line",
    // so `diff_lines` with Patience algorithm gives us token-level LCS.
    let base_hashes: Vec<u64> = base_tokens.iter().map(|t| t.hash).collect();
    let current_hashes: Vec<u64> = current_tokens.iter().map(|t| t.hash).collect();

    let base_hash_lines: String = base_hashes.iter().map(|h| format!("{h}\n")).collect();
    let current_hash_lines: String = current_hashes.iter().map(|h| format!("{h}\n")).collect();

    let diffs = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Patience)
        .diff_lines(&base_hash_lines, &current_hash_lines);

    // Map the diff ops back to line ranges using the token position info.
    let hunks = map_ops_to_hunks(
        &diffs.ops(),
        &base_tokens,
        &current_tokens,
        base_content,
        current_content,
    );

    Some(SyntaxDiff { hunks })
}

/// Convert `similar` diff ops (which operate on token-indices) to line-level
/// `SyntaxHunk`s using the position information stored in each token.
fn map_ops_to_hunks(
    ops: &[similar::DiffOp],
    base_tokens: &[SyntaxToken],
    current_tokens: &[SyntaxToken],
    base_content: &str,
    current_content: &str,
) -> Vec<SyntaxHunk> {
    let mut hunks = Vec::new();

    for op in ops {
        match op {
            similar::DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                let base_start = base_tokens[*old_index].line;
                let base_end = base_tokens[*old_index + len - 1].line + 1;
                let current_start = current_tokens[*new_index].line;
                let current_end = current_tokens[*new_index + len - 1].line + 1;
                hunks.push(SyntaxHunk::Equal {
                    base_start,
                    base_end,
                    current_start,
                    current_end,
                });
            }
            similar::DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => {
                let start = base_tokens[*old_index].line;
                let end = base_tokens[*old_index + old_len - 1].line + 1;
                let anchor = if *new_index < current_tokens.len() {
                    current_tokens[*new_index].line
                } else {
                    // Deletion at end of file — anchor past the last line.
                    current_content.lines().count()
                };
                hunks.push(SyntaxHunk::Deletion { start, end, anchor });
            }
            similar::DiffOp::Insert {
                new_index,
                new_len,
                old_index: _,
            } => {
                let start = current_tokens[*new_index].line;
                let end = current_tokens[*new_index + new_len - 1].line + 1;
                hunks.push(SyntaxHunk::Addition { start, end });
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let base_start = base_tokens[*old_index].line;
                let base_end = base_tokens[*old_index + old_len - 1].line + 1;
                let current_start = current_tokens[*new_index].line;
                let current_end = current_tokens[*new_index + new_len - 1].line + 1;

                let base_lines = base_end.saturating_sub(base_start);
                let current_lines = current_end.saturating_sub(current_start);

                if base_lines <= 1 && current_lines <= 1 {
                    // Single-line change: use inline word-level highlights for
                    // precise character-level diffing (e.g. renamed function).
                    let insertion_highlights = compute_word_highlights(
                        base_content,
                        current_content,
                        base_start,
                        base_end,
                        current_start,
                        current_end,
                    );
                    let deletion_highlights = compute_word_highlights_for_deletion(
                        base_content,
                        current_content,
                        base_start,
                        base_end,
                        current_start,
                        current_end,
                    );

                    hunks.push(SyntaxHunk::Replacement {
                        base_start,
                        base_end,
                        current_start,
                        current_end,
                        insertion_highlights,
                        deletion_highlights,
                    });
                } else {
                    // Multi-line rewrite: emit clean block deletions/additions
                    // rather than fragmented inline highlights.
                    hunks.push(SyntaxHunk::Deletion {
                        start: base_start,
                        end: base_end,
                        anchor: current_start,
                    });
                    hunks.push(SyntaxHunk::Addition {
                        start: current_start,
                        end: current_end,
                    });
                }
            }
        }
    }

    hunks
}

/// Compute word-level highlights for the current (added) side of a replacement.
///
/// Within a replacement, some tokens may be unchanged (same hash on both sides).
/// Only the truly novel tokens should get word-level highlights.
/// This is the difftastic approach: unchanged words inside a changed hunk are
/// shown normally, only novel words are highlighted.
/// Compute word-level highlights for the current (insertion) side of a replacement.
///
/// Character offsets are relative to the concatenated text of the current-side
/// hunk lines (each line followed by a newline), matching the format expected
/// by `ChangeType::Replacement::insertion`.
///
/// Uses `similar::TextDiff::from_lines` on the actual hunk texts, then walks
/// through all ops tracking cumulative new-side character offsets and using
/// `iter_inline_changes` on Replace ops to get word-level highlights.
fn compute_word_highlights(
    base_content: &str,
    current_content: &str,
    base_start_line: usize,
    base_end_line: usize,
    current_start_line: usize,
    current_end_line: usize,
) -> Vec<std::ops::Range<usize>> {
    let base_hunk_text = extract_hunk_text(base_content, base_start_line, base_end_line);
    let current_hunk_text =
        extract_hunk_text(current_content, current_start_line, current_end_line);

    let text_diff = similar::TextDiff::from_lines(&base_hunk_text, &current_hunk_text);

    let mut highlights = Vec::new();
    let mut new_offset = 0usize;

    for op in text_diff.ops() {
        match &op {
            similar::DiffOp::Equal { new_index, len, .. } => {
                // Skip equal lines on the new side.
                for i in *new_index..*new_index + len {
                    if let Some(line) = current_hunk_text.lines().nth(i) {
                        new_offset += line.chars().count() + 1; // +1 for newline
                    }
                }
            }
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => {
                // Pure insertions — all text is novel.
                for i in *new_index..*new_index + new_len {
                    if let Some(line) = current_hunk_text.lines().nth(i) {
                        let char_len = line.chars().count();
                        if !line.trim().is_empty() {
                            highlights.push(new_offset..new_offset + char_len);
                        }
                        new_offset += char_len + 1;
                    }
                }
            }
            similar::DiffOp::Delete { .. } => {
                // No current-side content for deletions.
            }
            similar::DiffOp::Replace { .. } => {
                // Use `iter_inline_changes` to get word-level highlights within
                // this replaced region. Track new_offset as we go.
                for inline_change in text_diff.iter_inline_changes(&op) {
                    match inline_change.tag() {
                        similar::ChangeTag::Insert => {
                            for (highlighted, val) in inline_change.values() {
                                let char_len = val.chars().count();
                                if *highlighted {
                                    highlights.push(new_offset..new_offset + char_len);
                                }
                                new_offset += char_len;
                            }
                        }
                        similar::ChangeTag::Equal => {
                            for (_, val) in inline_change.values() {
                                new_offset += val.chars().count();
                            }
                        }
                        similar::ChangeTag::Delete => {
                            // Deleted text has no current-side position.
                        }
                    }
                }
            }
        }
    }

    highlights
}

/// Compute word-level highlights for the base (deletion) side of a replacement.
///
/// Same approach as `compute_word_highlights` but tracks old-side offsets.
fn compute_word_highlights_for_deletion(
    base_content: &str,
    current_content: &str,
    base_start_line: usize,
    base_end_line: usize,
    current_start_line: usize,
    current_end_line: usize,
) -> Vec<std::ops::Range<usize>> {
    let base_hunk_text = extract_hunk_text(base_content, base_start_line, base_end_line);
    let current_hunk_text =
        extract_hunk_text(current_content, current_start_line, current_end_line);

    let text_diff = similar::TextDiff::from_lines(&base_hunk_text, &current_hunk_text);

    let mut highlights = Vec::new();
    let mut old_offset = 0usize;

    for op in text_diff.ops() {
        match &op {
            similar::DiffOp::Equal { old_index, len, .. } => {
                for i in *old_index..*old_index + len {
                    if let Some(line) = base_hunk_text.lines().nth(i) {
                        old_offset += line.chars().count() + 1;
                    }
                }
            }
            similar::DiffOp::Insert { .. } => {
                // No base-side content for insertions.
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for i in *old_index..*old_index + old_len {
                    if let Some(line) = base_hunk_text.lines().nth(i) {
                        let char_len = line.chars().count();
                        if !line.trim().is_empty() {
                            highlights.push(old_offset..old_offset + char_len);
                        }
                        old_offset += char_len + 1;
                    }
                }
            }
            similar::DiffOp::Replace { .. } => {
                for inline_change in text_diff.iter_inline_changes(&op) {
                    match inline_change.tag() {
                        similar::ChangeTag::Delete => {
                            for (highlighted, val) in inline_change.values() {
                                let char_len = val.chars().count();
                                if *highlighted {
                                    highlights.push(old_offset..old_offset + char_len);
                                }
                                old_offset += char_len;
                            }
                        }
                        similar::ChangeTag::Equal => {
                            for (_, val) in inline_change.values() {
                                old_offset += val.chars().count();
                            }
                        }
                        similar::ChangeTag::Insert => {
                            // Inserted text has no base-side position.
                        }
                    }
                }
            }
        }
    }

    highlights
}

/// Extract lines `[start_line, end_line)` from content as a single string
/// with newlines. Line numbers are 0-indexed.
fn extract_hunk_text(content: &str, start_line: usize, end_line: usize) -> String {
    content
        .lines()
        .skip(start_line)
        .take(end_line - start_line)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(
        name: &str,
        type_prefix: Option<&str>,
        start: usize,
        end: usize,
        hash: u64,
    ) -> Entity {
        Entity {
            name: name.to_string(),
            type_prefix: type_prefix.map(String::from),
            start_line: start,
            end_line: end,
            body_hash: hash,
        }
    }

    #[test]
    fn test_extract_entities_rust() {
        let content = r#"
fn hello() {
    println!("hello");
}

struct Config {
    name: String,
}

fn world() {
    println!("world");
}
"#;
        let path = Path::new("test.rs");
        let entities = extract_entities(content, path).expect("should extract entities");

        assert!(
            entities.len() >= 3,
            "expected at least 3 entities, got {}: {:?}",
            entities.len(),
            entities
        );

        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello"), "missing 'hello': {:?}", names);
        assert!(names.contains(&"Config"), "missing 'Config': {:?}", names);
        assert!(names.contains(&"world"), "missing 'world': {:?}", names);
    }

    #[test]
    fn test_extract_entities_unsupported_language() {
        let content = "some text";
        let path = Path::new("test.xyz");
        assert!(extract_entities(content, path).is_none());
    }

    #[test]
    fn test_diff_entities_modified() {
        let base = vec![make_entity("foo", Some("fn"), 0, 5, 111)];
        let current = vec![
            make_entity("foo", Some("fn"), 0, 7, 222), // different hash = modified
        ];

        let diff = diff_entities(&base, &current);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, EntityChangeKind::Modified);
    }

    #[test]
    fn test_diff_entities_unchanged() {
        let base = vec![make_entity("foo", Some("fn"), 0, 5, 111)];
        let current = vec![
            make_entity("foo", Some("fn"), 0, 5, 111), // same hash = unchanged
        ];

        let diff = diff_entities(&base, &current);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, EntityChangeKind::Unchanged);
    }

    #[test]
    fn test_diff_entities_renamed() {
        let base = vec![make_entity("old_name", Some("fn"), 0, 5, 111)];
        let current = vec![
            make_entity("new_name", Some("fn"), 0, 5, 111), // same hash, different name = renamed
        ];

        let diff = diff_entities(&base, &current);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(
            diff.changes[0].kind,
            EntityChangeKind::Renamed {
                old_name: "old_name".to_string()
            }
        );
    }

    #[test]
    fn test_diff_entities_added() {
        let base: Vec<Entity> = vec![];
        let current = vec![make_entity("new_fn", Some("fn"), 0, 5, 111)];

        let diff = diff_entities(&base, &current);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, EntityChangeKind::Added);
    }

    #[test]
    fn test_diff_entities_deleted() {
        let base = vec![make_entity("old_fn", Some("fn"), 0, 5, 111)];
        let current: Vec<Entity> = vec![];

        let diff = diff_entities(&base, &current);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, EntityChangeKind::Deleted);
    }

    #[test]
    fn test_diff_entities_moved() {
        let base = vec![
            make_entity("foo", Some("fn"), 0, 5, 111),
            make_entity("bar", Some("fn"), 10, 15, 222),
        ];
        // "foo" same body but moved to line 50 (shifted by 50 > 3)
        let current = vec![
            make_entity("bar", Some("fn"), 10, 15, 222),
            make_entity("foo", Some("fn"), 50, 55, 111),
        ];

        let diff = diff_entities(&base, &current);
        let foo_change = diff.changes.iter().find(|c| {
            c.current.as_ref().map_or(false, |e| e.name == "foo")
                || c.base.as_ref().map_or(false, |e| e.name == "foo")
        });
        assert!(foo_change.is_some());
        assert_eq!(foo_change.unwrap().kind, EntityChangeKind::Moved);
    }

    #[test]
    fn test_diff_entities_mixed() {
        let base = vec![
            make_entity("fn_a", Some("fn"), 0, 5, 111),
            make_entity("fn_b", Some("fn"), 10, 15, 222),
            make_entity("fn_c", Some("fn"), 20, 25, 333),
            make_entity("fn_d", Some("fn"), 30, 35, 444),
        ];
        let current = vec![
            make_entity("fn_a", Some("fn"), 0, 5, 111),   // unchanged
            make_entity("fn_b", Some("fn"), 10, 20, 555), // modified (different hash)
            make_entity("fn_c_renamed", Some("fn"), 20, 25, 333), // renamed (same hash, different name)
            // fn_d is deleted
            make_entity("fn_e", Some("fn"), 40, 45, 666), // added
        ];

        let diff = diff_entities(&base, &current);
        assert_eq!(diff.changes.len(), 5);

        let kinds: Vec<_> = diff.changes.iter().map(|c| c.kind.clone()).collect();
        assert!(kinds.contains(&EntityChangeKind::Unchanged));
        assert!(kinds.contains(&EntityChangeKind::Modified));
        assert!(kinds.contains(&EntityChangeKind::Renamed {
            old_name: "fn_c".to_string()
        }));
        assert!(kinds.contains(&EntityChangeKind::Added));
        assert!(kinds.contains(&EntityChangeKind::Deleted));

        let significant = diff.significant_changes();
        assert_eq!(significant.len(), 4); // all except Unchanged
    }

    #[test]
    fn test_compute_entity_diff() {
        let base = r#"
fn hello() {
    println!("hello");
}

struct Config {
    name: String,
}
"#;
        let current = r#"
fn hello() {
    println!("hello world");
}

struct Config {
    name: String,
    value: i32,
}
"#;
        let path = Path::new("test.rs");
        let diff = compute_entity_diff(base, current, path).expect("should compute entity diff");

        assert!(diff.changes.len() >= 2, "expected at least 2 changes");
        // At least one modified (hello or Config, since body changed)
        let modified_count = diff
            .changes
            .iter()
            .filter(|c| c.kind == EntityChangeKind::Modified)
            .count();
        assert!(modified_count >= 1, "expected at least 1 modified entity");
    }

    #[test]
    fn test_compute_entity_diff_no_entities() {
        let base = "just some text\nno entities\n";
        let current = "just some text\nno entities\n";
        let path = Path::new("test.rs");
        // A .rs file with no entities — extract_entities returns empty vec.
        // compute_entity_diff returns None when both are empty.
        let result = compute_entity_diff(base, current, path);
        assert!(result.is_none());
    }

    #[test]
    fn test_formatting_only_unchanged() {
        let base = "fn foo() {\n    let x = 1;\n    let y = 2;\n}\n";
        let current = "fn foo() {\n  let x = 1;\n  let y = 2;\n}\n";
        let path = Path::new("test.rs");

        let diff = compute_entity_diff(base, current, path).expect("should compute diff");
        // Whitespace-only changes should produce the same body_hash → Unchanged
        let foo_change = diff
            .changes
            .iter()
            .find(|c| c.current.as_ref().map_or(false, |e| e.name == "foo"));
        assert!(foo_change.is_some());
        assert_eq!(foo_change.unwrap().kind, EntityChangeKind::Unchanged);
    }

    #[test]
    fn test_tokenize_entity_name() {
        assert_eq!(
            tokenize_entity_name("validateToken"),
            vec!["validate", "token"]
        );
        assert_eq!(
            tokenize_entity_name("process_order"),
            vec!["process", "order"]
        );
        assert_eq!(
            tokenize_entity_name("XMLParser"),
            vec!["x", "m", "l", "parser"]
        );
        assert_eq!(tokenize_entity_name("foo"), vec!["foo"]);
    }

    #[test]
    fn test_jaccard_index() {
        let a: Vec<String> = vec!["validate".to_string(), "token".to_string()];
        let b: Vec<String> = vec!["validate".to_string(), "token".to_string()];
        assert!((jaccard_index(&a, &b) - 1.0).abs() < f64::EPSILON);

        let c: Vec<String> = vec!["validate".to_string(), "user".to_string()];
        assert!(jaccard_index(&a, &c) > 0.3 && jaccard_index(&a, &c) < 0.7);

        let d: Vec<String> = vec!["completely".to_string(), "different".to_string()];
        assert!(jaccard_index(&a, &d) < 0.1);
    }

    #[test]
    fn test_extract_entities_python() {
        let content = r#"
def hello():
    print("hello")

class Config:
    name: str
    def __init__(self):
        pass

def world():
    print("world")
"#;
        let path = Path::new("test.py");
        let entities = extract_entities(content, path).expect("should extract Python entities");

        assert!(
            entities.len() >= 3,
            "expected at least 3 entities, got {}: {:?}",
            entities.len(),
            entities
        );
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello"), "missing 'hello': {:?}", names);
        assert!(names.contains(&"Config"), "missing 'Config': {:?}", names);
        assert!(names.contains(&"world"), "missing 'world': {:?}", names);
    }

    #[test]
    fn test_extract_entities_javascript() {
        let content = r#"
function hello() {
    console.log('hello');
}

class Config {
    constructor() {
        this.name = 'test';
    }
}
"#;
        let path = Path::new("test.js");
        let entities = extract_entities(content, path).expect("should extract JS entities");

        assert!(
            entities.len() >= 2,
            "expected at least 2 entities, got {}: {:?}",
            entities.len(),
            entities
        );
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello"), "missing 'hello': {:?}", names);
        assert!(names.contains(&"Config"), "missing 'Config': {:?}", names);
    }

    #[test]
    fn test_rename_detection_end_to_end() {
        // Base version has `old_fn`, current version has `new_fn` with same body.
        let base = r#"
fn old_fn() {
    let x = 1;
    println!("hello");
}
"#;
        let current = r#"
fn new_fn() {
    let x = 1;
    println!("hello");
}
"#;
        let path = Path::new("test.rs");
        let diff = compute_entity_diff(base, current, path).expect("should compute diff");

        let rename_changes: Vec<_> = diff
            .changes
            .iter()
            .filter(|c| matches!(c.kind, EntityChangeKind::Renamed { .. }))
            .collect();
        assert_eq!(
            rename_changes.len(),
            1,
            "expected exactly 1 rename, got {:?}",
            diff.changes
        );
        if let EntityChangeKind::Renamed { old_name } = &rename_changes[0].kind {
            assert_eq!(old_name, "old_fn");
        }
        assert_eq!(rename_changes[0].current.as_ref().unwrap().name, "new_fn");
    }

    #[test]
    fn test_entity_diff_for_language_without_identifiers() {
        // YAML has no identifiers.scm
        let base = "key: value\n";
        let current = "key: new_value\n";
        let path = Path::new("test.yaml");
        // Should return None since YAML has no identifiers.scm
        assert!(compute_entity_diff(base, current, path).is_none());
    }

    #[test]
    fn test_body_hash_whitespace_normalization() {
        let content_a = "fn foo() {\n    let x = 1;\n}\n";
        let content_b = "fn foo() {\nlet x = 1;\n}\n";

        // "foo" is at byte range 3..6 in both
        let hash_a = compute_body_hash(content_a, 0, 3, 3, 6);
        let hash_b = compute_body_hash(content_b, 0, 3, 3, 6);
        assert_eq!(
            hash_a, hash_b,
            "whitespace-normalized body hashes should match"
        );
    }

    #[test]
    fn test_body_hash_rename_invariant() {
        let content_a = "fn foo() {\n    let x = 1;\n}\n";
        let content_b = "fn bar() {\n    let x = 1;\n}\n";

        // "foo" at 3..6, "bar" at 3..6 — names are masked so hashes match
        let hash_a = compute_body_hash(content_a, 0, 3, 3, 6);
        let hash_b = compute_body_hash(content_b, 0, 3, 3, 6);
        assert_eq!(
            hash_a, hash_b,
            "renamed function should produce same body hash when name is masked"
        );
    }

    #[test]
    fn test_body_hash_single_line_entity() {
        let content_a = "fn foo() { 1 }\n";
        let content_b = "fn foo() { 2 }\n";

        // "foo" at byte range 3..6
        let hash_a = compute_body_hash(content_a, 0, 1, 3, 6);
        let hash_b = compute_body_hash(content_b, 0, 1, 3, 6);
        assert_ne!(
            hash_a, hash_b,
            "single-line entity body change should produce different hash"
        );
    }

    #[test]
    fn test_insertion_above_does_not_cause_moved() {
        // Adding a helper at the top should NOT classify later functions as Moved.
        // base: [foo(0-5), bar(10-15)]
        // current: [helper(0-3), foo(5-10), bar(15-20)] — all same bodies, shifted down
        let base = vec![
            make_entity("foo", Some("fn"), 0, 5, 111),
            make_entity("bar", Some("fn"), 10, 15, 222),
        ];
        let current = vec![
            make_entity("helper", Some("fn"), 0, 3, 999), // added above
            make_entity("foo", Some("fn"), 5, 10, 111),   // same body, shifted down
            make_entity("bar", Some("fn"), 15, 20, 222),  // same body, shifted down
        ];

        let diff = diff_entities(&base, &current);
        // foo: base_rank=0/2=0%, current_rank=1/3=33%, diff=33% — borderline
        // bar: base_rank=1/2=50%, current_rank=2/3=67%, diff=17% — borderline
        // Both should NOT be Moved because their relative ordering is preserved.
        let moved: Vec<_> = diff
            .changes
            .iter()
            .filter(|c| c.kind == EntityChangeKind::Moved)
            .collect();
        assert_eq!(
            moved.len(),
            0,
            "insertions above should not cause Moved classification"
        );
    }

    // ── Syntax diff tests ────────────────────────────────────────────────────

    #[test]
    fn test_compute_syntax_diff_identical() {
        let base = "fn hello() {\n    println!(\"hello\");\n}\n";
        let current = base;
        let path = Path::new("test.rs");

        let diff = compute_syntax_diff(base, current, path).expect("should compute syntax diff");
        // Identical files should produce no changes (all Equal hunks, or empty).
        let non_equal: Vec<_> = diff
            .hunks
            .iter()
            .filter(|h| !matches!(h, SyntaxHunk::Equal { .. }))
            .collect();
        assert!(
            non_equal.is_empty(),
            "identical files should have no changes"
        );
    }

    #[test]
    fn test_compute_syntax_diff_addition() {
        let base = "fn hello() {\n    println!(\"hello\");\n}\n";
        let current = "fn hello() {\n    println!(\"hello\");\n}\nfn world() {\n    println!(\"world\");\n}\n";
        let path = Path::new("test.rs");

        let diff = compute_syntax_diff(base, current, path).expect("should compute syntax diff");
        // Should have an Addition hunk for the new `world` function.
        let additions: Vec<_> = diff
            .hunks
            .iter()
            .filter_map(|h| match h {
                SyntaxHunk::Addition { start, end } => Some(*start..*end),
                _ => None,
            })
            .collect();
        assert!(!additions.is_empty(), "should have at least one addition");
    }

    #[test]
    fn test_compute_syntax_diff_deletion() {
        let base = "fn hello() {\n    println!(\"hello\");\n}\nfn world() {\n    println!(\"world\");\n}\n";
        let current = "fn hello() {\n    println!(\"hello\");\n}\n";
        let path = Path::new("test.rs");

        let diff = compute_syntax_diff(base, current, path).expect("should compute syntax diff");
        // Should have a Deletion hunk for the removed `world` function.
        let deletions: Vec<_> = diff
            .hunks
            .iter()
            .filter_map(|h| match h {
                SyntaxHunk::Deletion { start, end, anchor } => Some((*start..*end, *anchor)),
                _ => None,
            })
            .collect();
        assert!(!deletions.is_empty(), "should have at least one deletion");
    }

    #[test]
    fn test_compute_syntax_diff_rename() {
        // Renaming a function: the old name is deleted, the new name is inserted.
        // With syntax-token diff, the function body tokens should match as Equal,
        // and only the name token should be in a Replacement hunk.
        let base = "fn old_name() {\n    let x = 1;\n}\n";
        let current = "fn new_name() {\n    let x = 1;\n}\n";
        let path = Path::new("test.rs");

        let diff = compute_syntax_diff(base, current, path).expect("should compute syntax diff");

        // The key property: the diff should NOT treat the entire function body as
        // changed. Only the name token should differ.
        let replacements: Vec<_> = diff
            .hunks
            .iter()
            .filter_map(|h| match h {
                SyntaxHunk::Replacement {
                    base_start,
                    base_end,
                    current_start,
                    current_end,
                    ..
                } => Some((*base_start..*base_end, *current_start..*current_end)),
                _ => None,
            })
            .collect();

        // The replacement hunk should span only 1-2 lines (the name), not the
        // entire function body.
        let max_hunk_size = replacements
            .iter()
            .map(|(base, cur)| base.len().max(cur.len()))
            .max()
            .unwrap_or(0);
        assert!(
            max_hunk_size <= 2,
            "rename should produce a small replacement hunk, got {max_hunk_size} lines: {replacements:?}"
        );
    }

    #[test]
    fn test_compute_syntax_diff_unsupported_language() {
        let base = "some text";
        let current = "some other text";
        let path = Path::new("test.xyz");

        // Should return None for unsupported languages.
        assert!(compute_syntax_diff(base, current, path).is_none());
    }

    #[test]
    fn test_extract_syntax_tokens_rust() {
        let content = "fn hello() {\n    println!(\"hello\");\n}\n";
        let path = Path::new("test.rs");

        let tokens = extract_syntax_tokens(content, path).expect("should extract tokens");
        assert!(!tokens.is_empty(), "should have tokens");

        // Should contain at least: fn, hello, (, ), {, println, !, (, "hello", ), ;, }
        let texts: Vec<&str> = tokens.iter().map(|t| t._text.as_str()).collect();
        assert!(texts.contains(&"fn"), "missing 'fn': {texts:?}");
        assert!(texts.contains(&"hello"), "missing 'hello': {texts:?}");
        assert!(texts.contains(&"println"), "missing 'println': {texts:?}");
    }

    #[test]
    fn test_word_highlights_rename_function() {
        // Verify word highlights are character offsets within actual line text.
        let base = "fn old_name() {\n    let x = 1;\n}\n";
        let current = "fn new_name() {\n    let x = 1;\n}\n";
        let path = Path::new("test.rs");

        let diff = compute_syntax_diff(base, current, path).expect("should compute syntax diff");

        // Find the replacement hunk.
        let replacement = diff.hunks.iter().find_map(|h| match h {
            SyntaxHunk::Replacement {
                insertion_highlights,
                deletion_highlights,
                ..
            } => Some((insertion_highlights.clone(), deletion_highlights.clone())),
            _ => None,
        });

        if let Some((insertion, deletion)) = replacement {
            // Current hunk text: "fn new_name() {\n    let x = 1;\n}\n"
            // The insertion highlight should cover "new_name" which is at chars 3..11.
            // If it's a single-line Replace (just line 0 changed), the offset is
            // within that line: 'f'=0, 'n'=1, ' '=2, 'n'=3... so "new_name"
            // starts at char 3 and ends at char 11.
            //
            // For a multi-line Replace covering the whole function, the offset
            // includes previous lines. Either way, "new_name" should be in the highlights.
            let highlighted_text: String = {
                let current_hunk = "fn new_name() {\n    let x = 1;\n}\n";
                insertion
                    .iter()
                    .map(|r| {
                        current_hunk
                            .chars()
                            .skip(r.start)
                            .take(r.end - r.start)
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            };
            assert!(
                highlighted_text.contains("new_name"),
                "insertion highlights should contain 'new_name', got: {highlighted_text:?} (ranges: {insertion:?})"
            );

            // Deletion highlights should cover "old_name".
            let deleted_text: String = {
                let base_hunk = "fn old_name() {\n    let x = 1;\n}\n";
                deletion
                    .iter()
                    .map(|r| {
                        base_hunk
                            .chars()
                            .skip(r.start)
                            .take(r.end - r.start)
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            };
            assert!(
                deleted_text.contains("old_name"),
                "deletion highlights should contain 'old_name', got: {deleted_text:?} (ranges: {deletion:?})"
            );
        }
    }

    #[test]
    fn test_comment_rewrite_is_opaque() {
        // When an entire comment block is rewritten, it should show as clean
        // Deletion + Addition blocks, not fragmented Replacement hunks with
        // partial word matches.
        let base = "// Original version header\n// Some old description\nfn foo() {}\n";
        let current =
            "// Modified version header\n// A totally different description\nfn foo() {}\n";
        let path = Path::new("test.rs");

        let diff = compute_syntax_diff(base, current, path).expect("should compute syntax diff");

        // Should have NO Replacement hunks for comments.
        let replacements: Vec<_> = diff
            .hunks
            .iter()
            .filter(|h| matches!(h, SyntaxHunk::Replacement { .. }))
            .collect();
        assert!(
            replacements.is_empty(),
            "comment rewrites should not produce Replacement hunks, got {replacements:?}"
        );

        // Should have Deletion + Addition for the comment block.
        let deletions: Vec<_> = diff
            .hunks
            .iter()
            .filter_map(|h| match h {
                SyntaxHunk::Deletion { start, end, .. } => Some(*start..*end),
                _ => None,
            })
            .collect();
        let additions: Vec<_> = diff
            .hunks
            .iter()
            .filter_map(|h| match h {
                SyntaxHunk::Addition { start, end } => Some(*start..*end),
                _ => None,
            })
            .collect();
        assert!(
            !deletions.is_empty(),
            "should have a deletion hunk for old comments"
        );
        assert!(
            !additions.is_empty(),
            "should have an addition hunk for new comments"
        );
    }
}
