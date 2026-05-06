# Spec: Prevent overshoot when scrolling top of code diff (GH-9808)

## Problem

While scrolling up in a code diff inside a long blocklist, when
the scroll hits the top of the diff it spills over into scrolling
the parent blocklist. Users want the inner-scroll boundary to
*absorb* the rest of an in-progress scroll gesture — only on
discrete subsequent scrolls should the parent take over.

## Goal

When a scroll gesture starts inside a code-diff scroll container,
clamp the gesture to that container until the gesture ends. A
new scroll gesture (after a brief settling period) is free to
hit the parent.

## Behavior contract

- B1. A scroll event arriving at the diff scroll container while
  a gesture is in flight (i.e., wheel/trackpad delta within the
  same gesture window) is consumed by the diff even when the
  diff is at its top/bottom edge. The remaining delta is
  discarded for the diff and NOT propagated to the parent.
- B2. The "gesture window" is detected by the existing
  scroll-event-stream timing: events within ≤200ms of each other
  belong to the same gesture (matches macOS/Windows trackpad
  inertia conventions).
- B3. After a 200ms gap, the next scroll arriving at the diff
  bubbles up if the diff is at its boundary.
- B4. Same behavior at top AND bottom edges.
- B5. Click-to-jump (clicking the scrollbar) bypasses this rule
  — that's a discrete event, not part of a gesture.
- B6. The new behavior applies to **all** scroll containers
  embedded in the blocklist (code diff, code review snippets,
  embedded markdown), not just code diff. Issue references diff
  but the same UX gap applies elsewhere.

## Acceptance criteria

- A1. Scroll up in a long diff; reach top; same gesture
  continues — blocklist does NOT scroll. Scroll bar at the diff's
  top remains visible.
- A2. Stop scrolling for >200ms, scroll up again — blocklist
  scrolls normally.
- A3. Click the scrollbar above the diff to jump up — works as
  before (no gesture-clamp).
- A4. Same for bottom edge in a long diff.

## Implementation pointers

- Scroll handling for embedded diff containers is in the diff
  view's scrollable wrapper; grep for `on_scroll` /
  `scroll_event` near the diff render path.
- The 200ms gesture-window is implementable as
  `last_scroll_time: Option<Instant>` on the container's view
  state; a new event >200ms after `last_scroll_time` is treated
  as a new gesture.

## Test plan

- T1. Synthetic scroll-event stream test: feed deltas with
  100ms spacing reaching the top, assert no propagation to parent.
- T2. Same stream with a 300ms gap mid-stream; assert deltas
  after the gap propagate.
- T3. Bottom-edge mirror of T1/T2.
- T4. Discrete scrollbar-click jump still propagates.

## Out of scope

- Animated "rubber-band" overshoot indicator. The fix is silent
  absorption.
- Configurable threshold (200ms is a hard-coded constant; can be
  tuned per platform if profiling shows misfires).
