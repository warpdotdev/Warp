# Tab Context Menu Copy Metadata — Product Spec
## Summary
Add copy actions to the tab context menu for metadata Warp already knows about the tab or active pane: branch name, tab title, current working directory, and PR link.
## Problem
Vertical tabs surface useful metadata such as branch, working directory, and PR link, but users must manually select or rederive that information when they want to share it elsewhere. The attached tab context menu is the natural place to expose quick copy actions because it is already where users manage tabs and active panes.
## Goals
- Show copy actions for branch, tab title, current working directory, and PR link when the relevant metadata exists.
- Avoid disabled or empty menu items; if Warp does not have a value, omit the corresponding copy action.
- Use the same metadata sources that vertical tabs already use, so menu availability matches what Warp knows about the tab or pane.
- Support both regular tab context menus and vertical-tabs pane context menus.
## Non-goals
- Fetch branch or PR data synchronously when metadata is not already available.
- Add new settings or feature flags for the copy actions.
- Change how tab titles, branch labels, working directories, or PR badges are rendered in vertical tabs.
- Add toast notifications for these copy actions.
## User Experience
When a user opens the tab context menu, Warp includes a copy metadata section if at least one copyable metadata value is available.
Available actions:
- Copy branch
- Copy tab title
- Copy current working directory
- Copy PR link
The section appears only when one or more of those actions are present. Each item copies the corresponding raw value to the system clipboard. The menu keeps the existing separator behavior, so the copy metadata section is visually grouped with the rest of the menu.
For vertical-tabs pane context menus, terminal-specific metadata comes from the pane represented by the context menu target. For regular tab context menus, terminal-specific metadata comes from the focused terminal session in the tab. Tab title comes from the tab-level display title.
## Success Criteria
1. A tab with a known branch shows Copy branch, and selecting it copies the branch name.
2. A tab with a non-empty display title shows Copy tab title, and selecting it copies that title.
3. A terminal tab with a known current working directory shows Copy current working directory, and selecting it copies the directory.
4. A terminal tab with a known PR URL shows Copy PR link, and selecting it copies the URL.
5. Copy actions are omitted individually when their metadata is unavailable or empty.
6. Existing context menu actions and separators continue to behave as before.
## Validation
- Manually verify the menu on a terminal in a git repository with and without a PR chip.
- Manually verify the menu on a terminal outside a git repository.
- Manually verify the vertical-tabs active-pane context menu.
- Run formatting and a targeted compile check for the affected crate.
