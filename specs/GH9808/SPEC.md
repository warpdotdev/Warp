# Spec: Prevent overshoot when scrolling top of code diff (GH-9808)

## Problem

While scrolling up in a code diff inside a long blocklist, when
the scroll hits the top of the diff it spills over into scrolling
the parent blocklist. Users want the inner-scroll boundary to
*absorb* the rest of an in-progress scroll gesture — only on
discrete subsequent scrolls should the parent take over.

## Goal

When a scroll gesture starts inside a code-diff scroll container
that can scroll in the gesture direction, clamp the gesture to
that container until the gesture ends. A new scroll gesture that
starts while the diff is already at its boundary is free to hit
the parent.

## Behavior contract

- B1. The first wheel/trackpad scroll event after a gesture-window
  gap determines the gesture owner. If the diff can scroll in that
  event's direction, the diff owns the gesture. If the diff is
  already at its top/bottom boundary for that direction, the parent
  blocklist owns the gesture.
- B1a. When the diff owns a gesture, subsequent scroll events in
  the same gesture window are consumed by the diff even after it
  reaches its top/bottom edge. Remaining delta is discarded for the
  diff and NOT propagated to the parent.
- B2. The "gesture window" is detected by the existing
  scroll-event-stream timing: events within ≤200ms of each other
  belong to the same gesture (matches macOS/Windows trackpad
  inertia conventions).
- B3. When the parent blocklist owns a gesture because the first
  event started while the diff was already at its boundary, every
  subsequent scroll event in that same ≤200ms gesture window keeps
  bubbling to the parent. After a 200ms gap, ownership is
  re-evaluated from the next scroll event.
- B4. Same behavior at top AND bottom edges.
- B5. Click-to-jump bypasses this rule because it is a pointer
  action, not a wheel/trackpad gesture. Clicking or dragging the
  inner diff scrollbar affects only the diff scroll position.
  Clicking or dragging the parent blocklist scrollbar affects only
  the parent blocklist scroll position. Neither scrollbar click
  changes gesture ownership for later wheel/trackpad events.
- B6. V1 applies only to code-diff scroll containers embedded in
  the blocklist. Code review snippets, embedded markdown, and other
  blocklist-embedded scroll containers are out of scope unless they
  reuse the same diff scroll wrapper.

## Acceptance criteria

- A1. Scroll up in a long diff; reach top; same gesture
  continues — blocklist does NOT scroll. Scroll bar at the diff's
  top remains visible.
- A2. Stop scrolling for >200ms, scroll up again — blocklist
  scrolls normally.
- A3. Click or drag the inner diff scrollbar — the diff scrolls as
  before, with no parent gesture clamp involved.
- A4. Same for bottom edge in a long diff.
- A5. Start a new scroll gesture while the diff is already at its
  top boundary — the parent blocklist scrolls for that whole
  gesture window.

## Implementation pointers

- Scroll handling for embedded diff containers is in the diff
  view's scrollable wrapper; grep for `on_scroll` /
  `scroll_event` near the diff render path.
- The 200ms gesture-window is implementable as
  `last_scroll_time: Option<Instant>` plus a current gesture owner
  (`diff` / `parent`) on the container's view state; a new event
  >200ms after `last_scroll_time` is treated as a new gesture.

## Test plan

- T1. Synthetic scroll-event stream test: feed deltas with
  100ms spacing reaching the top, assert no propagation to parent.
- T2. Same stream with a 300ms gap mid-stream; assert deltas
  after the gap propagate.
- T3. Bottom-edge mirror of T1/T2.
- T4. A new stream whose first delta starts while the diff is
  already at the boundary propagates all deltas in that stream to
  the parent.
- T5. Discrete inner diff scrollbar-click jump remains local to
  the diff and does not affect later wheel/trackpad ownership.

## Out of scope

- Animated "rubber-band" overshoot indicator. The fix is silent
  absorption.
- Configurable threshold (200ms is a hard-coded constant; can be
  tuned per platform if profiling shows misfires).
- Applying the same gesture-owner behavior to non-diff embedded
  scroll containers such as code review snippets or embedded
  markdown, unless they already share the diff scroll wrapper.
