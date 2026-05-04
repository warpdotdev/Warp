# GH9816: Configurable code editor line number modes
## Summary
Add a configurable line numbering mode for Warp code editors so users can choose Absolute, Relative, or Hybrid line numbers. The setting is independent of Vim mode, defaults to today’s absolute numbering, and makes Vim-style vertical motion counts easier to read when users choose relative or hybrid numbering.
## Problem
Warp code editors currently show only absolute line numbers. Vim users commonly rely on relative or hybrid line numbers to choose motions like `5j` and `12k` without mentally subtracting the current line from nearby line numbers. Because the existing input editor surfaces do not show line-number gutters, this feature should improve the code editor without implying a new gutter in command input.
## Goals
1. Let users choose how line numbers are displayed in code editor gutters.
2. Preserve the current absolute numbering behavior by default.
3. Make relative and hybrid modes update immediately as the active cursor line changes.
4. Keep the setting usable by all code editor users, not only users who enable Vim keybindings.
5. Avoid adding line numbers to command input editors or rich-text notebook editors as part of this change.
## Non-goals
1. Do not add line-number gutters to the terminal input editor, AI input editor, or other command-entry surfaces.
2. Do not add Vim `:set number`, `:set relativenumber`, or `:set norelativenumber` commands in this iteration.
3. Do not change Vim motion behavior, cursor movement, selections, search, find-references, or diff navigation.
4. Do not change whether a particular code editor surface chooses to show or hide its gutter; the new setting only affects gutters that already render line numbers.
5. Do not redesign the gutter width, diff hunk controls, hidden-section controls, or inline review comment controls beyond the minimum needed to display the selected numbering mode.
## Figma
Figma: none provided. The feature reuses the existing code editor gutter and settings UI patterns.
## Behavior
1. Warp exposes a line numbering mode setting for code editors with exactly three choices:
   - **Absolute**: each line shows its absolute, one-based line number. This is the current behavior and the default for all users.
   - **Relative**: the active cursor line shows `0`; every other visible line shows the absolute distance in lines from the active cursor line.
   - **Hybrid**: the active cursor line shows its absolute, one-based line number; every other visible line shows the absolute distance in lines from the active cursor line.
2. The setting is available from Settings under the Text Editing area, near the existing code/text editing controls. It is not nested under the Vim-mode toggle and remains visible whether Vim keybindings are enabled or disabled.
3. The setting persists like other public editor settings and is restored for future Warp windows and sessions. If the user has not chosen a value, Warp behaves exactly as it does today: Absolute mode.
4. Changing the setting updates all currently open code editor gutters without requiring the user to reopen files, restart Warp, toggle Vim mode, or refocus the editor.
5. Absolute mode is behaviorally identical to the current code editor gutter:
   - The first file line displays `1`, the second displays `2`, and so on.
   - Code editor surfaces that start numbering from a caller-provided starting line continue to use that starting line.
   - Hidden sections, diff hunk controls, and gutter action buttons behave as they do today.
6. Relative mode uses the active cursor line as the origin:
   - If the cursor is on line 10, line 10 displays `0`, line 9 displays `1`, line 11 displays `1`, line 5 displays `5`, and line 22 displays `12`.
   - Moving the cursor, clicking another line, selecting text, or using keyboard navigation recomputes the displayed distances immediately.
   - The numbers are always non-negative integers; lines above and below the cursor both show positive distances.
7. Hybrid mode combines absolute and relative display:
   - The active cursor line shows its absolute line number.
   - All other visible numbered lines show their relative distance from the active cursor line.
   - With the cursor on line 10, line 10 displays `10`, line 9 displays `1`, line 11 displays `1`, and line 22 displays `12`.
8. For multiple cursors or multiple selections, the active cursor line is the primary selection head used by the editor for cursor-position reporting. The gutter uses that single active line as the relative origin until a future design intentionally supports multiple relative origins.
9. For visual selections, the active line remains the selection head, not the selection anchor or the full selected range. The displayed numbers update as the selection head moves.
10. The line number mode is independent of Vim mode:
    - Users can choose Relative or Hybrid before enabling Vim keybindings.
    - Enabling or disabling Vim keybindings does not reset or hide the chosen line numbering mode.
    - Vim status bar and clipboard settings remain separate from line numbering.
11. Code editor line numbers correspond to logical file lines, not soft-wrap rows. A long line that visually wraps still has one line number, and wrapped continuation rows do not introduce extra relative counts.
12. Hidden/collapsed code regions keep their existing hidden-section gutter affordances. The hidden section itself does not need to display a relative count for every hidden line.
13. Diff and review gutters keep their current affordances:
    - Current-buffer lines that already display a line number use the chosen mode.
    - Removed/temporary diff lines that currently omit a line number continue to omit one unless a separate diff design changes that behavior.
    - Diff hunk buttons, comment buttons, hover hit targets, and collapse/expand interactions continue to work.
14. The visual style of line numbers remains consistent with the existing gutter: same font family, size, colors, selection behavior, and alignment unless a small width or alignment adjustment is necessary to prevent relative or hybrid values from clipping.
15. The gutter reserves enough width for the largest value that can appear in the current mode:
    - Absolute and Hybrid must fit the largest absolute line number for the surface.
    - Relative must fit the largest visible relative distance where practical, and must never overlap editor text or gutter controls.
16. Command input editors, terminal prompt editors, AI input editors, and rich-text notebook editors do not show line numbers after this change. Their Vim status indicators and Vim keybindings continue to work as they do today.
17. The settings UI is searchable with terms such as `line number`, `relative line`, `hybrid`, `vim`, and `gutter`.
18. If a settings file contains an invalid line numbering value, Warp falls back to Absolute mode using the existing settings validation/error behavior rather than failing to render editors.
## Success criteria
1. A new user or upgraded user with no explicit setting sees the same absolute code editor line numbers as before.
2. Selecting Relative mode shows `0` on the active cursor line and distances on surrounding numbered code editor lines.
3. Selecting Hybrid mode shows the active cursor line’s absolute number and distances on surrounding numbered code editor lines.
4. Moving the cursor with mouse, arrow keys, search, Vim motions, or goto-line updates relative and hybrid gutters immediately.
5. The setting is visible and usable when Vim keybindings are disabled.
6. Enabling or disabling Vim keybindings does not change the selected line numbering mode.
7. Terminal input and AI input surfaces still do not render line-number gutters.
8. Diff hunk controls, inline comment buttons, hidden-section controls, and find-references anchoring still work in code editors with each line numbering mode.
## Validation
1. Manually verify a multi-line file in the code editor with Absolute, Relative, and Hybrid modes selected.
2. In Relative and Hybrid modes, move the cursor above and below visible lines using mouse, arrow keys, goto-line, and Vim motions; verify displayed values update correctly.
3. Verify the setting persists after closing and reopening Warp or reloading settings.
4. Verify the setting remains visible when Vim mode is disabled and that toggling Vim mode does not reset it.
5. Verify terminal command input, AI input, and rich-text notebook editors do not gain line-number gutters.
6. Verify code review/diff editors still show diff decorations and gutter buttons correctly in all three modes.
