---
name: 01 — Tab color shortcuts
status: draft
---

# Tab color shortcuts — TECH

See `roadmap/01-tab-colors/PRODUCT.md` for behavior.

## Context

The tab-color rendering and storage stack already exists upstream and is intact in this checkout. This feature only adds the keyboard surface on top.

Existing pieces (do not rebuild):

- `app/src/tab.rs:79` — `SelectedTabColor { Unset, Color(AnsiColorIdentifier), Cleared }`. Per-tab on `TabData::selected_color`.
- `crates/warp_core/src/ui/theme/mod.rs:539` — `AnsiColorIdentifier` enum with the eight ANSI colors. The product spec's color set is fixed to this palette (see PRODUCT invariant 1's open question).
- `app/src/workspace/action.rs:210` — `WorkspaceAction::ToggleTabColor { color, tab_index }`. Used by the existing tab context menu. Toggle semantics: same color → clear, different color → set.
- `app/src/workspace/view.rs:5068` — `WorkspaceView::toggle_tab_color`, the handler for the above. Already chooses between `SelectedTabColor::Cleared` and `SelectedTabColor::Unset` based on `FeatureFlag::DirectoryTabColors`. Already emits `TabTelemetryAction::SetColor` / `ResetColor`.
- `app/src/persistence/sqlite.rs:899` — `selected_color` is serialized to and restored from the workspace sqlite store. PRODUCT invariant 9 requires no change here.
- `app/src/workspace/mod.rs (~150-560)` — workspace `EditableBinding` registry. New default keybindings are registered alongside existing ones like `workspace:close_active_tab`, `workspace:toggle_navigation_palette`, etc., grouped via `.with_group(bindings::BindingGroup::Navigation.as_str())`. Tab-related bindings already live in this group (see lines ~494-578).
- `crates/warpui_core/src/keymap.rs:645` — `EditableBinding` builder API: `with_mac_key_binding`, `with_linux_or_windows_key_binding`, `with_group`, `with_context_predicate` are the methods we use. There is no `Tabs` group; `Navigation` is the established home for tab-related defaults.

Upstream branch `oz-agent/APP-4321-active-tab-color-indication` (commit `86570e7`) is unrelated polish for already-color-coded tabs and is not merged. We do not depend on or cherry-pick it for this feature.

## Proposed changes

### New action variants

Add two variants to `WorkspaceAction` in `app/src/workspace/action.rs`:

```rust
SetActiveTabColor { color: AnsiColorIdentifier },
ResetActiveTabColor,
```

Place them next to `ToggleTabColor` (line 210). They take no `tab_index` — the handler resolves the active tab. This matches PRODUCT invariant 4 (always acts on the active tab) and keeps the keybinding payload minimal. Add them to whatever exhaustive-match arms exist for `WorkspaceAction` (e.g. the persistability filter at line 730 — match the surrounding pattern).

Keep `ToggleTabColor` unchanged. The context menu's existing toggle UX is correct for that surface; do not change it.

### New handler methods

In `app/src/workspace/view.rs`, alongside `toggle_tab_color` (line 5068):

- `set_tab_color(&mut self, index: usize, color: AnsiColorIdentifier, ctx: &mut ViewContext<Self>)` — unconditional set. If `self.tabs[index].color() == Some(color)`, return without notifying (PRODUCT invariant 2: visually unchanged, no toggle-off). Otherwise set `self.tabs[index].selected_color = SelectedTabColor::Color(color)`, emit `TabTelemetryAction::SetColor`, and `ctx.notify()`.
- `reset_tab_color(&mut self, index: usize, ctx: &mut ViewContext<Self>)` — unconditional reset. If the tab is already uncolored (`self.tabs[index].selected_color` is `Unset` and there's no Cleared override needed), return without notifying (PRODUCT invariant 3, last bullet). Otherwise set to `SelectedTabColor::Cleared` if `FeatureFlag::DirectoryTabColors` is enabled, else `Unset` — same branch the existing `toggle_tab_color` already uses. Emit `TabTelemetryAction::ResetColor` and `ctx.notify()`.

Bounds-check the index identically to `toggle_tab_color` and `log::warn!` on miss. Both methods are pub.

Add a thin "active tab" wrapper used by the action handler — e.g. `set_active_tab_color(&mut self, color, ctx)` and `reset_active_tab_color(&mut self, ctx)` — that resolves the active tab index and delegates. If there's an existing helper like `active_tab_index()`, use it; otherwise read whatever field the rest of the workspace view reads. PRODUCT invariant 11: a missing/zero-tab state is a no-op.

### Action dispatch

In the `WorkspaceAction` match arm in `app/src/workspace/view.rs` (line ~20019, alongside the existing `ToggleTabColor` arm), add:

```rust
SetActiveTabColor { color } => self.set_active_tab_color(*color, ctx),
ResetActiveTabColor => self.reset_active_tab_color(ctx),
```

### Default keybindings

In `app/src/workspace/mod.rs`, add nine `EditableBinding` registrations near the existing tab/navigation bindings (around lines 494-578). Naming convention follows `workspace:<verb>_<noun>`:

```rust
EditableBinding::new(
    "workspace:set_active_tab_color_red",
    "Set active tab color: Red",
    WorkspaceAction::SetActiveTabColor { color: AnsiColorIdentifier::Red },
)
.with_group(bindings::BindingGroup::Navigation.as_str())
.with_context_predicate(id!("Workspace"))
.with_mac_key_binding("cmd-alt-1")
.with_linux_or_windows_key_binding("ctrl-alt-1"),
```

Repeat for the eight colors per the PRODUCT.md table (⌘⌥1=Red, ⌘⌥2=Yellow, ⌘⌥3=Green, ⌘⌥4=Cyan, ⌘⌥5=Blue, ⌘⌥6=Magenta, ⌘⌥7=White, ⌘⌥8=Black) and `workspace:reset_active_tab_color` on `cmd-alt-0` / `ctrl-alt-0`. Use the same `Workspace` context predicate the surrounding tab bindings use, so the shortcuts respect PRODUCT invariant 10 (focus rules — modals/palette/etc. that capture keys keep capturing them).

The Linux/Windows mappings use `ctrl-alt-N` to mirror the Mac `cmd-alt-N`, matching the convention used for the other ⌘⌥-style bindings in this file. If the existing tab-switch bindings use a different cross-platform convention (verify against neighbours during implementation), follow theirs instead.

No new entries in `BindingGroup`. Tab-related bindings already use `Navigation`; reuse it. PRODUCT invariant 6 only requires that the entries appear and are rebindable in the keybindings settings page — `EditableBinding` registration alone gives us that.

### Persistence

No change. `selected_color` already round-trips through sqlite (PRODUCT invariant 9 satisfied by existing code).

### Feature flag

None. PRODUCT invariant 12 requires the feature ships unconditionally.

## Testing and validation

Unit tests, in the existing test module for `WorkspaceView`:

- `set_tab_color` on an uncolored tab → `selected_color` becomes `Color(c)`. Maps to PRODUCT invariant 2 first half.
- `set_tab_color` on a tab already that color → no change (assert `ctx.notify()` not called, or just assert the field stays `Color(c)`). Maps to PRODUCT invariant 2 second half (no toggle-off).
- `set_tab_color` on a tab with a different color → `selected_color` becomes the new `Color(c)`. Maps to PRODUCT invariant 2.
- `reset_tab_color` on a colored tab with `DirectoryTabColors` disabled → `Unset`. Maps to PRODUCT invariant 3.
- `reset_tab_color` on a colored tab with `DirectoryTabColors` enabled → `Cleared`. Maps to PRODUCT invariant 3 first bullet.
- `reset_tab_color` on an already-uncolored tab → no-op. Maps to PRODUCT invariant 3 last bullet.
- Bounds: `set_tab_color`/`reset_tab_color` on an out-of-range index logs and returns. Mirrors the existing `toggle_tab_color` test if there is one.

Active-tab routing test:
- `set_active_tab_color`/`reset_active_tab_color` mutate only the active tab; siblings untouched. Maps to PRODUCT invariant 4.
- Zero-tab state is a no-op. Maps to PRODUCT invariant 11.

No integration test for the keybinding routing layer — `EditableBinding` registration is config-shaped and the keymap system has its own coverage.

Manual validation: the smoke test in `PRODUCT.md` is the canonical pre-merge check. Run it against a `cargo run` build before requesting review. Steps 9 (settings page entries appear and are rebindable) and 10 (persistence across restart) cover the parts unit tests can't.
