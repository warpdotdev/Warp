# TECH.md — JSON / YAML structured-data block viewer

Issue: https://github.com/warpdotdev/warp/issues/9138
Product spec: `specs/GH9138/product.md`

## Context

Terminal blocks in Warp are rendered as a character grid (ANSI cell matrix)
via `blockgrid_renderer.rs`. There is no semantic parsing of block output today;
all output is raw text. Warp already supports injecting arbitrary views into the
block list through the `RichContent` system, which is how AI blocks, warpify
success blocks, and env-var collection blocks are rendered. This feature uses
that same system to add a structured tree view alongside the existing grid.

Relevant files:

- `app/src/terminal/model/blockgrid.rs` — `BlockGrid::contents_to_string` (line
  ~424) extracts the raw text from a completed block's grid; this is the entry
  point for reading the output text for detection.
- `app/src/terminal/model/blocks.rs` — `BlockList` holds the ordered list of
  blocks; `RichContentItem` (line ~78) represents an injected view with its
  position in the list.
- `app/src/terminal/view/rich_content.rs` — `RichContent` wraps an
  `element_builder: Box<dyn Fn() -> Box<dyn Element>>` and optional
  `RichContentMetadata`. This is the concrete type passed to the blocklist to
  insert a custom view. `RichContentType` (in
  `app/src/terminal/model/rich_content.rs`) enumerates typed rich content; a
  new variant will be added here.
- `app/src/terminal/view.rs` — `TerminalView::on_user_block_completed` (line
  ~9724) and the `ModelEvent::BlockCompleted` handler (line ~10109) are where
  post-completion logic runs. Detection and insertion will hook in here.
- `crates/warp_features/src/lib.rs` — `FeatureFlag` enum; a new flag gates the
  feature during rollout.
- `app/src/settings/` — where the global "Render rich output in blocks" setting
  will be plumbed.

Both `serde_json` (workspace, with `raw_value` feature) and `serde_yaml` (0.8,
workspace) are already declared as dependencies in `app/Cargo.toml`.

## Proposed changes

### 1. Feature flag

Add `JsonYamlBlockViewer` to `FeatureFlag` in
`crates/warp_features/src/lib.rs`. Gate detection and all new UI behind
`FeatureFlag::JsonYamlBlockViewer.is_enabled()`. Add the flag to `DOGFOOD_FLAGS`
for initial internal testing; promote to `PREVIEW_FLAGS` / `RELEASE_FLAGS`
through the normal rollout process.

### 2. Settings

Add a boolean setting `render_rich_block_output: bool` (default `true`) to the
terminal settings struct in `app/src/settings/`. Wire it to a toggle in the
Terminal section of the Settings UI. When the setting is disabled,
`FeatureFlag::JsonYamlBlockViewer.is_enabled()` alone is not sufficient to
trigger detection; both the flag and the setting must be active. (Checking the
setting at the call site rather than inside the feature flag preserves the clean
separation between compile-time gating and user preference.)

**Immediate reversion (Behavior #20):** `TerminalView` subscribes to settings
change events (following the pattern of other settings-reactive views). When
`render_rich_block_output` transitions from `true` to `false`, the handler
clears `block_structured_view_active` (see §7), which causes all blocks to
render their raw grid on the next paint cycle. The `StructuredDataBlock`
`RichContent` items remain in the blocklist and retain their parsed values; they
are simply not rendered while the setting is off. When the setting is re-enabled,
adding a block's `BlockId` back to `block_structured_view_active` restores tree
view instantly without re-parsing.

### 3. Detection module

Create `app/src/terminal/structured_output.rs` with:

```rust
pub enum StructuredOutputKind {
    Json(serde_json::Value),
    Yaml(serde_yaml::Value),
}

/// Takes the canonical block text (ANSI-stripped, PTY-processed output from
/// `BlockGrid::contents_to_string`), trims whitespace, enforces the size cap,
/// and attempts JSON then YAML parsing synchronously.
/// Returns None if neither succeeds, if the input exceeds MAX_DETECT_BYTES,
/// or if the output is a bare YAML scalar.
pub fn detect(canonical_text: &str) -> Option<StructuredOutputKind>
```

`MAX_DETECT_BYTES = 5 * 1024 * 1024` (5 MB, per Behavior #4).

**Canonical text source:** `detect` receives the output of
`BlockGrid::contents_to_string` (which already strips ANSI escape sequences as
part of grid serialization), not the raw PTY byte stream. This is the same
string used by the Copy button and Warp Drive serialization, so the detection
input and the user-visible "raw" text are always identical.

**Timeout and CPU boundedness:** `serde_json::from_str` and
`serde_yaml::from_str` are synchronous and cannot be externally cancelled once
started. The 5 MB size cap is the primary bound on parse time: inputs are
rejected before calling any parser if `canonical_text.len() > MAX_DETECT_BYTES`.
This cap, not a runtime cancellation, is what guarantees bounded CPU work. The
50 ms figure in Behavior #3 is an empirical budget for typical inputs, validated
by the benchmark in the Testing section; it is not enforced via a thread-kill or
`tokio::time::timeout` wrapper, because such a wrapper would not stop the
underlying synchronous parse from running to completion on its thread. If
profiling on CI shows worst-case parse time exceeds 50 ms within the 5 MB cap,
reduce `MAX_DETECT_BYTES` or add a YAML-specific lower cap (e.g., 1 MB) rather
than adding a cancellation mechanism.

**YAML bare-scalar guard (Behavior #2):** after `serde_yaml::from_str`
succeeds, check that the root value is `Mapping` or `Sequence`; reject
`Value::String`, `Value::Number`, `Value::Bool`, `Value::Null`.

### 4. RichContentType variant

Add `StructuredDataBlock` to `RichContentType` in
`app/src/terminal/model/rich_content.rs`. No metadata struct needed initially;
the parsed value is owned by the view model (see §5).

### 5. Tree view

Create `app/src/terminal/view/structured_data_block.rs`. The struct implements
`warpui::View` (the same trait implemented by all WarpUI views; the pattern to
follow is `app/src/terminal/view/plugin_instructions_block.rs`, which is a
plain struct with a `render` method returning a `Box<dyn Element>`). The view
owns:

- `kind: StructuredOutputKind` — the parsed value.
- `raw_text: String` — the original block output, for the Copy button and
  for toggling back to raw.
- `node_states: HashMap<NodePath, NodeState>` — tracks expanded/collapsed state
  per node (`NodePath` is a `Vec<PathSegment>` where a segment is either a
  string key or a usize index).

Rendering:

- Build a recursive `Element` tree using WarpUI `Flex` / `Container` /
  `Text` / `Hoverable` primitives.
- Disclose triangles are `Icon` elements toggling `NodeState` on click via a
  `ViewContext::emit` action.
- Apply theme colors for keys (`theme.syntax_keyword()`), string values
  (`theme.syntax_string()`), and other scalars (`theme.syntax_literal()`).
  If those theme accessors don't exist yet, fall back to `theme.text()` with
  opacity adjustment — do not hard-code colors.
- Keyboard focus tracking via WarpUI's `Focusable` / tab-order mechanism,
  consistent with how other focusable block content works.
- "Copy value" / "Copy path" via `ViewContext::write_to_clipboard`.

### 6. Hook into block completion

In `TerminalView::on_user_block_completed` (`app/src/terminal/view.rs`, line
~9724), after the existing post-completion logic:

```
if FeatureFlag::JsonYamlBlockViewer.is_enabled()
    && settings.render_rich_block_output
    && env_var WARP_RICH_OUTPUT != "0"
{
    let raw = block.contents_to_string(…);
    // Spawn background task (warpui executor::Background) to run detection.
    // On success, dispatch an action that inserts a StructuredDataBlock
    // RichContent at the block's position in the blocklist.
}
```

Use the existing `warpui::async::executor::Background` pattern (already used
in `app/src/terminal/model/blocks.rs` and `view.rs`) so detection does not
block the UI thread. The 50 ms bound from Behavior #3 is enforced by the caller:
if the background future has not resolved within 50 ms, cancel it and leave the
block as raw text.

The `RichContent` is inserted with `RichContentInsertionPosition::BeforeBlockIndex`
at the index just after the completed block, so it visually replaces the block's
grid in the list. The raw grid is still present and rendered when the user
toggles to raw view (§7).

### 7. Toggle button

Add a `ToggleStructuredView(BlockId)` action to
`app/src/terminal/model/block.rs`'s action enum (or the appropriate terminal
action enum). Handle it in `TerminalView` by toggling a
`block_structured_view_active: HashSet<BlockId>` field on the view. When the
block is in the set, the `StructuredDataBlock` `RichContent` is visible; when
not, the block's normal grid element is shown instead and the `RichContent` is
hidden (not removed, so toggling back is instant).

The toggle button is rendered in the block hover toolbar by extending the
existing hover-toolbar rendering in `app/src/terminal/block_list_element.rs`,
following the pattern of the existing Copy / Share buttons.

### 8. `WARP_RICH_OUTPUT` env-var

Read `WARP_RICH_OUTPUT` from the block's environment snapshot at completion
time. The block model already exposes environment metadata for related features;
use the same mechanism. If the var is absent or set to anything other than `0`,
detection proceeds normally.

## Testing and validation

**Unit tests** (in `app/src/terminal/structured_output.rs` or a sibling
`_tests.rs`):

- Behavior #1/#2: `detect("{\"key\": 1}")` returns `Some(Json(_))`;
  `detect("key: value\n")` returns `Some(Yaml(_))`;
  `detect("42")` returns `None` (bare scalar);
  `detect("not json or yaml [[[")` returns `None`.
- Behavior #4: `detect(&"x".repeat(MAX_DETECT_BYTES + 1))` returns `None`.
- Behavior #5 (env var) is validated at the call-site level in view tests.
- Behavior #22: `detect("[ 1, 2, 3 ]")` returns `Some(Json(Array(_)))` and
  renders with index keys `[0]`, `[1]`, `[2]`.
- Behavior #23: `detect("{\"incomplete\":")` returns `None`.

**Integration tests** (in `crates/integration/`):

- Run a command whose output is a known JSON blob; assert the resulting block
  has a `StructuredDataBlock` `RichContent` inserted.
- Run the same command with `WARP_RICH_OUTPUT=0`; assert no `RichContent` is
  inserted.
- Toggle from tree view to raw and back; assert the block's display mode flips
  correctly.

**Behavior-to-verification mapping:**

- #3 (50 ms bound): unit test with a synthetic 4.9 MB JSON blob measuring
  wall-clock time of `detect()`; must complete under 50 ms on CI.
- #9 (initial expand depth): inspect the rendered element tree to confirm the
  top two levels are `NodeState::Expanded` and deeper levels are
  `NodeState::Collapsed`.
- #14/#15/#16 (toggle): integration test covers toggle state flip and hover
  visibility.
- #17/#18 (copy context menu): manual verification — right-click leaf and
  non-leaf nodes, confirm clipboard contents match spec.
- #19 (Copy button copies raw): assert `ClipboardContent` equals
  `block.raw_text` regardless of tree-view state.
- #20 (Settings toggle): toggle the setting off; assert subsequent block
  completions do not trigger detection.
- #25 (Warp Drive): existing block-serialization tests; confirm serialized form
  equals `block.raw_text`, not the tree rendering.
- #26 (resize reflow): integration test that resizes the terminal window after
  detection and asserts no panic and no content loss.

## Risks and mitigations

- **Performance:** `serde_yaml` is known to be slow on large inputs. The 5 MB
  cap and 50 ms timeout in detection mitigate this. If benchmarks show YAML
  detection is too slow even under the cap, reduce the YAML-specific cap to 1 MB
  as a follow-up.
- **False-positive YAML detection:** many command outputs that are not YAML
  happen to parse as valid YAML scalars or single-key mappings. The bare-scalar
  guard (Behavior #2) eliminates the most common false positive. If user reports
  surface other false positives, tighten the heuristic (e.g., require at least
  two keys, or require a newline in the output).
- **TerminalModel lock contention:** `contents_to_string` acquires the terminal
  model lock. Do not hold the lock across the detection future; copy the string
  out first, then release the lock before spawning the background task.

## Follow-ups

- Streaming JSON: render a "detecting…" placeholder while the command is still
  running; transition to tree view at block completion. Tracked as a follow-up
  to this issue.
- TOML detection and rendering (also mentioned in #9138 issue body).
- Image block and table block rendering are tracked separately in #9138 and are
  not addressed by this spec.
- Remove `FeatureFlag::JsonYamlBlockViewer` and the flag guard after stable
  rollout.
