# PRODUCT.md — Search sessions by custom tab and pane names

Issue: https://github.com/warpdotdev/warp/issues/9155

## Summary

Make user-assigned custom names for tabs and panes discoverable through every session-search surface in Warp. A user who has renamed a tab (e.g. "deploy") or a specific pane within a tab (e.g. "logs") can find that session by typing the name they chose, whether they search from the command palette or the vertical-tabs sidebar's "search tabs…" input. In the command-palette result row, the matched name is surfaced and highlighted so the user can see which tab or pane each result corresponds to.

Figma: none provided.

## Problem

Today the two session-search surfaces handle custom names inconsistently:

- The command-palette session search indexes prompt, command, and hint text only — neither custom tab names nor custom pane names participate.
- The vertical-tabs sidebar's "search tabs…" input includes custom names in some display modes but silently drops the custom *tab* name in Panes mode (the mode where each pane is its own row), so typing a renamed tab's name returns no results in the sidebar even though the name is visible on screen.

Users who rename tabs and panes expect the name they assigned to be the most reliable way to find a session. The current behavior contradicts that expectation in both surfaces.

## Goals / Non-goals

In scope:

- Command-palette session search matches against the user-set custom tab name and custom pane name.
- The command-palette result row displays the custom tab name and/or custom pane name when set, with highlighting.
- The vertical-tabs sidebar search matches against the user-set custom tab name and custom pane name in every sidebar display mode (summary, focused-session, panes), without regressing existing matches on prompt, command, hint, generated title, or subtitle.
- Behavior is consistent across the two surfaces and across the underlying search paths each surface uses; the user perceives the same matching rules everywhere.

Out of scope:

- Tab-rename and pane-rename UX themselves (the rename actions, inline editor, telemetry events) — unchanged.
- Indexing auto-generated tab and pane titles (the fallbacks shown when no custom name is set) as new "custom-name" fields. Auto-generated titles continue to be reachable via the existing prompt / command / generated-title match surfaces wherever they are reachable today.
- Visual redesign of either surface beyond the additions described in the result-row invariants.
- Other search surfaces (command-history search, settings search, etc.).
- Changes to how custom tab and pane names are persisted across restarts — existing persistence behavior is unchanged.

## Behavior

Session search appears on two surfaces; both must follow these invariants:

- **Command palette** — the cross-tab session navigator opened from the command palette.
- **Vertical-tabs sidebar** — the "search tabs…" input at the top of the side panel; filters the sidebar's visible rows in every display mode it offers (summary, focused-session, panes).

A session may have up to two custom names attached:

- A **custom tab name**, set on the tab containing the session — applies to every session (pane) in that tab.
- A **custom pane name**, set on the specific pane the session occupies — applies to only that one session.

Both, either, or neither may be set. Where the behavior is identical for both, "custom name" refers to either one. Unless an invariant is explicitly scoped to one surface, it applies to both.

1. **Searchable text — custom names only.** When a tab has a non-empty custom tab name, that name participates in session search for every session in that tab. When a pane has a non-empty custom pane name, that name participates in session search for the one session in that pane. Tabs and panes without a custom name behave exactly as they do today — no new match surface is introduced for them.

2. **Empty and whitespace-only name handling.** A custom name that is zero-length, or consists entirely of whitespace, is not indexed for search and produces no leading label in the command-palette result row. Leading and trailing whitespace around an otherwise non-empty name is ignored for indexing and for the result-row label; interior whitespace is preserved. This applies equally to custom tab names and custom pane names. (Sidebar header rendering of custom names is unchanged by this feature; whitespace consistency in `PaneGroup::set_title` is tracked separately — see tech.md Follow-ups.)

3. **Case-insensitive substring match.** Typing any case-insensitive substring of a session's custom tab name or custom pane name returns that session as a hit on every session-search surface. The match behavior is identical regardless of which underlying search path serves the query; the user perceives a single, consistent search.

4. **Multi-byte / non-ASCII names.** Custom names containing multi-byte characters (CJK, emoji, accented characters) match the same way as multi-byte content already does for prompt / command / generated title. Highlight ranges remain correct — no off-by-one between byte and character indexing.

5. **Restored sessions are searchable.** Custom tab and pane names persisted in a previous Warp run and restored on launch become searchable by their restored values as soon as restoration completes, with no extra user action.

6. **Renames take effect on the next search.** Setting, changing, or clearing a custom tab name or custom pane name is reflected in the very next session-search invocation on either surface. A cleared name produces no stale match; a freshly-set name is immediately findable. The sidebar's filtered view re-evaluates against the active query as soon as the rename commits.

7. **Multi-pane tabs.**
   - A tab containing multiple panes has at most one custom tab name, shared by every session in that tab. A query that matches the tab name returns each of those sessions.
   - Each pane in a multi-pane tab can independently have its own custom pane name. A query that matches a single pane's custom name returns only that pane's session — not its siblings in the same tab.
   - A session can be matched on its tab name, on its own pane name, or on both, independently.
   - In the command palette, sessions sharing a matching tab name appear as separate result rows, not collapsed.
   - In the sidebar, a tab-name match keeps the tab visible and shows every pane row in it that the user would normally see in that mode; a pane-name-only match keeps the tab visible but filters that tab's pane list to the matching pane(s).

8. **Multi-window.** Custom tab and pane names from every open window participate in search wherever cross-window session search already reaches today, consistent with existing cross-window behavior.

9. **No duplicated rows on multi-field match (command palette).** When a query matches a session on more than one field — any combination of custom tab name, custom pane name, prompt, command, hint — the session appears exactly once in the result list. Every matched field is reflected in highlighting (see invariant 13).

10. **Sidebar parity across display modes.** In every vertical-tabs display mode (summary, focused-session, panes), typing a substring of a tab's custom name keeps that tab visible in the filtered view, and typing a substring of a pane's custom name keeps the pane (and its containing tab) visible. There is no display mode in which a custom name is shown on screen but not searchable. This explicitly includes Panes mode, where today the tab-level custom name renders as a group header but is not in the search index.

11. **Sidebar — no regression to existing matches.** Every query that produced a sidebar match before this change still produces the same match — generated tab titles, subtitles, terminal prompt/command, and pane titles remain reachable. Adding custom-name search must not produce false negatives.

12. **Result row — labels shown when set (command palette).** Whenever a session in the command-palette result list has a custom tab name and/or a custom pane name, the row displays each set name as a leading label at the start of the primary text line, ahead of the existing prompt/command/hint content. Labels are shown whether or not the current query matched them.
    - When only a custom tab name is set: the tab name is shown as a single leading label.
    - When only a custom pane name is set: the pane name is shown as a single leading label.
    - When both are set: both are shown together with the tab name first followed by the pane name, visually distinguished as two segments so the user can tell tab-level context apart from pane-level context.
    - **Open question:** the exact visual treatment between the two segments (separator character, spacing, weight, color) is a design decision and should match the Figma when one is provided.

13. **Result row — no label when neither set (command palette).** When a session has neither a custom tab name nor a custom pane name, the result-row layout is unchanged from today: no leading label, no empty placeholder, no extra whitespace.

14. **Result row — highlighting (command palette).** When the query matches a substring of a custom tab name or custom pane name, that substring is visually highlighted within the corresponding label using the same highlight treatment used today for prompt/command/hint matches. When the query matches multiple fields (e.g. both labels, or a label and another field), all matching segments are highlighted simultaneously in their respective regions of the row.

15. **Result row — overflow (command palette).** Long custom names truncate within the result row using the same truncation rules already applied to prompt/command/hint, without pushing other row content off-screen. When truncation is necessary and the query matched a label, the truncation preserves the matched substring in the visible portion of that label (e.g. by anchoring truncation away from the match) so the user can see why the row matched. When both labels are present and the combined label width exceeds the available space, both labels share that space rather than one starving the other entirely.

16. **No length cap introduced.** This feature does not impose new maximum lengths on custom tab names or custom pane names beyond whatever the rename UX already enforces. Whatever name the user is allowed to set is fully indexed for search and fully available for display (subject to the truncation rules in invariant 15).

17. **Sidebar display unchanged.** This feature does not change how the vertical-tabs sidebar renders custom tab or pane names today. Tab-name group headers, pane-row titles, and existing affordances render the same as before. The change in the sidebar is search behavior only.

18. **Active tab and active pane not excluded.** A session belonging to the currently-focused tab or pane can still appear in command-palette results and sidebar matches when the query matches it. This change does not introduce filtering by focus state.

19. **Theming and accessibility.** The new command-palette leading labels and their highlights render correctly across all themes (light, dark, custom). When both a tab-name label and a pane-name label are shown, the visual relationship between them (separator, weight) renders correctly across all themes. Each label's text is exposed to assistive technologies as part of the row's accessible name, alongside the existing prompt/command/hint content. No label text is announced only visually.

20. **No regression for unmatched queries.** A query that does not appear in any custom tab name, custom pane name, prompt, command, hint, generated title, or subtitle returns the same set of results (empty or otherwise) it would have returned before this change, on either surface. Adding custom-name search must not produce false negatives for previously-matching queries.

21. **Rename UX and telemetry are unchanged.** The tab-rename and pane-rename UX (inline editors, Enter/Escape behavior, setting and clearing custom names) and existing rename telemetry events are unchanged by this feature. This is purely a search-and-display addition; renaming itself does not gain new side effects.
