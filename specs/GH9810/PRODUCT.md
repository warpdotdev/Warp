# Spec: Toolbelt buttons on collapsed conversation blocklist items (GH-9810)

## Problem

When a conversation is collapsed in the blocklist (i.e., not
opened in the modal view), users have no quick way to fork it or
copy a debug/share link. They must first expand the conversation,
then locate the action — friction for the "I just want to share
this debug link" workflow.

## Goal

Add a toolbelt to collapsed conversation blocklist items with two
controls: a primary **Fork** button and a kebab overflow menu for
secondary actions (copy conversation link, copy share link when
available, copy debug link).

## Behavior contract

- B1. Collapsed conversation blocklist items render an inline
  toolbelt aligned to the trailing edge.
- B2. Primary action: **Fork**. Single-click forks the
  conversation; identical semantics to the Fork button inside the
  modal view.
- B3. Kebab overflow menu (⋯) contains:
  - "Copy conversation link" (always present).
  - "Copy share link" (present only when the conversation has a
    share URL — same condition the modal uses).
  - "Copy debug link" (present only in dev / debug-mode builds OR
    when the user has the existing developer-tools setting on).
- B4. Right-click on the row anywhere outside the toolbelt opens
  the same kebab overflow menu (per @david's request in the issue).
- B5. The toolbelt is collapsed-state-only — when the user
  expands the conversation into the modal view, the existing
  modal-side toolbelt is the source of truth and the inline
  toolbelt is hidden (no duplication).
- B6. Keyboard focus uses row-roving navigation. Tab/Shift-Tab
  moves focus between collapsed conversation rows as it does today;
  the inline Fork button and kebab are not separate Tab stops.
  When a collapsed row is focused, Left/Right arrow moves the
  active in-row control between the row body, Fork button, and
  kebab. Enter or Space activates the active in-row control:
  - Row body: opens/expands the conversation using today's row
    behavior.
  - Fork button: forks the conversation with the same semantics as
    pointer-clicking Fork.
  - Kebab: opens the overflow menu.
- B7. Once the kebab menu is open, Up/Down arrow moves between menu
  items, Enter/Space activates the highlighted item, Escape closes
  the menu and restores focus to the collapsed row, and Tab follows
  the app's existing menu-dismiss behavior.

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
  navigation.

## Implementation pointers

- Collapsed conversation row renderer lives in
  `app/src/ai/blocklist/agent_view/` (search for
  `collapsed_conversation` or grep the existing collapsed-row
  layout).
- The modal-side Fork action and link-copy actions are the
  source-of-truth implementations; the new buttons dispatch the
  same actions via the existing action-bus pattern.

## Test plan

- T1. Click handler dispatches Fork action; reuses the existing
  fork test fixture.
- T2. Kebab menu shows the three entries with conditional
  presence rules.
- T3. Right-click outside the toolbelt opens the kebab menu.
- T4. Modal-open state hides the inline toolbelt (snapshot test).
- T5. Keyboard navigation reaches row body, Fork, and kebab with
  Left/Right; Enter/Space activate the focused in-row control.
- T6. Kebab menu supports Up/Down, Enter/Space, and Escape without
  dropping focus from the collapsed row.

## Out of scope

- New top-level actions beyond the four listed.
- Drag-and-drop or multi-select on collapsed rows.
- Touch / mobile gesture support.
