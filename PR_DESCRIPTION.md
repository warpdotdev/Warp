## Description

The "Runs" agent management view rebuilds its entire card list whenever any conversation's status changes. Most of those events are visually no-ops (re-emitted statuses or status changes that don't cross the active filter), but subscribers can't tell because the event carries no detail.

This PR is the first of two and only changes the shape of the conversation status update event. Subscribers continue to behave exactly as before. The follow-up PR uses the new payload to skip rebuilds in the common cases.

Concretely:
- `BlocklistAIHistoryEvent::UpdatedConversationStatus` now carries the previous and new `ConversationStatus`. Restoration events report `prev_status: None` since restoration doesn't change the underlying status.
- `AgentConversationsModelEvent::ConversationUpdated` now carries a `conversation_id` and a typed `ConversationUpdateKind` of either `Restored` or `StatusSet { prev_filter, new_filter }`. Filter buckets are precomputed at emission time so subscribers don't have to recompute membership.
- A small helper `conversation_status_filter` maps `ConversationStatus` to its `StatusFilter` bucket.
- All existing subscribers were updated to the new event shape with `{ .. }` patterns and ignore the new fields. The variant is annotated with `#[allow(dead_code)]` until the follow-up PR lands.

## Testing

- Added/updated four unit tests on the model: `Restored` emission, `StatusSet` transitions across filter buckets, same-bucket re-emission, and the `ConversationStatus → StatusFilter` mapping.
- Existing `agent_conversations_model` tests pass against the new payload.
- `cargo check`, `cargo fmt`, `cargo clippy` all clean.
- No user-visible behavior change, so no manual smoke test needed.

## Server API dependencies

No server dependencies — purely client-side event-shape changes.

## Agent Mode

- [x] Warp Agent Mode - This PR was created via Warp's AI Agent Mode

## Changelog Entries for Stable

(No changelog entry — no user-visible behavior change. The follow-up PR carries the changelog entry.)
