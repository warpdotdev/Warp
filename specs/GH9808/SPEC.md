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
identifiers and CSS selectors that mark a container as
"blocklist-embedded scrollable".

## Behavior contract

### Gesture ownership

- B1. The first wheel/trackpad scroll event after a gesture-window
  gap determines the gesture owner. If the inner container can
  scroll in that event's direction, the inner container owns the
  gesture. If the inner container is already at its top/bottom
  boundary for that direction at the moment the gesture starts,
  the parent blocklist owns the gesture.
- B1a. When the inner container owns a gesture, subsequent scroll
  events in the same gesture window are consumed by the inner
  container even after it reaches its top/bottom edge. Remaining
  delta is discarded for the inner container and NOT propagated
  to the parent.
- B2. The "gesture window" is detected by the existing
  scroll-event-stream timing: events within ≤200ms of each other
  belong to the same gesture (matches macOS/Windows trackpad
  inertia conventions).
- B3. When the parent blocklist owns a gesture because the first
  event started while the inner container was already at its
  boundary, every subsequent scroll event in that same ≤200ms
  gesture window keeps bubbling to the parent. After a 200ms gap,
  ownership is re-evaluated from the next scroll event.
- B4. Same behavior at top AND bottom edges.

### Gesture ownership at boundaries (binding rule)

- B-OWN. **Boundary state at gesture start is the sole binding
  decision point.** When a new wheel/trackpad gesture starts (the
  first event after ≥200ms of input idleness), ownership is
  resolved exactly once from the inner container's boundary state
  in the gesture's direction at that instant:
  - inner container can scroll in direction → inner owns the
    entire gesture
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

### Scrollbar clicks

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
- B5a. **"Scrollbar target" defined.** A scrollbar target is any
  element matching either of:
  - `role="scrollbar"` ARIA semantics, or
  - the platform scrollbar pseudo-element selectors
    `::-webkit-scrollbar`, `::-webkit-scrollbar-thumb`,
    `::-webkit-scrollbar-track`, `::-webkit-scrollbar-button`
  Pointer events whose hit-target matches one of these selectors
  are scrollbar pointer events; wheel/trackpad events generated
  near these selectors are not.
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
- A3. Click or drag the inner container's scrollbar (target
  matches `role="scrollbar"` or
  `::-webkit-scrollbar*` per B5a) — the inner container scrolls
  as before, with no parent gesture clamp involved.
- A3-parent. Click or drag the parent blocklist scrollbar
  (same target match definition) — the parent scrolls
  immediately even if an inner-owned wheel gesture is in flight.
- A4. Same as A1 but at the bottom edge.
- A5. Start a new scroll gesture while the inner container is
  already at its top boundary — the parent blocklist scrolls for
  that whole gesture window. The first delta is NOT bubbled to
  the inner container.
- A6. Coverage: A1, A2, A4, A5 each pass for at least two
  distinct container types from Scope (e.g., code diff AND file
  tree, OR code diff AND inline table).

## Implementation pointers

- A "blocklist-embedded scrollable container" is any descendant
  scroll container of the blocklist whose root element matches
  one of the following identifiers (extend this list as
  containers are added):
  - code diff: diff view's scrollable wrapper component (grep
    for `on_scroll` / `scroll_event` near the diff render path)
  - file tree: directory-tree panel scroll wrapper
  - inline tables: data-table scroll wrapper used for embedded
    tabular output
  - log panels: embedded log/output scroll wrapper
- The shared scroll-clamp behavior should live in a single
  reusable wrapper / hook so each container opts in by name
  rather than duplicating gesture-window state.
- The 200ms gesture-window state is implementable as
  `last_scroll_time: Option<Instant>` plus a current gesture
  owner (`inner` / `parent`) on the container's view state; a new
  event >200ms after `last_scroll_time` is treated as a new
  gesture and re-evaluates ownership from boundary state per
  B-OWN.
- Scrollbar pointer hit-testing should resolve via ARIA role
  first, then platform pseudo-element selectors per B5a.

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
  matches B5a) remains local to the inner container and does not
  affect later wheel/trackpad ownership.
- T6. Parent-scrollbar click during an active inner-owned wheel
  gesture immediately scrolls the parent without disturbing
  remaining inner-bound wheel events.
- T7. Multi-container coverage: T1, T2, T3, T4 are repeated for
  at least one non-diff container type (file tree or inline
  table) to verify the shared wrapper applies uniformly.

## Out of scope

- Animated "rubber-band" overshoot indicator. The fix is silent
  absorption.
- Configurable threshold (200ms is a hard-coded constant; can be
  tuned per platform if profiling shows misfires).
- Non-blocklist-embedded scroll containers (e.g., top-level app
  panels, modal scroll regions) — those follow native browser /
  OS scroll-chaining semantics and are unaffected by V1.
