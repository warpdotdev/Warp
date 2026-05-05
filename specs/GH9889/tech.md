# Technical spec: Per-message timestamps in Agent Mode (GH-9889)

This spec is the implementation companion to `product.md`. It picks
the data-source path, the UI integration site, the timer strategy,
and the testable invariants.

## What already exists

- `AIAgentExchange` carries a `start_ts: Option<DateTime<Local>>`
  and `completed_ts: Option<DateTime<Local>>`
  ([app/src/ai/agent/conversation.rs:3218-3219](app/src/ai/agent/conversation.rs)).
- `Conversation::start_time_from_exchange_messages` derives the
  start time from the latest input's `AIAgentContext::CurrentTime`
  ([app/src/ai/agent/conversation.rs:556](app/src/ai/agent/conversation.rs)).
- The corresponding finish-time helper at lines 538-553 derives
  `completed_ts` from the latest message timestamp on the exchange.
- `Conversation::start_ts` exposes the conversation-wide start
  ([app/src/ai/agent/conversation.rs:920](app/src/ai/agent/conversation.rs)).
- `chrono::Local` is the existing convention for derived times.
- Conversation restoration replays the underlying message
  timestamps, so restored conversations have the same `start_ts` /
  `completed_ts` derivation as live ones (A6 is automatic if the
  view re-derives on render).

## What does not exist

- No view-layer surface for these timestamps. Every consumer today
  uses them for ordering, status, or export — never for display
  next to a message bubble.
- No relative-time formatter. The codebase uses
  `Local.timestamp_opt(...)` for raw `DateTime<Local>` and then
  defers to whatever the consumer needs.
- No 1Hz tick infrastructure for live duration counters in the
  agent view. The block-status header has its own independent
  refresh path; we should not graft onto that.

## Data path: derive at render time, not store on the view model

The agent view renders from the `Conversation` model. Add two pure
helpers (no new state):

```rust
// In a new module: app/src/ai/blocklist/agent_view/exchange_times.rs

pub struct ExchangeTimes {
    pub submitted_at: Option<DateTime<Local>>,
    pub completed_at: Option<DateTime<Local>>,  // None == in progress
    pub cancelled_at: Option<DateTime<Local>>,  // None unless cancelled
}

pub fn exchange_times(
    conversation: &Conversation,
    exchange: &AIAgentExchange,
) -> ExchangeTimes { ... }
```

`exchange_times` reuses `start_time_from_exchange_messages` (rename
to `pub(crate)` — currently private at conversation.rs:556) and a
sibling helper for completion. Cancellation is detected via the
existing exchange status enum.

Why a fresh helper module instead of methods on `Conversation`:
- `Conversation` is already large.
- The helper has zero callers outside the agent view; co-locating
  it next to the consumer reduces blast radius.

## UI integration site

Per product.md §risk 1 recommendation: **inline in the existing
message-bubble metadata row**.

For the user prompt bubble (in `agent_view/agent_message_bar.rs`):
- Append a `TimestampLabel` widget next to the existing badges
  (model, branch, etc.) with `submitted_at`.

For the agent response bubble:
- Append a `TimestampLabel` with `completed_at` and a duration
  `DurationLabel`. If `completed_at` is `None`, render a live
  `ProgressDurationLabel` instead.

Three new widgets:
- `TimestampLabel` — relative-or-absolute renderer, refreshes via
  the shared 30s tick.
- `DurationLabel` — `submitted_at → completed_at` formatter.
- `ProgressDurationLabel` — live "running for Xs" counter, refreshes
  via the shared 1Hz tick.

All three live in `app/src/ai/blocklist/agent_view/timestamp_widgets.rs`.

## Timer strategy

Per product.md §risk 2 recommendation: **single shared 1Hz timer,
gated to visible in-progress exchanges**.

- Add a `TimestampTickService` registered on the agent view's owning
  context.
- It holds a `Vec<Weak<RefCell<dyn Tick>>>` of subscribers
  (the `ProgressDurationLabel` instances and the 30s `TimestampLabel`
  refresh).
- Two tick frequencies: 1Hz for progress labels, 30s for relative
  labels. Use a single underlying timer firing at 1Hz and
  internally rate-limit the 30s consumers.
- Stops ticking when the agent view loses visibility
  (existing `is_visible` hook on the view; verify by grepping
  the agent_view mod).
- Resumes on visibility regain.

Why not per-widget timers:
- A long conversation can have 50+ visible exchanges. 50 separate
  per-widget timers would dwarf the cost of one shared timer.
- A shared timer makes "pause when hidden" a one-line toggle
  instead of a per-widget cleanup dance.

## Settings entry

Add a new boolean setting:

```rust
// In app/src/settings/ai.rs near the existing voice_input_toggle_key

settings::macros::implement_setting!(
    show_message_timestamps: bool,
    AISettings,
    SupportedPlatforms::DESKTOP,
    SyncToCloud::Always,
    private: false,
    toml_path: "agents.show_message_timestamps",
    description: "Show submitted-at and completion timestamps next to each Agent Mode exchange.",
    default_value: true,
);
```

`SyncToCloud::Always` because this is a UX preference that should
follow the user across devices (unlike voice hotkey which is
device-specific).

The agent view reads this setting on render; when false, none of
the three widgets are added to the bubble layout (so there is no
layout cost in the off case — A5 invariant).

## Format auto-promotion

`TimestampLabel` re-renders on every 30s tick AND on visibility
regain. A pure function `format_relative_or_absolute(ts: DateTime<Local>,
now: DateTime<Local>) -> String` implements B2.

```rust
fn format_relative_or_absolute(ts: DateTime<Local>, now: DateTime<Local>) -> String {
    let delta = now.signed_duration_since(ts);
    match delta {
        d if d < ChronoDuration::minutes(1) => "just now".into(),
        d if d < ChronoDuration::hours(1) => format!("{}m ago", d.num_minutes()),
        d if d < ChronoDuration::days(1) => ts.format("%-I:%M %p").to_string(),
        d if d < ChronoDuration::days(7) => ts.format("%a %-I:%M %p").to_string(),
        _ => ts.format("%Y-%m-%d %H:%M").to_string(),
    }
}
```

(The `%-I` form drops the leading zero on macOS/Linux. On Windows
use `%#I`. Branch via `cfg!(windows)` or use a small helper.)

## Tooltip

Hover tooltip uses an existing tooltip widget; pass it the ISO 8601
string `ts.format("%Y-%m-%d %H:%M:%S %:z").to_string()`.

## Missing-timestamp fallback

If `exchange_times(...).submitted_at` is `None`:
- Render "—" in the slot.
- Once per exchange id (track via a `HashSet<AIAgentExchangeId>` on
  the `TimestampTickService`), emit
  `log::warn!(exchange_id = ?id, "missing submitted_at timestamp")`.

Same path for `completed_at` when the exchange is no longer in
progress.

## Cancellation handling (B8)

The `ExchangeTimes` struct exposes `cancelled_at: Option<DateTime<Local>>`.
`DurationLabel` checks `cancelled_at` before `completed_at`; if
present, renders "cancelled at HH:MM • Xs" instead of "HH:MM • Xs".

## Test plan

### Unit tests (`app/src/ai/blocklist/agent_view/exchange_times_test.rs` — new)

- T1: `exchange_times` returns submitted-at from
  `AIAgentContext::CurrentTime`.
- T2: `exchange_times` returns completed-at from the latest message
  timestamp on the exchange.
- T3: In-progress exchange returns `completed_at: None`.
- T4: Cancelled exchange returns `cancelled_at: Some(...)`.
- T5: Exchange with no timestamps anywhere returns all-None.

### Unit tests (`app/src/ai/blocklist/agent_view/timestamp_widgets_test.rs` — new)

- T6: `format_relative_or_absolute(now - 30s, now) == "just now"`.
- T7: `format_relative_or_absolute(now - 5min, now) == "5m ago"`.
- T8: `format_relative_or_absolute(now - 3h, now) == "3:47 PM"`
  (matches `ts.format("%-I:%M %p")` for the input).
- T9: `format_relative_or_absolute(now - 3d, now) == "Mon 3:47 PM"`
  for an input dated Mon.
- T10: `format_relative_or_absolute(now - 30d, now) == "2026-04-04 15:47"`.
- T11: Duration formatter cases (B3): "<1s", "Xs", "Xm Ys", "Xh Ym".

### Integration tests (`app/src/integration_testing/agent_mode/timestamps_test.rs` — new)

- IT1: Submit a prompt; the user bubble immediately shows
  "just now" next to it.
- IT2: While the agent responds, the response bubble shows
  "running for Xs" with X incrementing once per second.
- IT3: On agent completion, the counter is replaced with
  "HH:MM • Xs".
- IT4: Hover the timestamp; tooltip shows the ISO 8601 form.
- IT5: With `agents.show_message_timestamps = false`, none of the
  widgets render and the bubble layout is identical to the current
  build (snapshot test).
- IT6: Restore a yaml conversation with messages dated yesterday;
  exchanges show "Mon 3:47 PM" not "just now."
- IT7: An exchange with stripped timestamps shows "—" and emits a
  single `log::warn!`.
- IT8: After 5 minutes (advance the clock in test), a "just now"
  exchange auto-promotes to "5m ago" without user action.

## Files touched

- `app/src/ai/agent/conversation.rs` — `pub(crate)` on
  `start_time_from_exchange_messages` (one-line visibility change).
- `app/src/settings/ai.rs` — new `show_message_timestamps` setting.
- `app/src/ai/blocklist/agent_view/exchange_times.rs` (new) —
  data-derivation helper.
- `app/src/ai/blocklist/agent_view/timestamp_widgets.rs` (new) —
  three label widgets.
- `app/src/ai/blocklist/agent_view/timestamp_tick_service.rs` (new)
  — single shared 1Hz timer.
- `app/src/ai/blocklist/agent_view/agent_message_bar.rs` —
  integrate widgets into the existing bubble metadata row.
- `app/src/ai/blocklist/agent_view/exchange_times_test.rs` (new)
  — T1–T5.
- `app/src/ai/blocklist/agent_view/timestamp_widgets_test.rs` (new)
  — T6–T11.
- `app/src/integration_testing/agent_mode/timestamps_test.rs` (new)
  — IT1–IT8.

## Out-of-scope follow-ups

- Per-token / per-tool-call timestamps.
- Cumulative agent CPU time / credits per exchange (overlaps
  #10000, #10052).
- Time-zone selection and 24h vs 12h preference.
- Exporting timestamps to the conversation yaml view.
- CLI agent (third-party) conversation timestamps — same
  `AIAgentExchange` model could be reused but the CLI agent surface
  has its own bubble layout.

## Open questions for maintainer review

1. Default: `true` (opt-out) or `false` (opt-in)? Spec
   recommends `true`; this is the most reversible decision.
2. Inside-bubble (recommended) vs. gutter rendering. Defers to
   design.
3. Tick service lifetime: registered on the conversation view or
   on a higher-level singleton? Per-conversation if conversations
   can render in multiple places; singleton if not.
4. ISO 8601 tooltip vs. a localized "Tuesday, May 4, 2026 at
   3:47:23 PM CDT" form. ISO is unambiguous; localized is friendlier.
