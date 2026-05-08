# Start Timestamp on Collapsed Agent Reasoning Phases (GH-10292)

## Summary

During Agent conversations, reasoning/thinking phases (and other agent sub-blocks
such as tool calls and plan steps) collapse into a single expandable header line
that currently carries no time information. This spec adds a start timestamp to
each collapsed sub-block header (e.g. `▸ Thinking… (2m ago)` or
`▸ Thinking… (11:42:07)`), and shows start time + duration when the block is
expanded. Format is configurable per user preference and locale-aware.

## Problem

- Long agent runs accumulate many collapsed reasoning / tool-call / plan-step
  rows. Users cannot tell when each phase started without expanding it, which
  makes it hard to:
  - Correlate a phase with a terminal action they remember triggering.
  - Judge whether a still-running phase has been pending too long.
  - Audit a transcript after the fact (e.g. "what was the model doing at 11:42?").
- The conversation already records start events for these sub-blocks server-side;
  they just are not surfaced in the collapsed UI.
- Per-message timestamps (PR #10128) cover the message-level granularity but do
  not extend into the sub-blocks within a single agent message.

## Goals

- Collapsed reasoning / tool-call / plan-step rows show a start timestamp inline
  in the header.
- Expanded headers show both an absolute start time and a duration (or
  `running…` for in-progress phases).
- Format respects the user's locale and existing 12h/24h convention.
- A user-facing setting selects between absolute, relative, automatic, or off.
- Coverage extends consistently across all collapsible "agent sub-blocks"
  (reasoning, tool calls, plan steps).

## Non-Goals

- Not per-token / per-chunk timing within a phase.
- Not editing or reordering phases in the conversation.
- Not new server-side analytics on per-phase durations beyond what already
  exists.
- Not redesigning the collapsed-row visual treatment beyond adding the
  timestamp affordance.
- Not changing how message-level timestamps render (those continue to come
  from #10128).

## Behavior Contract

### B1. Affected sub-blocks

The following collapsible blocks within an agent message are in scope:

- Reasoning / thinking phases.
- Tool-call blocks.
- Plan-step entries.

Any future "agent sub-block" type that has a start event in the conversation
timeline should adopt the same treatment.

### B2. Collapsed-row format

```
▸ <kind>… (<time>)
```

`<time>` is the start timestamp of the sub-block. Default formatting rule:

- RELATIVE if the start was within the last 60 minutes
  (e.g. `just now`, `34s ago`, `2m ago`).
- ABSOLUTE if older
  (e.g. `11:42:07` for 24h locale, `11:42 AM` for 12h locale).

The `(<time>)` token is rendered as a secondary, lower-contrast label so it
does not visually compete with the kind label.

### B3. Expanded-row header

When the block is expanded, the header shows both an absolute start time and a
duration:

- Completed phase: `Started 11:42:07 · 4.3s`
- In-progress phase: `Started 11:42:07 · running…` and updates as it runs.

Absolute time in the expanded header is always rendered, regardless of B2's
relative/absolute choice for the collapsed row.

### B4. Live-update cadence

- Relative timestamps refresh every 5 seconds when their row is visible.
- The single most-recent in-progress phase refreshes every 1 second.
- Absolute timestamps do not refresh.
- A single coalesced ticker drives all visible rows; per-row timers are not
  permitted (see Implementation Pointers).

### B5. Locale & 12h/24h

- Use the existing app locale.
- Use the `time.use_24h` setting when present.
- If `time.use_24h` is unset, fall back to the OS preference.

### B6. Setting

`agent.reasoning_phase_timestamp_format` controls collapsed-row formatting:

- `"off"` — no timestamp on collapsed or expanded rows.
- `"absolute"` — always absolute time.
- `"relative"` — always relative time.
- `"auto"` (default) — relative ≤ 60 min, absolute beyond.

### B7. Accessibility

- The collapsed row's `aria-label` must include a human-readable timestamp
  description, e.g. `"Thinking phase started 2 minutes ago"` or
  `"Thinking phase started at 11:42:07"`.
- The expanded header's accessible name must include both start time and
  duration (or "running" while in-progress).
- Hover tooltip (see Open Questions) must not be the only carrier of timing
  information.

## Settings / API surface

| Setting | Type | Default | Notes |
| --- | --- | --- | --- |
| `agent.reasoning_phase_timestamp_format` | enum `"off" \| "absolute" \| "relative" \| "auto"` | `"auto"` | Drives collapsed-row format. |
| `agent.reasoning_phase_timestamp_show_in_expanded` | bool | `true` | When `false`, B3 is suppressed. |

UI placement: Settings → Agents → "Reasoning phase timestamps":

- Radio group bound to `agent.reasoning_phase_timestamp_format`.
- Checkbox bound to `agent.reasoning_phase_timestamp_show_in_expanded`.

No new public API. The values flow through the existing settings store.

## Acceptance Criteria

- A1. With format `"auto"`, a collapsed reasoning row whose start time is
  within the last 60 minutes shows a relative timestamp; older than 60 minutes
  shows an absolute timestamp.
- A2. Expanding a row shows `Started <absolute> · <duration>` for completed
  phases and `Started <absolute> · running…` for in-progress phases.
- A3. Setting `"off"` removes timestamps from collapsed and expanded rows.
- A4. Setting `"absolute"` forces absolute on collapsed rows; `"relative"`
  forces relative.
- A5. 12h vs 24h rendering follows `time.use_24h` (with OS fallback when unset).
- A6. Tool-call blocks and plan-step blocks get the same treatment as reasoning
  phases (collapsed timestamp, expanded start + duration).
- A7. Relative timestamps refresh on a 5-second cadence; the most recent
  in-progress phase refreshes on a 1-second cadence.
- A8. The collapsed row's `aria-label` carries an accurate timing description
  matching the visible format.

## Implementation Pointers

- Collapsed row component: `app/src/agent/conversation/reasoning_block.rs`.
- Tool-call block: `app/src/agent/conversation/tool_call_block.rs`.
- Plan-step block: `app/src/agent/conversation/plan_step.rs`.
- Reuse the existing relative-timestamp helper if one exists (search for
  `relative_time::format` or similar). Otherwise add
  `app/src/util/relative_time.rs` with:
  - `fn format_relative(now: Instant, started_at: Instant) -> String`
  - `fn format_absolute(started_at: SystemTime, use_24h: bool, locale: &Locale) -> String`
- Live-update timer must be a single coalesced ticker per conversation list,
  fanning out to subscribed components. No per-row timers; a conversation with
  N visible reasoning rows must allocate at most one 1 Hz subscriber and one
  5 Hz aggregate refresher.
- Settings wiring: extend the existing settings schema for the agent surface;
  thread the resolved format enum into the three block components via the
  existing context/props mechanism.
- Sub-blocks' start time must come from the existing conversation event stream;
  do not introduce a parallel timing source.

## Tests

- T1. `format_relative` returns `just now` < 5 s, `Ns ago` < 60 s, `Nm ago` < 60 min.
- T2. `format_relative` falls through to `format_absolute` past 60 minutes
  when called via the `auto` adapter.
- T3. Expanded view shows correct duration once a phase emits a completion
  event.
- T4. In-progress phase shows `running…` and the duration label updates as
  the ticker fires.
- T5. Setting `"off"` removes the timestamp DOM/aria-label entirely.
- T6. Setting `"absolute"` and `"relative"` force the corresponding format
  regardless of age.
- T7. `time.use_24h = true` renders `11:42:07`; `false` renders `11:42:07 AM`
  (or locale-equivalent 12h form). OS-fallback path is exercised when the
  setting is unset.
- T8. Tool-call block and plan-step block receive identical treatment per
  B1–B3.
- T9. Stepping the mock clock by 5 s causes all visible relative-formatted
  rows to re-render with the new value.
- T10. `aria-label` audit for collapsed and expanded rows matches the visible
  text.
- T11. Timer coalescing: rendering 50 reasoning rows registers exactly one
  1 Hz subscriber and one 5 Hz subscriber against the ticker.

## Open Questions

- Should hovering a collapsed row show a tooltip with the OTHER format (e.g.
  hover relative-shown row to see absolute time)? Suggested answer: yes for
  V1 — purely additive, supports both quick scanning and precise audit. Tooltip
  must echo, not replace, the timing info already exposed via `aria-label`.
- Should the expanded view also support a "show full ISO timestamp" affordance
  (right-click / context menu), for users exporting transcripts?

## Telemetry

No new events. The existing per-phase start/complete events already carry the
timing data this spec surfaces. If usage signals are required later, add a
single `agent.reasoning_phase_timestamp.format_changed` event tied to the
setting toggle (out of scope for V1).
