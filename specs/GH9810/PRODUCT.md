# GH-9810: Toolbelt buttons on collapsed conversation blocklist items

GitHub issue: https://github.com/warpdotdev/warp/issues/9810

## Summary

When a conversation is collapsed in the blocklist (i.e., not opened in
the modal view), users have no quick way to fork it or copy a debug /
share link. Today they must first expand the conversation, then locate
the action — friction for the "I just want to share this debug link"
workflow. This feature adds an inline toolbelt to collapsed
conversation rows with a primary Fork button and a kebab overflow
menu for secondary link-copy actions.

## Problem

- Forking and link-copying are common conversation actions; on a
  collapsed row they're invisible until the user expands the row,
  which requires extra clicks and changes the user's view context.
- The link-copy actions (conversation, share, debug) already exist
  inside the modal-side toolbelt, but the collapsed-row surface has
  no entry point for them at all.
- Right-click context menus on collapsed rows are currently unused;
  they're a natural surface for the secondary actions but were not
  wired up.

## Goals

1. Add an inline toolbelt to collapsed conversation blocklist items
   exposing a primary Fork action and a kebab overflow menu for
   conversation-link / share-link / debug-link copy.
2. Make the new actions reuse the exact same internal entry points
   that the modal-side toolbelt uses, so authorization, redaction,
   audit, and URL formatting are guaranteed identical.
3. Make the new controls keyboard- and screen-reader accessible
   without changing the row-level Tab-stop model (a collapsed row
   stays a single Tab stop).
4. Mirror the kebab overflow menu under right-click on the row.

## Non-goals

- New top-level actions beyond the four listed (Fork, copy
  conversation link, copy share link, copy debug link).
- Drag-and-drop or multi-select on collapsed rows.
- Touch / mobile gesture support.
- Changing the modal-side toolbelt or the underlying share / debug
  link generation pipelines. The modal remains the source of truth;
  the inline toolbelt is a new entry point only.

## User-facing behavior

### Inline toolbelt on collapsed rows

- B1. Collapsed conversation blocklist items render an inline
  toolbelt aligned to the trailing edge.
- B2. Primary action: **Fork**. Single-click forks the conversation;
  identical semantics to the Fork button inside the modal view.
- B3. Kebab overflow menu (⋯) contains:
  - "Copy conversation link" (always present).
  - "Copy share link" (present only when the conversation has a
    share URL — same condition the modal uses).
  - "Copy debug link" (present only when debug-mode is active for
    the conversation — same condition the modal uses, including
    any developer-tools setting gate).
- B4. Right-click on the row anywhere outside the toolbelt opens the
  same kebab overflow menu (per @david's request in the issue).
- B5. The toolbelt is collapsed-state-only — when the user expands
  the conversation into the modal view, the existing modal-side
  toolbelt is the source of truth and the inline toolbelt is
  hidden (no duplication).

### Link-copy privacy contract

- B6. The share-link and debug-link copy actions in the kebab menu
  reuse the **exact same** authorization, redaction, audit, and URL
  formatting pathways as the modal-side share / debug surfaces.
  The collapsed-row menu is a new entry point but does NOT bypass
  any modal precondition. Concretely:
  - **Auth state.** If the user is not authenticated for share, the
    Copy share link item is hidden or disabled with the same
    wording the modal uses.
  - **Share permission.** If share permission is revoked for the
    conversation, the item is hidden or disabled with the same
    wording the modal uses.
  - **Debug-mode gate.** If debug mode is not active for the
    conversation, the Copy debug link item is not shown.
  - **Redaction.** Whatever redaction the modal applies before
    producing a share / debug URL also applies on the collapsed
    row. The collapsed row never holds raw conversation content
    that the modal would have redacted.
  - **Audit.** Each copy action emits the same audit / telemetry
    event the modal emits, with `source = "collapsed_row"` so the
    surface is distinguishable in audit logs.
- B7. URL parity. The URL placed on the clipboard from the kebab
  menu is byte-for-byte identical to the URL the modal-side action
  would produce for the same conversation in the same state.

### Keyboard model

The collapsed row remains a **single Tab stop**; the inline buttons
inside it are not individually Tab-stoppable. Roving focus between
in-row controls is via arrow keys.

- B8. Tab / Shift-Tab moves focus between collapsed conversation
  rows as it does today. Inline Fork and kebab are not separate
  Tab stops.
- B9. When a collapsed row has Tab focus:
  - Left / Right arrow rove the active in-row control between the
    row body, the Fork button, and the kebab.
  - Enter or Space activates the active in-row control:
    - Row body: opens / expands the conversation using today's row
      behavior.
    - Fork button: forks the conversation.
    - Kebab: opens the overflow menu.
  - Escape, when no menu is open, returns the active control to
    the row body (no-op if it was already there).
- B10. When the kebab menu is open:
  - Up / Down arrow moves between menu items.
  - Enter / Space activates the highlighted item.
  - Escape closes the menu and restores Tab focus to the kebab
    button (and therefore to the row).
  - Tab follows the app's existing menu-dismiss behavior.

## Acceptance criteria

- A1. Collapsed conversation row shows a Fork button + kebab.
- A2. Click Fork: conversation forks with the same outcome as
  modal-side Fork.
- A3. Click kebab → "Copy conversation link" copies the same URL
  the modal-side action would.
- A4. Right-click row → kebab menu opens at the cursor.
- A5. Expanding the conversation into the modal view hides the
  inline toolbelt.
- A6. With keyboard focus on a collapsed row, users can reach and
  activate both Fork and kebab without leaving row-roving
  navigation; arrow-roving and Enter/Space activation behave per
  B9 / B10.
- A7. **Share-link parity.** The "Copy share link" item, when
  enabled, places on the clipboard a URL byte-for-byte equal to
  the URL produced by the modal-side share-link action for the
  same conversation in the same state.
- A8. **Debug-link gate.** The "Copy debug link" item is shown only
  when debug mode is active for the conversation, matching the
  modal-side gate exactly.
- A9. **Revoked-share state.** When share permission has been
  revoked for the conversation, the inline "Copy share link" item
  is hidden or disabled with the same wording the modal-side
  surface uses for the same state.
- A10. **Audit parity.** A copy action from the inline kebab menu
  emits the same audit / telemetry event as the modal-side action,
  with `source = "collapsed_row"`.

## Open questions

None outstanding. Resolved during spec review:

- Right-click on the row outside the toolbelt is the agreed
  surface for the overflow menu (per @david).
- The kebab is the only mouse-click affordance for the secondary
  actions; no second visible button per item.

## Success metrics

- Time-to-copy share link from a collapsed row: target reduction
  versus the current expand-then-copy flow (instrumented via the
  `source = "collapsed_row"` audit field).
- Adoption: percentage of share-link / debug-link copies originating
  from the collapsed row vs the modal, segmented by week.

## See also

- Tech spec: `specs/GH9810/TECH.md`
