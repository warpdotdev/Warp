# GH-9810: Tech Spec

GitHub issue: https://github.com/warpdotdev/warp/issues/9810
Product spec: `specs/GH9810/PRODUCT.md`

## Problem

The collapsed conversation blocklist row currently has no inline
controls for Fork or link-copy. The implementation needs to add an
inline toolbelt + kebab overflow menu, mirror the menu under
right-click, and route every action through the same internal API
the modal-side toolbelt already uses, so authorization, redaction,
audit, and URL formatting are guaranteed identical to the modal.

## Relevant code

- Collapsed conversation row renderer: `app/src/ai/blocklist/agent_view/`
  (search for `collapsed_conversation` or grep the existing
  collapsed-row layout).
- Modal-side conversation toolbelt: source-of-truth implementations
  for Fork, copy conversation link, copy share link, copy debug link.
  The new inline toolbelt dispatches the same actions via the
  existing action-bus pattern. **Do not reimplement** any URL
  formatting, redaction, or auth-state evaluation; route through the
  modal's existing entry points.
- Overflow / context-menu primitives in
  `app/src/ai/blocklist/agent_view/` (or the shared menu primitive
  used elsewhere in the agent view).

## Implementation pointers

- Reuse existing action constants for the four actions; do not
  introduce parallel ones.
- Audit / telemetry event payloads gain a `source` field with values
  `"modal"` (existing) and `"collapsed_row"` (new). Every emit
  point must populate it.
- The kebab menu's conditional items (share / debug) MUST consult
  the same predicate function the modal uses to decide
  visibility / enabled state — extract that predicate to a shared
  helper if it isn't already one. Do not duplicate the conditions
  inline.

## Focus model

The collapsed row is a **single Tab stop**. Inline Fork and kebab
buttons are not individually Tab-stoppable. Within the row, focus
roves between three logical positions via arrow keys:

```
[ row body ] <— Left/Right —> [ Fork ] <— Left/Right —> [ Kebab ]
```

### Canonical focus implementation: `aria-activedescendant`

V1 uses **`aria-activedescendant` exclusively** for in-row focus
movement. The roving-`tabindex` alternative is explicitly NOT
used. This is the canonical contract; ambiguity between the two
approaches caused the round-N nit and is resolved here.

Concrete requirements:

1. **DOM-focus owner.** The row container is the only element
   within the row that receives DOM focus. It has
   `tabindex="0"`. The inline Fork button and the kebab button
   have `tabindex="-1"` and DO NOT receive DOM focus during
   in-row arrow navigation. (`document.activeElement` stays on
   the row container as the user presses Left/Right.)
2. **`aria-activedescendant`.** The row container exposes
   `aria-activedescendant="<id-of-active-inner-element>"`,
   updated by the keyboard handler as the user presses Left /
   Right. The three valid descendant ids are the row-body
   sentinel, the Fork button, and the kebab button. Each inner
   element has a stable id.
3. **Visual focus ring.** Because DOM focus stays on the row
   container, the visual focus ring on the active inner element
   is rendered via CSS `:where([id=...])` keyed off
   `aria-activedescendant`, NOT via `:focus` /
   `:focus-visible`. This is asserted by snapshot in T5a (added
   below).
4. **Enter / Space.** Pressing Enter or Space on the row
   container dispatches the activation handler for whichever
   element id is currently the `aria-activedescendant` value.
   Activation is a method call on the descendant, not a
   synthetic `click()` on a focused element (since the
   descendant is never DOM-focused).
5. **Escape (no menu open).** Resets
   `aria-activedescendant` to the row-body sentinel id.
6. **Roving-`tabindex` is forbidden in V1.** The implementation
   MUST NOT toggle `tabindex` between `0` and `-1` on the
   inline buttons during arrow navigation. If a future
   accessibility-engine constraint requires roving-`tabindex`
   instead, that is a follow-up spec change with its own test
   plan. The lint/code-review rule for this section: any commit
   that changes `tabindex` on the inline Fork or kebab button
   in response to a keyboard event is rejected.
7. **Menu open.** The kebab menu, when open, owns its own focus
   loop. Inside the open menu, the menu itself takes DOM focus
   (a separate scope from the in-row `aria-activedescendant`
   model). Up/Down navigate menu items; Enter/Space activate;
   Escape closes the menu and restores DOM focus to the row
   container with `aria-activedescendant` pointing at the
   kebab button id.

This single-implementation contract makes T5 (keyboard
navigation), T6 (menu key handling), and T11 (accessibility
audit) deterministic: tests assert
`document.activeElement === rowContainer` throughout in-row
navigation, and assert the value of
`row.getAttribute("aria-activedescendant")` after each
Left/Right keypress.

## Accessibility contract

The collapsed row + inline toolbelt + kebab menu MUST satisfy the
following ARIA structure:

- The row container:
  - `role="listitem"` (it lives inside the existing list).
  - `tabindex="0"` (single Tab stop).
  - `aria-label="Conversation row, [collapsed|expanded]: <title>"`.
- The inline button group (Fork + kebab) within the row:
  - `role="toolbar"` with `aria-label="Conversation actions"`.
- The Fork button:
  - `tabindex="-1"`.
  - `aria-label="Fork conversation"`.
- The kebab button:
  - `tabindex="-1"`.
  - `aria-label="More conversation actions"`.
  - `aria-haspopup="menu"`.
  - `aria-expanded` reflecting menu open / closed state.
  - `aria-controls` referencing the menu's id when open.
- The overflow menu, when open:
  - `role="menu"` with `aria-label="More conversation actions"`.
- Each menu item:
  - `role="menuitem"`.
  - Disabled items use `aria-disabled="true"` (not removed from the
    DOM) so screen readers can announce the disabled-state wording
    that matches the modal.

Screen reader announcement order:

1. Row label (e.g., "Conversation row, collapsed: <title>").
2. On Right-arrow into the toolbar: "Conversation actions toolbar,
   Fork button" (or whichever control receives focus).
3. On Right-arrow further: "More conversation actions, menu popup,
   collapsed".
4. On kebab activation: menu items announce in their list order.

## Privacy / authorization implementation

The link-copy actions are routed through the same internal API the
modal uses. Concretely:

1. The kebab menu builder calls the shared `is_share_link_available`
   predicate (extracted if not already shared) — same return value
   as the modal sees in the same state.
2. The kebab menu builder calls the shared `is_debug_link_available`
   predicate — same return value as the modal sees.
3. On activation, each item dispatches the existing action with the
   conversation id; the action impl is unchanged.
4. The action impl is the only place URL formatting, redaction, and
   audit emission happen. The collapsed-row entry point does not
   touch raw conversation data.
5. Audit emit gains `source = "collapsed_row"` (versus the modal's
   `source = "modal"`).

This guarantees URL parity with the modal (PRODUCT A7), debug-link
gating parity (A8), revoked-share state parity (A9), and audit
parity (A10).

## Test plan

- T1. Click handler dispatches Fork action; reuses the existing
  fork test fixture and asserts the same post-state as a modal-side
  fork.
- T2. Kebab menu shows the three entries with conditional presence
  rules. Driven by the shared availability predicates, asserted
  against the same predicate's modal-side return values.
- T3. Right-click outside the toolbelt opens the kebab menu at the
  cursor.
- T3a. **Right-click no-expand regression.** Right-clicking on a
  collapsed row to open the overflow menu MUST NOT also expand
  the row, open it, or otherwise change its collapsed/expanded
  state. Specifically, the test asserts:
  1. The row's `aria-expanded` (or equivalent
     collapsed/expanded state attribute) is unchanged from
     before the right-click to after the right-click and after
     the menu closes.
  2. The row's body content remains in its pre-right-click
     rendered form (no expansion DOM mutations).
  3. The activation handler that toggles expand/collapse is NOT
     invoked on the right-click event path. The test uses a spy
     on the toggle handler and asserts zero invocations.
  4. Closing the menu (Escape, click-away, item activation)
     also does NOT toggle expand/collapse.
  5. The same invariants hold whether the right-click lands on
     the row body, on the Fork button, or on the kebab button
     itself.
  This guards against the common regression where a right-click
  fires both a `contextmenu` handler (opens menu) AND a
  `mousedown` / `click` handler (toggles expand). The
  implementation MUST `preventDefault()` and `stopPropagation()`
  on the `contextmenu` event so the row's click/expand handler
  never sees it.
- T4. Modal-open state hides the inline toolbelt (snapshot test).
- T5. Keyboard navigation reaches row body, Fork, and kebab with
  Left/Right; Enter / Space activate the focused in-row control.
  Tab moves between rows, not into in-row controls.
- T5a. **`aria-activedescendant` canonical-implementation test.**
  Drive Left/Right keys through the row and assert, after each
  keypress, that:
  1. `document.activeElement === rowContainer` (DOM focus
     stays on the row container; never moves to the Fork or
     kebab button).
  2. `rowContainer.getAttribute("aria-activedescendant")`
     transitions through the three sentinel ids in the
     documented order (row body → Fork → kebab on Right,
     reverse on Left).
  3. The Fork and kebab buttons both have `tabindex="-1"`
     throughout, NEVER `tabindex="0"`, regardless of which is
     the active descendant. (Guards against an accidental
     drift to roving-`tabindex`.)
  4. The visual focus ring is keyed off `aria-activedescendant`
     via CSS, not via the native `:focus` pseudo-class
     (snapshot assertion of the rendered styles).
  5. Pressing Enter / Space with each descendant active
     invokes the descendant's activation handler exactly once
     (asserted by spies on Fork-activate and kebab-activate).
- T6. Kebab menu supports Up/Down, Enter/Space, and Escape without
  dropping focus from the collapsed row.
- T7. **Share-link parity test.** For a conversation in a fixture
  state with share enabled, assert the URL placed on the clipboard
  by the inline "Copy share link" action equals byte-for-byte the
  URL the modal-side action produces for the same fixture.
- T8. **Debug-link gate test.** For a fixture conversation without
  debug mode, assert the "Copy debug link" item is not present in
  the kebab menu and the keyboard never lands on it. For the same
  fixture with debug mode active, assert presence.
- T9. **Revoked-share parity test.** For a fixture conversation
  with share permission revoked, assert the inline "Copy share
  link" item is hidden or disabled with the same wording / state
  the modal-side surface shows for the same fixture.
- T10. **Audit parity test.** Activating each link-copy action from
  the inline kebab emits the same audit event as the modal-side
  action with `source = "collapsed_row"`. Assert the rest of the
  payload is identical.
- T11. **Accessibility audit.** Run an accessibility tree audit
  against a rendered collapsed row; assert the ARIA structure in
  this file (row, toolbar, buttons, kebab haspopup/expanded, menu,
  menuitems) is present and labels match.
