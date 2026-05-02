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
}
