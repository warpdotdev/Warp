# Spec: Prevent overshoot when scrolling top of code diff (GH-9808)

## Problem

While scrolling up in a code diff inside a long blocklist, when
the scroll hits the top of the diff it spills over into scrolling
the parent blocklist. Users want the inner-scroll boundary to
*absorb* the rest of an in-progress scroll gesture — only on
discrete subsequent scrolls should the parent take over.

The same overshoot issue occurs for other blocklist-embedded
scrollable containers (file trees, inline tables, log panels);
the fix in V1 covers all of them.

## Goal

When a scroll gesture starts inside any blocklist-embedded
scrollable container that can scroll in the gesture direction,
clamp the gesture to that container until the gesture ends. A new
scroll gesture that starts while the inner container is already
at its boundary is free to hit the parent.

## Scope (V1)

V1 applies to ALL blocklist-embedded scrollable containers, not
just code diffs. Affected container types include, at minimum:

- code-diff scroll containers (the original GH-9808 case)
- file-tree / directory-tree panels
- inline data tables
- inline log / output panels

Implementation pointers below enumerate the concrete component
identifiers that mark a container as
"blocklist-embedded scrollable".

### Direction coverage (vertical AND horizontal)

The clamp applies to BOTH vertical and horizontal axes per
container, independently. Some blocklist-embedded containers
scroll only vertically (e.g., file trees and most diffs); others
support both axes (e.g., a diff with long unwrapped lines, or a
wide inline table). The behavior contract is per-axis:

- For each axis, "boundary" means at the top/left edge or
  bottom/right edge in that axis's direction.
- If the inner container can scroll in the gesture's primary axis
  direction, it owns the gesture for that axis.
- If the inner container is at its boundary in the primary axis at
  gesture start, the parent owns that axis for the gesture window.

#### Multi-axis (diagonal) gestures

For containers that can scroll on both axes, a diagonal gesture
must be locked to a single axis to apply the clamp deterministically:

1. **Primary-axis selection.** A gesture is classified by the
   accumulated delta of its first event. If `|dy| >= 0.7 * (|dx| + |dy|)`
   the gesture is **vertical-primary**; if
   `|dx| >= 0.7 * (|dx| + |dy|)` it is **horizontal-primary**.
2. **Ambiguous diagonal.** If neither axis crosses the 70%
   threshold on the first event, the gesture is **axis-locked by
   first boundary collision**: the first axis whose boundary the
   inner container is already at decides ownership for that axis;
   if neither is at a boundary, the gesture is provisionally
   inner-owned on both axes and the lock resolves to whichever
   axis the inner container hits a boundary in first within the
   same gesture window. Once locked, the lock persists for the
   gesture window.
3. **Per-axis ownership.** Once primary axis is selected,
   ownership is resolved per B-OWN below using only that axis's
   boundary state. The non-primary-axis component of the gesture
   continues to be applied to the inner container if it can
   scroll in that direction (i.e., the lock affects which
   container owns scroll-chaining, not whether each axis scrolls
   the inner container).

## Behavior contract

### Gesture ownership

- B1. The first wheel/trackpad scroll event after a gesture-window
  gap determines the gesture owner. If the inner container can
  scroll in that event's direction (per the primary axis selected
  above), the inner container owns the gesture. If the inner
  container is already at its top/bottom (or left/right) boundary
  for that direction at the moment the gesture starts, the parent
  blocklist owns the gesture.
- B1a. When the inner container owns a gesture, subsequent scroll
  events in the same gesture window are consumed by the inner
  container even after it reaches its top/bottom (or left/right)
  edge. Remaining delta is discarded for the inner container and
  NOT propagated to the parent.
- B2. The "gesture window" is detected by the existing
  scroll-event-stream timing: events within ≤200ms of each other
  belong to the same gesture (matches macOS/Windows trackpad
  inertia conventions).
- B3. When the parent blocklist owns a gesture because the first
  event started while the inner container was already at its
  boundary, every subsequent scroll event in that same ≤200ms
  gesture window keeps bubbling to the parent. After a 200ms gap,
  ownership is re-evaluated from the next scroll event.
- B4. Same behavior at top AND bottom edges (vertical) AND at
  left AND right edges (horizontal).

### Gesture ownership at boundaries (binding rule)

- B-OWN. **Boundary state at gesture start is the sole binding
  decision point.** When a new wheel/trackpad gesture starts (the
  first event after ≥200ms of input idleness), ownership is
  resolved exactly once from the inner container's boundary state
  in the gesture's primary-axis direction at that instant:
  - inner container can scroll in primary-axis direction → inner
    owns the entire gesture
  - inner container is at its boundary in that direction → parent
    owns the entire gesture
- B-OWN-1. Implementers MUST NOT bubble the first delta to the
  inner container and then later "discover" the boundary and
  re-route remaining deltas to the parent. There is no mid-gesture
  transfer of ownership. Once a gesture is bound to a container
  on its first event, it stays bound until 200ms of input
  idleness ends the gesture.
- B-OWN-2. The 200ms gesture window applies only to gestures that
  have already started. It does not "split" a single gesture into
  inner-then-parent halves.

### Scrollbar clicks (Warp custom scrollbar hit-test contract)

Warp uses a custom-rendered scrollbar
(`crates/warpui_core/src/elements/shared_scrollbar.rs` and the
new-scrollable family in
`crates/warpui_core/src/elements/new_scrollable/`). Hit-testing
goes through Warp's own scrollbar geometry, NOT browser ARIA
roles or `::-webkit-scrollbar*` pseudo-element selectors. The
`role="scrollbar"` / pseudo-element references in the previous
draft of this spec were incorrect for Warp's rendering model and
are dropped.

- B5. Click/drag on a scrollbar bypasses the wheel/trackpad
  gesture clamp because it is a pointer action, not a
  wheel/trackpad gesture. Specifically:
  - The parent blocklist's own scrollbar always responds to
    direct click/drag, regardless of any active inner gesture.
  - The inner container's scrollbar — when explicitly clicked or
    dragged via thumb or track — also bypasses the gesture clamp
    because click is a non-momentum input.
  - Wheel/trackpad input that happens to occur over the scrollbar
    area still respects the gesture-window contract above; only
    actual click/drag pointer input bypasses.
- B5a. **"Scrollbar target" defined (Warp scrollbar hit-test).**
  A scrollbar pointer hit is determined by Warp's scrollbar
  module via the following hit-test contract:

  ```rust
  // crates/warpui_core/src/elements/shared_scrollbar.rs
  pub enum ScrollbarTarget {
      Track,   // pointer over the track region (track_bounds minus thumb_bounds)
      Thumb,   // pointer over the thumb region (thumb_bounds)
      None,    // pointer not over scrollbar geometry
  }

  pub fn hit_test(point: Vector2F, geometry: &ScrollbarGeometry)
      -> ScrollbarTarget;
  ```

  `ScrollbarGeometry` is the existing `track_bounds` /
  `thumb_bounds` data exposed by `shared_scrollbar.rs`.

  Pointer events whose hit-test returns `Track` or `Thumb` are
  scrollbar pointer events and bypass the gesture clamp. Pointer
  events that return `None` are routed to the inner container's
  content. Wheel/trackpad events are NEVER classified through
  this hit-test; they always route to the gesture-ownership
  state machine.
- B5b. Neither scrollbar click changes gesture ownership for
  later wheel/trackpad events.

## Acceptance criteria

In each criterion below, "inner container" means any
blocklist-embedded scrollable container in scope (see Scope). At
least two distinct container types must satisfy each criterion
in tests.

- A1. Scroll up in a long inner container; reach top; same
  gesture continues — blocklist does NOT scroll. Scrollbar at the
  inner container's top remains visible.
- A2. Stop scrolling for >200ms, scroll up again — blocklist
  scrolls normally.
- A3. Click or drag the inner container's scrollbar (Warp
  scrollbar `hit_test()` returns `Track` or `Thumb` per B5a) —
  the inner container scrolls as before, with no parent gesture
  clamp involved.
- A3-parent. Click or drag the parent blocklist scrollbar (same
  `hit_test()` definition) — the parent scrolls immediately even
  if an inner-owned wheel gesture is in flight.
- A4. Same as A1 but at the bottom edge.
- A4h. Horizontal-axis mirror of A1: scroll left in a wide inner
  container; reach left edge; same gesture continues — blocklist
  does NOT scroll horizontally.
- A4h-right. Horizontal-axis mirror of A4: scroll right; reach
  right edge; same gesture continues — blocklist does NOT scroll
  horizontally.
- A5. Start a new scroll gesture while the inner container is
  already at its top boundary — the parent blocklist scrolls for
  that whole gesture window. The first delta is NOT bubbled to
  the inner container.
- A6. Coverage: A1, A2, A4, A5 each pass for at least two
  distinct container types from Scope.
- A7. Diagonal-gesture axis lock: a diagonal trackpad gesture
  whose first event has `|dy| >= 0.7 * (|dx|+|dy|)` is treated
  as vertical-primary; ownership is resolved by the vertical
  boundary only. The horizontal component scrolls the inner
  container if it can. After the gesture ends, the next gesture
  re-classifies independently.
- A8. Wheel-over-scrollbar non-bypass: a wheel event whose cursor
  is positioned over the scrollbar track is processed by the
  gesture-ownership state machine (NOT bypassed), per B5a.

## Implementation pointers

A "blocklist-embedded scrollable container" is any descendant
scroll container of the blocklist whose root element matches one
of the following identifiers in the codebase. This list must be
extended as new containers are added.

| Container type    | File path                                                     | Component identifier                                         |
| ----------------- | ------------------------------------------------------------- | ------------------------------------------------------------ |
| Code diff (inline action) | `app/src/ai/blocklist/inline_action/code_diff_view.rs` | The diff view's outer scrollable wrapper (constructed via the shared scrollable element from `warpui_core`) |
| Code diff (standalone)    | `app/src/code/diff_viewer.rs`                          | `diff_viewer` scrollable region                              |
| File tree         | `app/src/code/file_tree/view.rs`, `view/render.rs`            | File-tree view's scrollable wrapper                          |
| Inline tables (markdown) | `crates/ai/src/gfm_table.rs`                            | GFM table rendered into `crates/editor/src/render/element/table.rs`; the table cell's scrollable wrapper when content overflows |
| Log / output panels | `app/src/terminal/block_list_viewport.rs`                   | Block-list viewport's embedded output regions                |

The shared clamp lives in the `Scrollable` /
`new_scrollable` element family in
`crates/warpui_core/src/elements/`. The clamp behavior is added
as an opt-in flag on the scrollable wrapper (e.g.,
`clamp_to_parent_blocklist: bool`) so each container opts in by
constructing the wrapper with the flag set, rather than
duplicating gesture-window state across containers.

The 200ms gesture-window state is implementable as
`last_scroll_time: Option<Instant>` plus a current gesture
owner (`InnerVertical`, `InnerHorizontal`, `Parent`, or `None`)
on the wrapper's view state; a new event >200ms after
`last_scroll_time` is treated as a new gesture and re-evaluates
ownership from boundary state per B-OWN.

Scrollbar pointer hit-testing resolves via the
`ScrollbarTarget` hit-test API on the existing scrollbar geometry
(`shared_scrollbar.rs`) — NOT via ARIA roles or browser
pseudo-elements.

## Test plan

Each test below runs against at least two container types from
Scope.

- T1. Synthetic scroll-event stream test: feed deltas with
  100ms spacing reaching the top, assert no propagation to
  parent.
- T2. Same stream with a 300ms gap mid-stream; assert deltas
  after the gap propagate.
- T3. Bottom-edge mirror of T1/T2.
- T4. A new stream whose first delta starts while the inner
  container is already at the boundary propagates ALL deltas in
  that stream to the parent — including the very first delta
  (no first-delta-to-inner bubble).
- T5. Discrete inner-container scrollbar-click jump (target
  matches B5a Warp `hit_test()` returning `Track` or `Thumb`)
  remains local to the inner container and does not affect later
  wheel/trackpad ownership.
- T6. Parent-scrollbar click during an active inner-owned wheel
  gesture immediately scrolls the parent without disturbing
  remaining inner-bound wheel events.
- T7. Multi-container coverage: T1, T2, T3, T4 are repeated
  across each container type listed in Implementation Pointers
  to verify the shared wrapper applies uniformly. Specifically:
  - T_diff_clamp: code-diff container (inline + standalone)
  - T_filetree_clamp: file-tree container
  - T_table_clamp: GFM-table container with horizontal overflow
  - T_log_clamp: block-list viewport output region
- T_horizontal_axis_clamp. Horizontal-axis mirror of T1: deltas
  at 100ms spacing along the X axis, reaching the left edge;
  assert no propagation to parent. Repeat at right edge.
- T_diagonal_axis_lock. Diagonal trackpad gesture: feed first
  event with `(dx, dy) = (10, 25)` (vertical-primary because
  `25/35 >= 0.7`); assert vertical boundary determines ownership
  and horizontal component still scrolls the inner container.
  Then feed `(dx, dy) = (25, 10)` and assert horizontal-primary
  classification. Then feed `(dx, dy) = (15, 15)` (ambiguous);
  assert axis-lock by first boundary collision per Direction
  coverage.
- T_wheel_over_scrollbar. Wheel events whose cursor is over the
  scrollbar track go through the gesture state machine and are
  NOT classified as scrollbar clicks (per A8 / B5a).

## Out of scope

- Animated "rubber-band" overshoot indicator. The fix is silent
  absorption.
- Configurable threshold (200ms is a hard-coded constant; can be
  tuned per platform if profiling shows misfires).
- Non-blocklist-embedded scroll containers (e.g., top-level app
  panels, modal scroll regions) — those follow native browser /
  OS scroll-chaining semantics and are unaffected by V1.
