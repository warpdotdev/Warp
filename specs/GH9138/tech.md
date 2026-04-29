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

Detection runs only for blocks completed in the current session. Session restore
and shared blocks always render as raw text on load; re-detecting at restore
time is deferred as a follow-up (see product spec Non-goals and Follow-ups).

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
trigger detection; both the flag and the setting must be active.

**Immediate reversion (Behavior #20):** `TerminalView` subscribes to settings
change events (following the pattern of other settings-reactive views). When
`render_rich_block_output` transitions from `true` to `false`, the handler
clears `block_structured_view_active` (see §7), which causes all blocks to
render their raw grid on the next paint cycle. The `StructuredDataBlock`
`RichContent` items remain in the blocklist and retain their parsed values; they
are simply not rendered while the setting is off. When the setting is re-enabled,
the handler re-populates `block_structured_view_active` with the `BlockId`s of
all `StructuredDataBlock` `RichContent` items currently in the blocklist,
restoring tree view for previously-detected blocks without re-parsing.

### 3. Detection module

Create `app/src/terminal/structured_output.rs` with:

```rust
pub enum StructuredOutputKind {
    Json(serde_json::Value),
    Yaml(serde_yaml::Value),
}

/// Takes the canonical block text (ANSI-stripped, PTY-processed output from
/// `BlockGrid::contents_to_string`), trims whitespace, enforces size and
/// structure caps, and attempts JSON then YAML parsing synchronously.
/// Returns None if neither succeeds, if input exceeds MAX_DETECT_BYTES,
/// if the parsed structure exceeds MAX_NODES or MAX_DEPTH, or if the
/// output is a bare YAML scalar.
pub fn detect(canonical_text: &str) -> Option<StructuredOutputKind>
```

**Limits:**
- `MAX_DETECT_BYTES = 5 * 1024 * 1024` — input size cap (Behavior #4). Checked
  before any parsing.
- `MAX_NODES = 10_000` — total node count across the parsed tree (Behavior #4a).
  Counted with a post-parse walk; if exceeded, return `None`.
- `MAX_DEPTH = 50` — maximum nesting level (Behavior #4a). Checked during the
  post-parse walk.

**Canonical text source:** `detect` receives the output of
`BlockGrid::contents_to_string` (which already strips ANSI escape sequences as
part of grid serialization), not the raw PTY byte stream. This is the same
string used by the Copy button and Warp Drive serialization, so the detection
input and the user-visible "raw" text are always identical.

**CPU boundedness:** `serde_json::from_str` and `serde_yaml::from_str` are
synchronous and cannot be externally cancelled once started. CPU work is bounded
by the `MAX_DETECT_BYTES` check before parse and the `MAX_NODES` / `MAX_DEPTH`
walk after parse. There is no `tokio::time::timeout` wrapper because it would
not interrupt a synchronous parse already running on a thread. If CI benchmarks
show worst-case parse time is unacceptable, reduce `MAX_DETECT_BYTES` or add a
YAML-specific lower cap rather than adding a cancellation mechanism.

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
- `raw_text: String` — the canonical block output, used by the Copy button and
  for Warp Drive / AI context (see Behavior #19, #25).
- `node_states: HashMap<NodePath, NodeState>` — tracks expanded/collapsed state
  per node (`NodePath` is a `Vec<PathSegment>` where a segment is either a
  string key or a usize index).

Rendering:

- Build a recursive `Element` tree using WarpUI `Flex` / `Container` /
  `Text` / `Hoverable` primitives.
- Disclose triangles are `Icon` elements toggling `NodeState` on click via a
  `ViewContext::emit` action.
- Apply theme colors for keys, string values, and other scalars using theme
  accessors. If the relevant accessors don't exist yet, fall back to
  `theme.text()` with opacity adjustment — do not hard-code colors.
- Keyboard focus tracking via WarpUI's focus mechanism, consistent with how
  other focusable block content works.
- "Copy value" / "Copy path" via `ViewContext::write_to_clipboard`.

### 6. Hook into block completion

In `TerminalView::on_user_block_completed` (`app/src/terminal/view.rs`, line
~9724), after the existing post-completion logic:

```
if FeatureFlag::JsonYamlBlockViewer.is_enabled()
    && settings.render_rich_block_output
    && !warp_rich_output_suppressed(block)
{
    let raw = block.contents_to_string(…);
    // Spawn background task (warpui executor::Background) to run detect().
    // On Some(_), dispatch an action that inserts a StructuredDataBlock
    // RichContent at the block's position in the blocklist and adds the
    // BlockId to block_structured_view_active.
}
```

Use the existing `warpui::async::executor::Background` pattern so detection
does not block the UI thread.

The `RichContent` is inserted with `RichContentInsertionPosition::BeforeBlockIndex`
at the index just after the completed block, so it visually replaces the block's
grid in the list. The raw grid is still present and rendered when the user
toggles to raw view (§7).

**`WARP_RICH_OUTPUT` check (`warp_rich_output_suppressed`):** The helper reads
`WARP_RICH_OUTPUT` from the completed block's captured environment if the block
model exposes a per-block environment snapshot. If no such API exists in the
current model, fall back to reading the variable from the current process
environment at detection time (i.e., whatever the shell exported at session
start). This fallback is acceptable for a first implementation; a follow-up can
wire up per-block env snapshots if the fallback proves insufficient.

### 7. Toggle button

Add a `ToggleStructuredView(BlockId)` action to the terminal action enum.
Handle it in `TerminalView` by toggling a
`block_structured_view_active: HashSet<BlockId>` field on the view. When the
block is in the set, the `StructuredDataBlock` `RichContent` is visible; when
not, the block's normal grid element is shown and the `RichContent` is hidden
(not removed, so toggling back is instant without re-parsing).

The toggle button is rendered in the block hover toolbar by extending the
existing hover-toolbar rendering in `app/src/terminal/block_list_element.rs`,
following the pattern of the existing Copy / Share buttons.

## Testing and validation

**Unit tests** (in `app/src/terminal/structured_output.rs` or a sibling
`_tests.rs`):

- Behavior #1/#2: `detect("{\"key\": 1}")` returns `Some(Json(_))`;
  `detect("key: value\n")` returns `Some(Yaml(_))`;
  `detect("42")` returns `None` (bare scalar);
  `detect("not json or yaml [[[")` returns `None`.
- Behavior #4: `detect(&"x".repeat(MAX_DETECT_BYTES + 1))` returns `None`.
- Behavior #4a (node count): synthesize a flat JSON object with `MAX_NODES + 1`
  keys within the byte cap; assert `detect` returns `None`.
- Behavior #4a (depth): synthesize a JSON object nested `MAX_DEPTH + 1` levels
  within the byte cap; assert `detect` returns `None`.
- Behavior #22: `detect("[ 1, 2, 3 ]")` returns `Some(Json(Array(_)))`.
- Behavior #23: `detect("{\"incomplete\":")` returns `None`.

**Integration tests** (in `crates/integration/`):

- Run a command whose output is a known JSON blob; assert the resulting block
  has a `StructuredDataBlock` `RichContent` inserted.
- Run the same command with `WARP_RICH_OUTPUT=0` in the environment; assert no
  `RichContent` is inserted.
- Toggle from tree view to raw and back; assert the block's display mode flips.
- Disable the setting; assert currently-active tree-view blocks revert to raw
  and `block_structured_view_active` is empty. Re-enable; assert previously-
  detected blocks return to tree view without re-parsing.

**Behavior-to-verification mapping:**

- #3 (background detection, no UI block): integration test that completes a
  JSON-output command and asserts the UI remains responsive during detection.
- #9 (initial expand depth): inspect rendered element tree to confirm top two
  levels are `NodeState::Expanded`, deeper levels `NodeState::Collapsed`.
- #14/#16 (toggle button visibility): integration test asserts button appears
  on hover and flips display mode.
- #15 (restore = raw text): session-restore test asserts no tree view is shown
  for previously-detected blocks after reload.
- #17/#18 (copy context menu): manual verification — right-click leaf and
  non-leaf nodes, confirm clipboard contents match spec.
- #19 (Copy button copies raw): assert `ClipboardContent` equals `raw_text`
  regardless of tree-view state.
- #20 (settings toggle + reversion): covered by integration test above.
- #25 (shared/restored blocks = raw): block-serialization test confirms
  serialized form equals `raw_text`; no `StructuredDataBlock` state is
  serialized.
- #26 (resize reflow): integration test resizes window after detection; asserts
  no panic and no content loss.

## Risks and mitigations

- **Performance:** `serde_yaml` is slow on large inputs. The `MAX_DETECT_BYTES`
  cap and `MAX_NODES` / `MAX_DEPTH` post-parse walk are the primary mitigations.
  If CI benchmarks show YAML detection is still slow at the cap, reduce the
  YAML-specific cap to 1 MB as a follow-up.
- **False-positive YAML detection:** many command outputs parse as valid YAML
  scalars or single-key mappings. The bare-scalar guard eliminates the most
  common false positive. If reports surface others, tighten the heuristic.
- **TerminalModel lock contention:** `contents_to_string` acquires the terminal
  model lock. Copy the string out first, then release the lock before spawning
  the background task.
- **`WARP_RICH_OUTPUT` env-var fallback:** the process-level fallback reads
  whatever the shell exported at session start, which may not reflect per-command
  overrides like `WARP_RICH_OUTPUT=0 my-command`. This is a known limitation of
  the fallback; document it in code and address in a follow-up.

## Follow-ups

- Per-block `WARP_RICH_OUTPUT` env snapshot, if the block model doesn't
  currently expose one.
- Re-detection at session restore and for shared blocks (product spec Non-goals).
- Streaming JSON: placeholder during command execution, tree at block completion.
- TOML detection and rendering.
- Image and table block rendering (separate from this spec).
- Remove `FeatureFlag::JsonYamlBlockViewer` after stable rollout.
