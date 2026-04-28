# PRODUCT.md — JSON / YAML structured-data block viewer

Issue: https://github.com/warpdotdev/warp/issues/9138
Relates to: #9138
Figma: none provided

## Summary

When a terminal block's output is valid JSON or YAML, Warp renders it as an
interactive collapsible tree instead of raw text. A toggle button on the block
lets the user switch between the tree view and the original raw output at any
time. The raw bytes are never modified; the tree is a read-only rendering lens.

## Goals / Non-goals

In scope:
- JSON detection and tree rendering for completed blocks.
- YAML detection and tree rendering for completed blocks.
- Toggle between tree view and raw text per block.
- Collapse / expand of individual nodes in the tree.
- Keyboard navigation inside the tree.
- "Copy value" and "Copy path" affordances on tree nodes.
- A Settings toggle to opt out of automatic structured-data rendering.
- A `WARP_RICH_OUTPUT=0` env-var escape hatch to suppress rendering for a
  specific command.

Out of scope for this spec (tracked separately in #9138):
- Image block rendering.
- Table block rendering (CSV/TSV/ASCII-box).
- TOML rendering.
- Editable / writable tree view.
- Streaming JSON rendering while the command is still running (the tree renders
  only after the block completes).

## Behavior

### Detection

1. After a block completes (exit code is received), Warp strips ANSI escape
   sequences from the block's output text and attempts to parse it first as JSON
   (`serde_json`) and then, if JSON fails, as YAML (`serde_yaml`). Detection
   runs only when the "Render rich output in blocks" setting is enabled (on by
   default) and `WARP_RICH_OUTPUT` is not set to `0` in the command's
   environment.

2. A block is treated as JSON if the stripped output, after trimming leading and
   trailing whitespace, parses successfully as a JSON value. A block is treated
   as YAML if it fails JSON detection and the stripped output parses
   successfully as a YAML mapping or sequence (bare scalars like `42` or `true`
   that happen to be valid YAML are not treated as YAML blocks).

3. Detection runs entirely in the background after block completion. While
   detection is in progress (sub-millisecond for typical outputs; bounded at 50 ms
   for very large outputs) the block renders normally as raw text. If detection
   succeeds within the time budget, the block transitions to tree view without
   user action.

4. If the block's output exceeds 5 MB of raw text, detection is skipped and the
   block renders as raw text. No error or indicator is shown in this case.

5. If `WARP_RICH_OUTPUT=0` is present in the environment at the time the command
   runs, detection is skipped for that block regardless of settings.

6. Detection is not retried after failing. If the output is not valid JSON or
   YAML, the block remains raw text.

### Tree view

7. When a block is in tree view, it replaces the raw text grid with a
   collapsible tree. The root node corresponds to the top-level JSON value or
   YAML document root. Object/mapping keys are shown as labeled branches;
   array/sequence elements are shown as indexed branches (`[0]`, `[1]`, …).
   Scalar leaf values (strings, numbers, booleans, nulls) are shown inline next
   to their key or index.

8. The tree renders with syntax-aware coloring derived from the active Warp
   theme: keys in one color, string values in another, numeric/boolean/null
   values in a third. No hard-coded colors; all colors come from the theme.

9. At initial render, the top two levels of the tree are expanded; deeper levels
   are collapsed. The user can expand or collapse any node by clicking its
   disclosure triangle or pressing Space / Enter while the node is focused.

10. Clicking the disclosure triangle of a collapsed object or array expands it in
    place; clicking it again collapses it. The scroll position of the block is
    preserved across expand/collapse operations.

11. Long string values (over 120 characters) are truncated with a "… (show more)"
    inline affordance. Clicking "show more" expands the string value in place;
    clicking "show less" collapses it again.

12. Keyboard navigation: Tab moves focus to the next interactive element (node
    or affordance) in the tree; Shift-Tab moves backwards. Arrow-right expands a
    focused collapsed node; Arrow-left collapses an expanded node. Arrow-down /
    Arrow-up move focus between adjacent visible nodes.

13. Pressing Cmd-A (macOS) / Ctrl-A (Windows/Linux) while focused inside the
    tree selects the raw text content of the entire block (not the tree
    rendering), so that standard copy behavior copies the original bytes.

### Toggle

14. Each block in tree view has a "Raw" toggle button in the block's hover
    toolbar, alongside the existing Copy and other block actions. Clicking it
    switches the block to raw-text rendering. The button label changes to "Tree"
    when the block is in raw-text mode, switching back to tree view when clicked.

15. The toggle state is per-block and in-session only. Restoring a session or
    reopening Warp resets all blocks to their default rendering (tree view for
    JSON/YAML blocks, raw text otherwise).

16. The toggle is visible only on hover of the block, consistent with existing
    block toolbar behavior.

### Copy behavior

17. Right-clicking a leaf value node in the tree shows a context menu with two
    items: "Copy value" and "Copy path".
    - "Copy value" copies the scalar value as a plain string (unquoted for
      strings, JSON-literal for numbers/booleans/nulls).
    - "Copy path" copies the dot-notation path from the root to the node
      (e.g., `users[0].email`). Array indices use bracket notation.

18. Right-clicking a non-leaf (object or array) node shows only "Copy value",
    which copies the subtree as pretty-printed JSON.

19. Using the block's existing Copy button (in the hover toolbar) always copies
    the raw text of the entire block, regardless of tree view state, consistent
    with how the block's serialized form is preserved.

### Settings

20. A "Render rich output in blocks" toggle in Settings → Terminal (or a
    dedicated subsection if one exists by the time this ships) controls whether
    JSON/YAML detection runs at all. Default: on. When toggled off, all existing
    tree-view blocks revert to raw text immediately; when toggled back on,
    already-completed blocks are not re-detected (detection only runs at block
    completion time).

21. The setting applies globally across all sessions, terminals, and panes.

### Edge cases

22. A block whose output is a JSON array renders the array as the root node
    (indices `[0]`, `[1]`, …). A block whose output is a JSON scalar (bare
    number, boolean, or string at the top level) renders in tree view with a
    single root leaf.

23. A block that produces partial JSON (e.g., a command that was interrupted
    mid-output) fails detection and renders as raw text. No error indicator is
    shown.

24. A block whose command exited non-zero is still eligible for tree-view
    rendering if its output is valid JSON or YAML. Exit code does not affect
    detection.

25. A block shared via Warp Drive or a share link retains the raw text as the
    canonical form. Recipients who have "Render rich output in blocks" enabled
    will see the tree view; recipients who have it disabled will see raw text.
    The shared form never bakes in the tree rendering.

26. When the block is in tree view and the terminal window is resized, the tree
    reflows to the new width. Node labels that no longer fit truncate with
    an ellipsis; expanding the window restores full display without requiring
    user interaction.

27. Agent Mode blocks that contain JSON or YAML output are subject to the same
    detection and rendering rules as user-command blocks.
