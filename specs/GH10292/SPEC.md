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
- Expanded headers show both an absolute start time and a live elapsed counter
  (in-progress) or final duration (completed) — the elapsed value updates on
  the existing 1 Hz ticker, never as a separate `running…` placeholder.
- Format respects the user's locale and existing 12h/24h convention.
- A user-facing setting selects between absolute, relative, automatic, or off.
- A second user-facing setting (`agent.subblock_timestamp_show_in_expanded`)
  controls whether the timestamp affordance appears in the expanded view at
  all; the format setting always governs the format itself.
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

When the block is expanded, the header shows both an absolute start time and an
elapsed counter / final duration. The same field is reused across the in-progress
and completed states — there is NO separate `running…` placeholder.

- In-progress phase: `Started 11:42:07 · <elapsed> elapsed`, where `<elapsed>`
  is computed from the start timestamp at each tick of the existing 1 Hz
  subscriber (B4). Examples while ticking: `1s elapsed`, `2s elapsed`,
  `3s elapsed`, … `4s elapsed`.
- Completed phase: `Started 11:42:07 · 4.3s` — at the moment the completion
  event arrives, the elapsed value is replaced by the final duration string
  (matching the precision used for completed phases elsewhere) and stops
  updating.

The expanded view always displays a numeric elapsed/duration value; it never
shows a static `running…` label. The 1 Hz ticker described in B4 is the only
clock driving in-progress updates — no per-row timer.

Absolute time in the expanded header is always rendered using B2.1's canonical
form, regardless of B2's relative/absolute choice for the collapsed row.

### B3.1. Visibility of the expanded-row timestamp

The `agent.subblock_timestamp_show_in_expanded` setting (Settings / API surface)
controls whether the expanded-view header carries the timestamp affordance at
all. Its precedence is defined in B6.

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

### B6.1. Precedence: format vs. show_in_expanded

The two settings are orthogonal but interact:

- `agent.subblock_timestamp_format` controls the FORMAT used wherever a
  timestamp is rendered (off / absolute / relative / auto).
- `agent.subblock_timestamp_show_in_expanded` controls VISIBILITY in the
  EXPANDED view ONLY. It has no effect on the collapsed row.

The full 3 × 2 matrix:

| `format`     | `show_in_expanded` | Collapsed row timestamp | Expanded header timestamp |
| ------------ | ------------------ | ----------------------- | ------------------------- |
| `"off"`      | `true`  *or* `false` | hidden                  | hidden (the format-`off` rule wins; `show_in_expanded` has no effect) |
| `"absolute"` | `true`             | absolute, per B2.1      | `Started <absolute> · <elapsed>/duration` per B3 |
| `"absolute"` | `false`            | absolute, per B2.1      | hidden (no `Started …` line and no elapsed/duration affordance in the expanded header) |
| `"relative"` | `true`             | relative, per B2.2      | `Started <absolute> · <elapsed>/duration` per B3 (expanded form ALWAYS uses canonical absolute, per B3) |
| `"relative"` | `false`            | relative, per B2.2      | hidden |
| `"auto"`     | `true`             | auto (rel ≤ 60 min, else abs) | `Started <absolute> · <elapsed>/duration` per B3 |
| `"auto"`     | `false`            | auto (rel ≤ 60 min, else abs) | hidden |

When the expanded header timestamp is hidden because of `show_in_expanded =
false`, the entire `Started … · <elapsed>/duration` affordance is suppressed —
NOT just the start time. This is intentional so the expanded view is fully
free of timing chrome when the user opts out.

The format `"off"` rule is the global override: when format is `"off"`,
`show_in_expanded` is ignored and the affordance is hidden in BOTH views.

### B7. Accessibility

- The collapsed row's `aria-label` must include a human-readable timestamp
  description, e.g. `"Thinking phase started 2 minutes ago"` or
  `"Thinking phase started at 11:42:07"`.
- The expanded header's accessible name must include start time plus the
  elapsed counter (in-progress) or final duration (completed) — wording
  matches the visible text and updates on the 1 Hz ticker.
- Hover tooltip (see Open Questions) must not be the only carrier of timing
  information.

## Settings / API surface

| Setting | Type | Default | Notes |
| --- | --- | --- | --- |
| `agent.subblock_timestamp_format` | enum `"off" \| "absolute" \| "relative" \| "auto"` | `"auto"` | Drives FORMAT for reasoning, tool-call, and plan-step sub-blocks. `"off"` is the global override — hides timestamps in BOTH collapsed and expanded views. |
| `agent.subblock_timestamp_show_in_expanded` | bool | `true` | Controls VISIBILITY in the EXPANDED view ONLY. When `false`, the entire `Started … · <elapsed>/duration` affordance is suppressed in the expanded header (collapsed row is unaffected). Has NO effect when `format = "off"`. Full precedence in B6.1. |

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
- A2. Expanding a completed phase shows `Started <absolute> · <duration>`
  (e.g. `Started 11:42:07 · 4.3s`). Expanding an in-progress phase shows
  `Started <absolute> · <elapsed> elapsed`, where `<elapsed>` advances on the
  existing 1 Hz ticker (e.g. `1s elapsed`, `2s elapsed`, …) and is replaced by
  the final duration string the moment the completion event arrives.
- A2.1. The expanded view never displays a static `running…` label. The
  elapsed counter is the only in-progress affordance.
- A3. Setting format `"off"` removes timestamps from collapsed and expanded
  rows in BOTH views, regardless of `subblock_timestamp_show_in_expanded`.
- A4. Setting `"absolute"` forces absolute on collapsed rows; `"relative"`
  forces relative.
- A5. 12h vs 24h rendering follows `time.use_24h` (with OS fallback when unset).
- A6. Tool-call blocks and plan-step blocks get the same treatment as reasoning
  phases (collapsed timestamp, expanded start + elapsed/duration per A2).
- A7. Relative timestamps refresh on a 5-second cadence; the most recent
  in-progress phase refreshes on a 1-second cadence and drives the elapsed
  counter described in A2.
- A8. The collapsed row's `aria-label` carries an accurate timing description
  matching the visible format.
- A_show_in_expanded_off_format_on. With format `!= "off"` and
  `subblock_timestamp_show_in_expanded = false`, the collapsed row shows the
  timestamp per its format setting, and the expanded header shows NO
  `Started …` line and NO elapsed/duration affordance.
- A_show_in_expanded_on. With format `!= "off"` and
  `subblock_timestamp_show_in_expanded = true`, both views show timestamps
  per B3.
- A_show_in_expanded_format_off_overrides. With format `"off"`, the value of
  `subblock_timestamp_show_in_expanded` (true OR false) has NO effect:
  timestamps remain hidden in BOTH views.

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
  `app/src/util/relative_time.rs`. The codebase already has
  `app/src/util/time_format.rs` for relative human-readable durations
  (e.g. `format_approx_duration_from_now`); the new module is dedicated to
  this feature's per-sub-block needs and MUST be the SINGLE shared helper used
  by reasoning, tool-call, and plan-step rendering — no duplicated formatters.
  Module exposes:
  - `fn format_relative_auto(now: SystemTime, started_at: SystemTime) -> RelativeOrAbsolute`
    (caps at 60 min, then signals fallthrough to absolute).
  - `fn format_relative_extended(now: SystemTime, started_at: SystemTime) -> String`
    (implements B2.2 ranges past 60 min; used only for forced `"relative"`).
  - `fn format_absolute(started_at: SystemTime, now: SystemTime, prefer_24h: bool) -> String`
    (implements B2.1). The clock is INJECTED via `now` so the date-prefix
    branch (`now - started_at > 24 h`) is fully deterministic and unit-testable
    without depending on the system clock. Behavior:
    - When `now - started_at <= 24 h`: time-only form per B2.1
      (`HH:MM:SS` or `h:MM:SS AM/PM`).
    - When `now - started_at > 24 h`: date-prefixed form per B2.1.
  - `(internal)` `fn format_date_prefix(date: SystemTime, locale: &Locale) -> String`
    used by `format_absolute` only. The locale resolver from
    `app/src/util/time_format.rs` plus `time.use_24h` are the inputs; if no
    locale-aware short-date helper exists yet, V1 uses the locale-neutral
    ISO-8601 form `YYYY-MM-DD` as a deterministic fallback. V1.5 may swap in a
    locale-aware short date.
- A SINGLE shared helper rule: all sub-block renderers (reasoning, tool-call,
  plan-step) call into `relative_time.rs` for both the collapsed and expanded
  forms. No renderer should hand-roll its own time string. The expanded view's
  in-progress elapsed counter (B3) calls `human_readable_precise_duration`
  from `app/src/util/time_format.rs` once per 1 Hz tick — do NOT introduce a
  parallel implementation.
- `(new module or co-located in agent_view)` Coalesced ticker for live-update
  cadence — single 1 Hz + 5 Hz subscriber per conversation list. Suggested
  location: `app/src/ai/blocklist/agent_view/timestamp_ticker.rs`. The 1 Hz
  subscriber drives BOTH the most-recent in-progress relative timestamp AND
  the expanded-view elapsed counter (B3). No per-row timers.
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
- T3. Expanded view shows the final duration string (e.g. `4.3s`) once a phase
  emits a completion event, replacing the prior elapsed counter at the exact
  tick the completion arrives.
- T4. In-progress phase shows the elapsed counter (`1s elapsed`, `2s elapsed`,
  …) and the value advances on each 1 Hz tick of the ticker. Test asserts the
  text never reads `running…` — the elapsed counter is the only in-progress
  affordance.
- T5. Setting format `"off"` removes the timestamp DOM/aria-label entirely
  from BOTH collapsed and expanded views, regardless of `show_in_expanded`.
- T6. Setting `"absolute"` and `"relative"` force the corresponding format
  regardless of age. Forced `"relative"` exercises B2.2 ranges:
  `60m ago`, `2h ago`, `2d ago`, `1w ago`.
- T7. Absolute form rendering per B2.1 with `format_absolute(start, now,
  prefer_24h)`: `prefer_24h = true` renders `11:42:07`; `prefer_24h = false`
  renders `11:42:07 AM`. With `now - start > 24 h` the helper prefixes the
  date (V1 fallback `2026-05-08`, e.g. `2026-05-08 11:42:07` /
  `2026-05-08 11:42:07 AM`). The clock-injection contract is exercised by
  passing two synthetic `now` values across the 24 h boundary in the same test
  to confirm no system-clock dependency.
- T7.1. The shared-helper rule is enforced: a single `format_absolute` call
  site is used by reasoning, tool-call, and plan-step rendering. Test asserts
  identical output for the same `(start, now, prefer_24h)` across all three
  call sites.
- T8. Tool-call block and plan-step block receive identical treatment per
  B1–B3.
- T9. Stepping the mock clock by 5 s causes all visible relative-formatted
  rows to re-render with the new value.
- T10. `aria-label` audit for collapsed and expanded rows matches the visible
  text, including the canonical absolute form when applicable. Expanded-view
  aria-label MUST include the elapsed counter while in-progress (e.g. "Started
  at 11:42:07, 4 seconds elapsed") and the final duration once complete.
- T11. Timer coalescing: rendering 50 sub-block rows registers exactly one
  1 Hz subscriber and one 5 Hz subscriber against the ticker.
- T_show_in_expanded_matrix. The 3 × 2 matrix in B6.1 is exercised end-to-end
  per cell: for each combination of `format ∈ {"off", "absolute", "relative",
  "auto"}` × `show_in_expanded ∈ {true, false}`, the test asserts the exact
  collapsed-row and expanded-header rendering described in B6.1's table. This
  includes the format-`"off"` override (both `show_in_expanded` values yield
  identical hidden output).

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
