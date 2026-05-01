# APP-2267 — Agent long-running command stopped footer and controls
## Context
APP-2267 reports that when an agent starts monitoring a long-running requested command and the user interrupts it with Ctrl-C, the stopped task footer and credit footer/toolbelt do not appear. The non-delegated requested-command path already shows the footer, so the fix is scoped to the handoff path where the requested command becomes a CLI subagent interaction.
The relevant render gates live in the AI block output renderer. Suggestions and the response footer are only rendered for the exchange that owns the current conversation controls, when the conversation is no longer in progress and the request is active (`app/src/ai/blocklist/block/view_impl/output.rs:235`, `app/src/ai/blocklist/block/view_impl/output.rs:246`). Failed-output usage/debug footers use the same ownership concept (`app/src/ai/blocklist/block/view_impl/output.rs:1090`).
Before this change, that ownership was tied to the latest non-passive root-task exchange. That was too broad for agent-initiated long-running commands: `TerminalView::handle_cli_subagent_controller_event` removes the action-result/subagent-bootstrap AI block from rich content so the original requested-command block stays directly above the command block (`app/src/terminal/view.rs:5523`). The removed exchange remained in `AIConversation`, so footer ownership could move to an invisible internal exchange and the visible requested-command response would fail the footer/toolbelt gate.
Conversation status is a separate gate. `BlocklistAIController::cancel_conversation_progress` handles both response-stream cancellation and the later state where the response has finished but requested-command actions are pending or running (`app/src/ai/blocklist/controller.rs:2158`). In the latter case, the conversation must leave `InProgress` once there are no unfinished actions, otherwise footer rendering remains suppressed even if ownership is correct.
There was also a spacing special-case in the requested-command view: bottom margin was removed based on exchange history, assuming a later AI block would provide spacing. That assumption breaks for cancelled long-running commands and hidden internal follow-up exchanges. The only stable reason to remove the bottom margin is physical adjacency to the expanded command block (`app/src/ai/blocklist/inline_action/requested_command.rs:1482`).
## Proposed changes
### Latest visible exchange ownership
Add a semantic visible-exchange owner instead of reusing `last_non_passive_exchange()` globally. `AIConversation::latest_visible_exchange()` returns the newest root-task exchange that is not passive and is not hidden (`app/src/ai/agent/conversation.rs:1109`). `last_non_passive_exchange()` stays unchanged because other flows still depend on chronological non-passive behavior.
Expose that semantic check through `AIBlockModelHelper::is_latest_visible_exchange_in_root_task()` (`app/src/ai/blocklist/block/model/helper.rs:30`, `app/src/ai/blocklist/block/model/helper.rs:103`). Use it anywhere the latest visible/root response should own conversation controls:
- Response suggestions, references/response footer, usage footer, and failed-output debug footer in `app/src/ai/blocklist/block/view_impl/output.rs`.
- Imported-comments current-thread controls in `app/src/ai/blocklist/block/view_impl.rs:1104`.
- "Open all comments" disabled-state ownership in `app/src/ai/blocklist/block.rs:5298`.
This keeps hidden/internal exchanges in the conversation data for history, copy, persistence, and debugging while excluding them from UI ownership decisions.
### Hide internal CLI-subagent bootstrap exchanges
When a CLI subagent is spawned from a requested command, mark the action-result/subagent-bootstrap exchange hidden before removing its rich-content view (`app/src/terminal/view.rs:5532`). This preserves the existing visual continuity behavior while preventing an invisible exchange from becoming the footer/control owner.
Update blocklist filtering to skip hidden exchanges when deciding what to render or restore. `conversation_would_render_in_blocklist()` now delegates to `exchanges_for_blocklist()`, and that helper filters out hidden exchanges after task-type filtering (`app/src/terminal/view/blocklist_filter.rs:15`, `app/src/terminal/view/blocklist_filter.rs:22`). This prevents hidden internal exchanges from reappearing through restore/fork paths.
### Finish cancellation state after action-only cancellation
Keep the stream-cancellation fast path, but when there is no in-flight stream, cancel pending/running actions and then update the conversation status once no unfinished actions remain (`app/src/ai/blocklist/controller.rs:2158`). Normal user cancellation becomes `Cancelled`; the optimistic CLI-subagent completion reason remains `Success`. Follow-up cancellation for the same conversation still returns early so it does not prematurely terminate the thread.
### Requested-command spacing cleanup
Remove the history-based bottom-padding special-case. `RequestedCommandView::render()` now removes bottom margin only when the requested-command card is rendered immediately above its expanded command block (`app/src/ai/blocklist/inline_action/requested_command.rs:1482`, `app/src/ai/blocklist/inline_action/requested_command.rs:1497`). This preserves the attached-card visual design while keeping normal spacing for cancelled commands and cases without a follow-up AI block.
As cleanup from the same self-review, `RequestedCommandView::new()` now accepts only the `AIConversationId` it needs for subscriptions instead of the full client identifier bundle (`app/src/ai/blocklist/inline_action/requested_command.rs:256`).
## Testing and validation
Manual validation:
- Reproduced the APP-2267 flow where an agent starts monitoring a long-running requested command, then the user interrupts/takes over after the CLI subagent has started.
- Confirmed the stopped task footer and response footer/toolbelt appear after the fix.
- Confirmed the requested-command card remains visually attached to the expanded command block only in the physical-adjacency case.
Automated/command validation performed for the implementation:
- `cargo fmt --all --manifest-path /Users/vkodithala/Desktop/warp/warp.varoon-fix-credit-footer/Cargo.toml`
- `cargo check --manifest-path /Users/vkodithala/Desktop/warp/warp.varoon-fix-credit-footer/Cargo.toml -p warp --lib`
- `git --no-pager diff --check`
No new automated tests were added for this change because the requested implementation scope was minimal and manual UI validation was the acceptance criterion.
## Risks and mitigations
The main risk is changing what "latest exchange" means for unrelated passive-suggestion or history logic. The mitigation is to keep `last_non_passive_exchange()` unchanged and introduce a narrowly named visible-exchange helper used only by UI ownership gates.
Another risk is hiding too much conversation data. Hidden exchanges are still stored in `AIConversation`; the hidden bit is only consumed by footer eligibility and blocklist render/restore filtering, so debugging and persisted conversation state remain available.
