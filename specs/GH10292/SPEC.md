# Start Timestamp on Collapsed Agent Sub-Blocks (GH-10292)

> Note: setting was renamed mid-spec from the earlier draft
> `agent.reasoning_phase_timestamp_format` to `agent.subblock_timestamp_format`
> for accuracy across affected sub-block types (reasoning, tool calls, plan steps).

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

`<time>` is the start timestamp of the sub-block. Default formatting rule
(`auto`):

- RELATIVE if the start was within the last 60 minutes
  (e.g. `just now`, `34s ago`, `2m ago`).
- ABSOLUTE if older
  (rendered per the canonical absolute-time format defined in B2.1).

The `(<time>)` token is rendered as a secondary, lower-contrast label so it
does not visually compete with the kind label.

### B2.1. Canonical absolute-time format

Every absolute timestamp shown by this feature — collapsed row, expanded header,
or `aria-label` — uses the SAME canonical form. Seconds are ALWAYS included so
collapsed and expanded rows are visually consistent.

- **24-hour mode** (`time.use_24h = true`):
  `HH:MM:SS` — zero-padded; locale-independent for the time portion.
  Example: `11:42:07`.
- **12-hour mode** (`time.use_24h = false`):
  `h:MM:SS AM/PM` — hour NOT zero-padded; minutes/seconds zero-padded;
  AM/PM marker per locale (e.g. `AM`/`PM` in en-US; `a.m.`/`p.m.` in
  some locales). Example: `11:42:07 AM`.
- **Older than 24 hours** (in any mode that resolves to absolute):
  prefix with the locale-appropriate short date in
  `YYYY-MM-DD <time>` form. Example (24h): `2026-05-08 11:42:07`;
  example (12h): `2026-05-08 11:42:07 AM`.
- The locale governs the short date and the AM/PM marker only; the
  digit/colon ordering of the time portion is fixed for parity.

### B2.2. Forced `relative` mode beyond 60 minutes

When the user explicitly sets the format to `"relative"` (NOT `"auto"`), the
relative form must extend past the 60-minute window:

| Age range | Format | Example |
| --- | --- | --- |
| < 5 s | `just now` | `just now` |
| 5 s – < 60 s | `<N>s ago` | `34s ago` |
| 60 s – < 60 min | `<N>m ago` | `2m ago`, `59m ago` |
| 60 min – < 120 min | `<N>m ago` (rounded down) | `60m ago`, `119m ago` |
| 2 h – < 48 h | `<N>h ago` (rounded down) | `2h ago`, `47h ago` |
| 48 h – < 7 d | `<N>d ago` (rounded down) | `2d ago`, `6d ago` |
| ≥ 7 d | `<N>w ago` (rounded down) | `1w ago`, `12w ago` |

All ranges round DOWN to the unit shown (floor division). This applies ONLY
when the user has explicitly chosen `"relative"`. The default `"auto"` mode
continues to switch to absolute past the 60-minute mark per B2.

### B3. Expanded-row header

When the block is expanded, the header shows both an absolute start time and a
duration:

- Completed phase: `Started 11:42:07 · 4.3s`
- In-progress phase: `Started 11:42:07 · running…` and updates as it runs.

Absolute time in the expanded header is always rendered using B2.1's canonical
form, regardless of B2's relative/absolute choice for the collapsed row.

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

`agent.subblock_timestamp_format` controls collapsed-row formatting:

- `"off"` — no timestamp on collapsed or expanded rows.
- `"absolute"` — always absolute time per B2.1.
- `"relative"` — always relative time per B2.2 (extends past 60 min).
- `"auto"` (default) — relative ≤ 60 min, absolute beyond per B2.1.

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
| `agent.subblock_timestamp_format` | enum `"off" \| "absolute" \| "relative" \| "auto"` | `"auto"` | Drives collapsed-row format for reasoning, tool-call, and plan-step sub-blocks. |
| `agent.subblock_timestamp_show_in_expanded` | bool | `true` | When `false`, B3 is suppressed. |

UI placement: Settings → Agents → "Agent sub-block timestamps":

- Radio group bound to `agent.subblock_timestamp_format`.
- Checkbox bound to `agent.subblock_timestamp_show_in_expanded`.

The label "Agent sub-block timestamps" reflects the broader scope of this
feature — it covers reasoning phases, tool-call blocks, and plan-step blocks,
not just reasoning. The earlier draft used the narrower
`agent.reasoning_phase_timestamp_format` / "Reasoning phase timestamps" naming;
the renamed form is the canonical version going forward.

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

> Paths verified against the worktree at spec time. Modules that don't yet
> exist are marked `(new module)` so reviewers can distinguish net-new files
> from edits to existing files.

- Agent block output renderer (collapsed/expanded reasoning header lives here
  today; "Thinking" label and `thinking_display_mode` are wired in this file):
  `app/src/ai/blocklist/block/view_impl/output.rs`.
- Agent block view container:
  `app/src/ai/blocklist/agent_view/agent_view_block.rs`.
- Settings (existing `ThinkingDisplayMode` enum is the closest precedent for
  the new `subblock_timestamp_format` enum and lives alongside other
  AI/agent settings): `app/src/settings/ai.rs`.
- Settings UI placement (Agents page — radio group + checkbox land here):
  `app/src/settings_view/ai_page.rs`.
- Settings migration / initialization (mirror the
  `KeepThinkingExpanded → ThinkingDisplayMode` migration pattern when adding
  the new enum, see lines around 121–155): `app/src/settings/initializer.rs`.
- Conversation event stream / per-phase start & completion events
  (timestamps surfaced here come from this stream, not a new timing source):
  `app/src/ai/blocklist/orchestration_events.rs` and
  `app/src/ai/agent/conversation_yaml.rs`.
- `(new module)` Time-formatting helper:
  `app/src/util/relative_time.rs` (no existing equivalent found in
  `app/src/util/`). Module exposes:
  - `fn format_relative_auto(now: SystemTime, started_at: SystemTime) -> RelativeOrAbsolute`
    (caps at 60 min, then signals fallthrough to absolute).
  - `fn format_relative_extended(now: SystemTime, started_at: SystemTime) -> String`
    (implements B2.2 ranges past 60 min; used only for forced `"relative"`).
  - `fn format_absolute(started_at: SystemTime, use_24h: bool, locale: &Locale) -> String`
    (implements B2.1; emits date-prefixed form when older than 24 h).
- `(new module or co-located in agent_view)` Coalesced ticker for live-update
  cadence — single 1 Hz + 5 Hz subscriber per conversation list. Suggested
  location: `app/src/ai/blocklist/agent_view/timestamp_ticker.rs`. No
  per-row timers.
- Tool-call and plan-step rendering: tool calls flow through the same agent
  output renderer above (`view_impl/output.rs`); plan-step UI is rendered via
  `app/src/ai/blocklist/prompt/plan_and_todo_list.rs`. Both pick up the
  resolved `subblock_timestamp_format` via the existing settings context.
- Sub-blocks' start time must come from the existing conversation event stream;
  do not introduce a parallel timing source.

## Tests

- T1. `format_relative_auto` returns `just now` < 5 s, `Ns ago` < 60 s,
  `Nm ago` < 60 min.
- T2. `format_relative_auto` signals fallthrough to `format_absolute` past 60
  minutes; `auto` adapter renders the canonical absolute form per B2.1.
- T3. Expanded view shows correct duration once a phase emits a completion
  event.
- T4. In-progress phase shows `running…` and the duration label updates as
  the ticker fires.
- T5. Setting `"off"` removes the timestamp DOM/aria-label entirely.
- T6. Setting `"absolute"` and `"relative"` force the corresponding format
  regardless of age. Forced `"relative"` exercises B2.2 ranges:
  `60m ago`, `2h ago`, `2d ago`, `1w ago`.
- T7. Absolute form rendering per B2.1: `time.use_24h = true` renders
  `11:42:07`; `time.use_24h = false` renders `11:42:07 AM`. A start time older
  than 24 h prefixes the locale date (e.g. `2026-05-08 11:42:07` /
  `2026-05-08 11:42:07 AM`). OS-fallback path is exercised when the setting
  is unset.
- T8. Tool-call block and plan-step block receive identical treatment per
  B1–B3.
- T9. Stepping the mock clock by 5 s causes all visible relative-formatted
  rows to re-render with the new value.
- T10. `aria-label` audit for collapsed and expanded rows matches the visible
  text, including the canonical absolute form when applicable.
- T11. Timer coalescing: rendering 50 sub-block rows registers exactly one
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
single `agent.subblock_timestamp.format_changed` event tied to the
setting toggle (out of scope for V1).
