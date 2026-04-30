# TECH.md — Search sessions by custom tab and pane names

Issue: https://github.com/warpdotdev/warp/issues/9155
Sibling docs: [product.md](product.md) (behavior contract).

## Context

This change makes user-set custom tab and pane names searchable on both session-search surfaces. Behavior is fully specified in `product.md`. This spec describes the implementation only.

Two concrete gaps to close:

- **Command palette:** `SessionNavigationData` carries no custom-name fields, and `searchable_session_string_and_ranges` indexes only prompt + command + hint ([search.rs:127-190](app/src/search/command_palette/navigation/search.rs#L127-L190)). Both names need to enter the document and the searchable string.
- **Vertical-tabs sidebar:** the `!uses_outer_group_container(display_granularity)` gate at [vertical_tabs.rs:1022-1024](app/src/workspace/view/vertical_tabs.rs#L1022-L1024) drops `pane_group.custom_title` from per-pane search fragments in Panes mode only. Removing the gate at the search-input layer (without touching render) closes the gap.

Two facts settled during spec review:

- `PaneConfiguration.custom_vertical_tabs_title` already round-trips through SQLite via migration `2026-04-17-020439_add_custom_vertical_tabs_title_to_pane_leaves`, with save at [sqlite.rs:1075](app/src/persistence/sqlite.rs#L1075) and restore at [sqlite.rs:2607](app/src/persistence/sqlite.rs#L2607). No persistence work needed (PRODUCT invariant 5).
- In Panes mode, threading the tab `custom_title` into `title_override` is sufficient on its own: every pane in a tab-name-matched tab independently passes `pane_matches_query` because `pane_search_text_fragments` already includes `display_title_override` ([vertical_tabs.rs:2861-2874](app/src/workspace/view/vertical_tabs.rs#L2861)). No render-side change required.

## Proposed changes

### Data model — extend `SessionNavigationData`

Add two fields to [`SessionNavigationData`](app/src/session_management.rs#L18-L35):

- `custom_tab_name: Option<String>`
- `custom_pane_name: Option<String>`

Both normalized through a new `normalize_custom_name(&str) -> Option<String>` helper that trims and returns `None` for empty / whitespace-only input. Single helper shared between palette population and any future caller (PRODUCT invariant 2).

Populate in [`PaneGroup::pane_sessions`](app/src/pane_group/mod.rs#L2353-L2361):

- `custom_tab_name` from `pane_group.custom_title(app)`.
- `custom_pane_name` from `pane.custom_vertical_tabs_title()` ([pane/mod.rs:735](app/src/pane_group/pane/mod.rs#L735)).

Both run through `normalize_custom_name`. Extend `SessionNavigationData::new`'s signature and the `TerminalPane::session_navigation_data` call site to thread the new fields through.

### Palette — searchable string + highlights

Extend [`searchable_session_string_and_ranges`](app/src/search/command_palette/navigation/search.rs#L127-L190) to prepend the names. Concat order matches PRODUCT invariant 12's visual order (tab first, then pane):

```
[custom_tab_name?] [custom_pane_name?] prompt [command] [hint]
```

Char-counted ranges are tracked alongside the existing `command_range` / `hint_text_range` on `SearchableSessionStringRanges`:

- `custom_tab_name_range: Option<Range<usize>>`
- `custom_pane_name_range: Option<Range<usize>>`

Extend [`SessionHighlightIndices`](app/src/search/command_palette/navigation/search.rs#L54-L87):

- `custom_tab_name_indices: Option<Vec<usize>>`
- `custom_pane_name_indices: Option<Vec<usize>>`

Construction follows the same pattern as `command_indices` (filter matched indices into the named range, subtract `range.start`). Both backends call `SessionHighlightIndices::new` with the same ranges struct, so fuzzy and Tantivy paths get the highlight extension for free. Multi-byte handling continues to flow through `byte_indices_to_char_indices` on the Tantivy path (PRODUCT invariant 4).

The Tantivy schema at [search.rs:259-266](app/src/search/command_palette/navigation/search.rs#L259) is **unchanged**: custom names live in the same single `session` field. Ranking via per-field boost was considered and explicitly dropped — the simpler concat keeps highlights, multi-byte handling, and tie-break (last-focus-ts) untouched with no schema delta. Both backends rebuild from `all_sessions` per query — Tantivy via `searcher.build_index(documents)` at [search.rs:278-296](app/src/search/command_palette/navigation/search.rs#L278-L296), fuzzy by in-memory iteration — so renames take effect on the next search with no invalidation hook (PRODUCT invariant 6).

### Palette — result row render

Update [`SearchItem`](app/src/search/command_palette/navigation/search_item.rs) and [`render.rs`](app/src/search/command_palette/navigation/render.rs):

- Read `custom_tab_name`, `custom_pane_name`, and the two new highlight index vecs from `MatchedSession`.
- Emit zero, one, or two leading label segments at the start of the row's primary line per PRODUCT invariants 12–13.
- Apply the existing prompt/command/hint highlight treatment to each label using the new index vecs (PRODUCT invariant 14).
- Truncation around the matched substring (PRODUCT invariant 15): when a label region needs truncation and the query matched inside it, anchor the truncation away from the match window. Reuse the line-truncation primitive used today for prompt/command/hint. When both labels are present and combined width exceeds available space, share rather than starve.
- Theming + accessibility (PRODUCT invariant 19): use existing themed text styles so themes work for free; add label text to the row's accessible name alongside prompt/command/hint.

Visual treatment between the two segments (separator, weight, color) is a design open question (referenced in PRODUCT invariant 12). The contract — two distinguishable segments, tab first, both highlight-capable — is fixed here; pixel decisions defer to design.

### Sidebar — close the Panes-mode gap

The sidebar has **two** filter sites that build per-pane `PaneProps` to feed `pane_matches_query`. Both must drop the `!uses_outer_group_container(display_granularity)` gate so the tab `custom_title` reaches the per-pane fragments in Panes mode:

- [`matching_tab_indices`](app/src/workspace/view/vertical_tabs.rs#L980-L1035) at [vertical_tabs.rs:1022-1024](app/src/workspace/view/vertical_tabs.rs#L1022-L1024) — drives keyboard tab navigation ([`activate_prev_tab` / `activate_next_tab`](app/src/workspace/view.rs#L10076)).
- [`render_groups`](app/src/workspace/view/vertical_tabs.rs#L1490-L1620) at [vertical_tabs.rs:1547-1549](app/src/workspace/view/vertical_tabs.rs#L1547-L1549) — drives the visible Panes-mode list.

The edit is identical at both:

```rust
// before
let title_override = (!uses_outer_group_container(display_granularity))
    .then(|| pane_group.custom_title(app))
    .flatten();

// after
let title_override = pane_group.custom_title(app);
```

Both edits are render-safe: the `PaneProps` constructed in these branches are throwaway match-only objects consumed by `pane_matches_query` and discarded; they are never rendered. The render-side gate lives independently at [vertical_tabs.rs:1777](app/src/workspace/view/vertical_tabs.rs#L1777) (`displayed_tab_title_override`, used by `render_tab_group` at lines 1828 / 1881) and is **untouched** — the tab name still renders only as a group header in Panes mode, not on each pane row (PRODUCT invariant 17). The sidebar already includes the tab `custom_title` in Summary mode and the pane `custom_vertical_tabs_title` in FocusedSession + Panes modes; no other sidebar changes are required (PRODUCT invariants 1, 10, 11). Sidebar remains window-scoped — both filter sites iterate `workspace.tabs` for the current window only — so PRODUCT invariant 8 (multi-window) is satisfied via the palette's existing `all_sessions(app)` window walk, not by any sidebar change.

Whitespace-only handling is already correct: `search_fragments_contain_query` ([vertical_tabs.rs:2854](app/src/workspace/view/vertical_tabs.rs#L2854)) skips whitespace-only fragments today. Names are normalized at the source via `normalize_custom_name` so display offsets agree with palette match ranges.

### End-to-end flow (palette)

1. User renames a tab → `finish_tab_rename` → `pane_group.set_title(...)`. No event-bus publish; no invalidation needed.
2. User opens the command palette and types a query.
3. `FullTextSessionSearcher::search` (or `FuzzySessionSearcher::search`) calls `SessionNavigationData::all_sessions(app)`.
4. `all_sessions` walks windows → workspaces → `PaneGroup::pane_sessions`, which now populates `custom_tab_name` and `custom_pane_name` from live state.
5. `searchable_session_string_and_ranges` produces the concat string + char-indexed ranges (now including the two new name ranges).
6. Backend matches; `SessionHighlightIndices::new` partitions matched indices into per-field index vecs.
7. `SearchItem` / `render.rs` reads names + per-field highlight indices and renders zero, one, or two leading labels with the existing highlight treatment.

## Testing and validation

Each numbered invariant in `product.md` (1–21 after the boost was dropped) maps to a concrete check.

| Invariant | Mechanism |
|---|---|
| 1 (custom names indexed) | Unit tests on `searchable_session_string_and_ranges` covering Some/Some, Some/None, None/Some, None/None × {tab, pane}. |
| 2 (whitespace-only → None) | Unit test on `normalize_custom_name`. |
| 3 (case-insensitive substring) | One regression test per backend (fuzzy + Tantivy). |
| 4 (multi-byte) | Extend [search_tests.rs](app/src/search/command_palette/navigation/search_tests.rs) — emoji + CJK custom names, byte-vs-char highlight ranges. |
| 5 (restored sessions) | Integration test: set names, restart, query. Persistence already verified to round-trip. |
| 6 (renames take effect immediately) | Integration test: rename, immediately query, assert hit. Falls out of per-query rebuild. |
| 7 (multi-pane) | Integration test: tab with two panes, names on tab + one pane; query each independently, assert correct row count. |
| 8 (multi-window) | Integration test with two windows; existing cross-window plumbing through `app.window_ids()`. |
| 9 (no dup rows on multi-field match) | Unit test: query that hits multiple ranges produces one `MatchedSession`. |
| 10 (sidebar parity) | Integration test in each of Summary, FocusedSession, Panes modes. |
| 11 (no sidebar regression) | Existing sidebar tests in `vertical_tabs.rs` test module stay green. |
| 12–15 (row render + truncation) | Snapshot / widget tests for `SearchItem` covering 0/1/2-label cases, highlights, and anchored truncation at narrow widths. |
| 16 (no length cap) | Covered by absence of cap code; manual review confirms no new validation gate. |
| 17 (sidebar render unchanged) | Existing sidebar render tests stay green; no edits to `PaneProps::displayed_title` or the sidebar render path. |
| 18 (active session not excluded) | Explicit test: focused session passes filter when query matches its name. |
| 19 (theming/a11y) | Storybook check across light / dark / custom themes; assistive-name test. |
| 20 (no false negatives) | Property test: pre-change query set produces the same hit set after the change. |
| 21 (rename UX / telemetry unchanged) | No edits to `finish_tab_rename` ([view.rs:1301-1321](app/src/workspace/view.rs#L1301-L1321)), `set_custom_pane_name` ([view.rs:5218](app/src/workspace/view.rs#L5218)), or `TabRenameEvent` ([events.rs:417-421](app/src/server/telemetry/events.rs#L417-L421)) — assert by file scope. |

Manual validation pass:

1. Three tabs; rename one "deploy", split it into two panes, rename one pane "logs".
2. Command palette: query `deploy` → both pane rows under that tab appear with the leading "deploy" label highlighted. Query `logs` → only that one row, with both labels rendered.
3. Sidebar Panes mode: query `deploy` → all panes under that tab visible. Query `logs` → only that pane.
4. Multi-byte: rename to "デプロイ" and "🚀 prod"; query a partial of each.
5. Quit and relaunch. Re-query — names still findable.

## Risks and mitigations

- **Sidebar gate change scope.** Removing the gate at the two filter sites could leak `title_override` into render call sites if those weren't kept gated. *Mitigation:* the change is scoped to the two filter-only call sites (`matching_tab_indices` line 1022 and `render_groups` line 1547), where the resulting `PaneProps` are throwaway match-only objects consumed by `pane_matches_query`. The render-side `displayed_tab_title_override` at vertical_tabs.rs:1777 (used by `render_tab_group`) is untouched, so PRODUCT invariant 17 holds.
- **Truncation around match for two labels.** Two side-by-side labels with potentially-different match offsets is the trickiest UI piece. *Mitigation:* prototype against the existing `prompt`/`command` truncation behavior; cover with snapshot tests at narrow widths.
- **Per-query rebuild cost.** Adding two char-range computations per session is negligible against the existing per-query Tantivy build. No measurement needed unless a benchmark regression appears.

## Follow-ups

- Pixel-perfect label visual treatment (separator / spacing / weight between tab- and pane-name segments — PRODUCT invariant 12) is a design open question; this spec captures the contract only.
- **Sidebar-header whitespace normalization for tab names.** `PaneGroup::set_title` ([pane_group/mod.rs:4781](app/src/pane_group/mod.rs#L4781)) doesn't trim, while `PaneConfiguration::set_custom_vertical_tabs_title` ([pane/mod.rs:775](app/src/pane_group/pane/mod.rs#L775)) does. A whitespace-only custom tab name renders today as a tab-header containing visible spaces. Pre-existing behavior independent of search; track separately in a new GitHub issue.
