# Tab Context Menu Copy Metadata — Tech Spec
Product spec: `specs/tab-context-copy-metadata/PRODUCT.md`
## Problem
`TabData::menu_items_with_pane_name_target` builds the tab right-click menu in `app/src/tab.rs`. It currently groups session sharing, tab modification, close actions, tab-config saving, and color options. The metadata needed for copy actions already exists on `PaneGroup` and `TerminalView`, but the menu does not expose it.
## Relevant Code
- `app/src/tab.rs` — tab context menu construction.
- `app/src/workspace/action.rs` — `WorkspaceAction::CopyTextToClipboard(String)` already exists.
- `app/src/workspace/view.rs` — `CopyTextToClipboard` writes plain text to the clipboard.
- `app/src/terminal/view/tab_metadata.rs` — `TerminalView` helpers for display working directory, terminal title, branch, PR URL, and diff stats.
- `app/src/pane_group/mod.rs` — `PaneGroup::display_title`, `custom_title`, `focused_session_view`, and `terminal_view_from_pane_id`.
- `app/src/workspace/view/vertical_tabs.rs` — current vertical-tabs metadata rendering, including pane-targeted context menu behavior.
## Current State
The tab context menu is assembled from section methods that return `Vec<MenuItem<WorkspaceAction>>`. Separators are inserted between non-empty sections by `menu_items_with_pane_name_target`.
`WorkspaceAction::CopyTextToClipboard(String)` is already handled in `Workspace::handle_action` and writes text to the system clipboard. Using this existing action avoids adding new workspace actions for each metadata type.
Vertical-tabs pane context menus pass a `PaneNameMenuTarget` with a `PaneViewLocator`. This locator can be reused to resolve terminal metadata for the clicked or active pane. Regular tab context menus have no pane target, so they should use the focused terminal session in the tab.
## Changes
### 1. Add a copy metadata menu section
Add a new `copy_metadata_menu_items` section to `TabData`. Insert it after session-sharing items and before tab-modification items so copy actions are grouped near other share/copy actions.
The section appends:
- `Copy branch` when `TerminalView::current_git_branch(ctx)` is non-empty.
- `Copy tab title` when `PaneGroup::display_title(ctx)` is non-empty.
- `Copy current working directory` when the selected terminal has a non-empty `pwd()`, falling back to `display_working_directory(ctx)` if needed.
- `Copy PR link` when `TerminalView::current_pull_request_url(ctx)` is non-empty.
Each item dispatches `WorkspaceAction::CopyTextToClipboard(value)`.
### 2. Resolve terminal metadata from the correct target
When `pane_name_target` is present and belongs to this tab's `PaneGroup`, use `PaneGroup::terminal_view_from_pane_id(target.locator.pane_id, ctx)`.
Otherwise, use `PaneGroup::focused_session_view(ctx)`.
If no terminal view is available, omit terminal-specific items but still show `Copy tab title` when the tab has a display title.
### 3. Keep metadata values clean
Filter all values through a small helper that trims whitespace for availability checks and stores the original non-empty string for copying. This prevents blank menu rows while preserving the copied value.
## Risks and Mitigations
**Menu noise:** Copy tab title may appear broadly because most tabs have a display title. This is intentional because title copying is useful and the value is known.
**Stale metadata:** Branch, CWD, and PR link are copied from already-known `TerminalView` state. The change does not perform new synchronous GitHub or filesystem lookups, so it preserves menu responsiveness.
**Pane targeting:** In vertical-tabs pane mode, terminal metadata should come from the clicked or active pane. The locator check avoids accidentally reading metadata from a pane in a different tab.
## Testing and Validation
- Run `cargo fmt`.
- Run a targeted compile check for the app crate or workspace slice affected by `app/src/tab.rs`.
- Manual checks:
  - Git terminal with branch metadata shows Copy branch.
  - Terminal with a PR chip shows Copy PR link.
  - Terminal without git metadata omits branch and PR items.
  - Non-terminal or metadata-less contexts still omit terminal-only copy items.
