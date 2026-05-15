# TECH.md - Add optional confirmation before closing tabs

Issue: https://github.com/warpdotdev/warp/issues/10995
Product spec: `specs/GH10995/product.md`

## Context

Tab close entry points already converge on workspace actions:

- `app/src/tab.rs:492-515` builds horizontal tab context-menu close actions for
  "Close tab", "Close other tabs", and "Close Tabs to the Right".
- `app/src/tab.rs:1175-1180` dispatches `WorkspaceAction::CloseTab(tab_index)`
  from the horizontal tab close button.
- `app/src/tab.rs:1778-1781` dispatches `WorkspaceAction::CloseTab(tab_index)`
  from horizontal tab middle-click.
- `app/src/workspace/view/vertical_tabs.rs:2119-2124` mirrors middle-click close
  for vertical tabs.
- `app/src/workspace/view/vertical_tabs.rs:2216-2234` dispatches
  `WorkspaceAction::CloseTab(tab_index)` from the vertical tab close button.
- `app/src/workspace/view.rs:20650-20658` handles `CloseTab`,
  `CloseActiveTab`, `CloseOtherTabs`, `CloseNonActiveTabs`, `CloseTabsRight`,
  and `CloseTabsRightActiveTab` by calling `close_tab`, `close_other_tabs`, or
  `close_tabs_direction`.

The actual close logic is centralized in `Workspace::close_tabs`:

- `app/src/workspace/view.rs:10418-10517` collects the tab indices, checks
  existing confirmations, cancels tab rename state, and removes tabs in reverse
  order.
- `app/src/workspace/view.rs:10428-10438` shows the existing shared-session
  confirmation when a shared tab is being closed.
- `app/src/workspace/view.rs:10440-10505` builds and shows the existing
  running-process / unsaved-state quit warning.
- `app/src/workspace/view.rs:10521-10539` treats last-tab close as a window-close
  case by passing `skip_confirmation || is_last_tab`.
- `app/src/workspace/view.rs:10555-10605` routes bulk close actions through the
  same `close_tabs` helper.

Relevant existing UI/settings references:

- `app/src/workspace/tab_settings.rs:445-546` defines `TabSettings`, including
  tab appearance settings under `appearance.tabs.*`.
- `app/src/settings_view/appearance_page.rs:1370-1409` renders the Appearance >
  Tabs settings category.
- `app/src/workspace/close_session_confirmation_dialog.rs:25-43` already has an
  `OpenDialogSource` enum that identifies the close source.
- `app/src/workspace/close_session_confirmation_dialog.rs:93-148` is specific to
  shared sessions and should not be reused for ordinary tab-close copy.
- `app/src/quit_warning/mod.rs:39-68` models the existing warning dialog for
  running processes, shared sessions, and unsaved state.

## Proposed Changes

### 1. Add a tab setting

Add a boolean setting to `TabSettings`:

- Rust field: `confirm_before_closing_tabs`
- Generated setting type: `ConfirmBeforeClosingTabs`
- Default: `false`
- Supported platforms: `SupportedPlatforms::ALL`
- Sync behavior: `SyncToCloud::Globally(RespectUserSyncSetting::Yes)`
- TOML path: `appearance.tabs.confirm_before_closing_tabs`
- Description: "Whether to ask for confirmation before closing tabs."

This keeps the setting next to existing tab appearance preferences and preserves
the default behavior for all existing users.

Add unit coverage in `app/src/workspace/tab_settings_tests.rs` for:

- default value is false
- TOML path is `appearance.tabs.confirm_before_closing_tabs`
- hierarchy is `appearance.tabs`
- TOML key is `confirm_before_closing_tabs`

### 2. Add the Settings UI row

Update `app/src/settings_view/appearance_page.rs`:

- Add `AppearancePageAction::ToggleConfirmBeforeClosingTabs`.
- Add `toggle_confirm_before_closing_tabs`.
- Add `ConfirmBeforeClosingTabsWidget`.
- Place the widget in the existing "Tabs" category near the close-button related
  settings, after "Show tab indicators" or near "Tab close button position".
- Label: "Confirm before closing tabs".
- Optional description: "Ask before closing tabs from buttons, shortcuts, and
  tab menus."
- Search terms: "confirm close tab closing tabs accidental".

Add a command-palette toggle through `init_actions_from_parent_view`, matching
other Appearance > Tabs toggles. Suggested visible action text: "confirm before
closing tabs".

Telemetry is optional. If a new telemetry event is not added, make sure existing
`TabOperations` telemetry continues to emit only after tabs actually close, not
when the confirmation dialog is shown.

### 3. Add a general tab-close confirmation dialog

Do not reuse `CloseSessionConfirmationDialog`, because its title, body, primary
button, and "Don't show again" checkbox are shared-session-specific.

Preferred implementation:

- Add a small general tab-close confirmation helper in workspace close logic,
  using the same modal/callback infrastructure as existing warnings.
- The helper receives:
  - `OpenDialogSource`
  - the tab indices that would close
  - `add_to_undo_stack`
  - whether this is a single-tab or bulk close
- Single-tab copy:
  - title: "Close tab?"
  - body: "This tab will be closed."
  - confirm: "Close tab"
  - cancel: "Cancel"
- Bulk copy:
  - title: `Close {count} tabs?`
  - body: "These tabs will be closed."
  - confirm: "Close tabs"
  - cancel: "Cancel"

The dialog should not include a "Don't show again" checkbox in the initial
version. The setting is already the explicit opt-in/out control.

### 4. Preserve confirmation precedence

Update `Workspace::close_tabs` so the order is:

1. Last-tab/window-close handling remains owned by `close_tab`.
2. Existing shared-session confirmation.
3. Existing running-process / unsaved-state warning.
4. New general tab-close confirmation, only when
   `TabSettings::as_ref(ctx).confirm_before_closing_tabs` is true.
5. Rename cancellation and tab removal.

The general confirmation must only appear after higher-severity warnings have
decided they do not need to show. This avoids double dialogs and preserves
existing safety semantics.

### 5. Avoid weakening warning checks after the dialog opens

The existing `skip_confirmation: bool` means "skip all confirmation checks". Do
not use that flag blindly when confirming the new general dialog, because risk
state can change while the dialog is open.

Recommended refactor:

- Replace or supplement `skip_confirmation: bool` with a small enum, for
  example:

  ```rust
  enum CloseTabsConfirmationMode {
      Normal,
      SkipAll,
      SkipGeneralTabClose,
  }
  ```

- Existing shared-session and quit-warning confirm callbacks can continue to use
  `SkipAll`, preserving today's behavior.
- The new general dialog confirm callback should use `SkipGeneralTabClose`, so
  it re-runs shared-session and running-process / unsaved-state checks before
  closing, while avoiding an infinite loop back into the same general dialog.

If the implementation keeps the boolean API, add a second explicit
`skip_general_tab_close_confirmation` parameter instead. The important invariant
is that confirming the new general dialog must not bypass higher-severity
warnings if those warnings become relevant before the actual close happens.

### 6. Bulk close behavior

Keep the current `tab_indices_vec` collection in `close_tabs`. Use its length to
decide whether to show single or bulk copy.

- If `tab_indices_vec.is_empty()`, return `true` without showing the general
  dialog.
- If the length is 1, use single-tab copy.
- If the length is greater than 1, show one bulk dialog for all tabs.
- On confirm, close the same set of tab indices using the existing reverse-order
  removal behavior.

### 7. Tests

Update or add workspace tests in `app/src/workspace/view_tests.rs`:

- setting off: `WorkspaceAction::CloseTab` closes immediately, matching current
  behavior
- setting on: `WorkspaceAction::CloseTab` opens the general confirmation and
  does not close immediately
- cancel: leaves tab count, active tab, rename state, and undo stack unchanged
- confirm: closes the intended tab and preserves existing undo behavior
- bulk close: `CloseOtherTabs` shows one dialog and closes all intended tabs only
  after confirm
- higher-severity precedence: a shared-session tab shows the existing
  close-session confirmation, not the new general confirmation
- higher-severity precedence: tabs with running processes / unsaved state show
  the existing quit warning, not the new general confirmation
- last-tab close: does not show the new general tab-close confirmation
- confirm callback re-checks higher-severity warnings if relevant state changes
  before final close

Add settings tests in `app/src/workspace/tab_settings_tests.rs` as described in
section 1.

If the modal is difficult to assert directly in `view_tests`, factor the dialog
decision into a small pure helper and unit-test the helper, then keep a focused
integration-style workspace test for the end-to-end close flow.

### 8. Manual validation

Run the app with `./script/run` and verify:

- setting off: close button and keybinding close tabs immediately
- setting on: close button, middle-click, keyboard close, and tab context-menu
  close show the confirmation
- "Cancel" keeps tabs unchanged
- "Close tab" / "Close tabs" closes the intended tabs
- vertical tabs use the same behavior
- shared-session and running-process warnings still take precedence and are not
  followed by a second generic close confirmation

### 9. Presubmit

Run:

```bash
cargo fmt
cargo clippy --workspace --all-targets --all-features --tests -- -D warnings
cargo nextest run -p warp_app --no-fail-fast
```

For a spec-only PR, `cargo fmt` plus markdown/diff checks may be sufficient if
maintainers do not require the full Rust validation before implementation. The
implementation PR should run the full presubmit set.
