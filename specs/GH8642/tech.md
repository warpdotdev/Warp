# Renaming Conversations — Tech Spec
Product spec: `specs/GH8642/product.md`
GitHub issue: https://github.com/warpdotdev/warp/issues/8642
## Context
The "primary line" on every agent card — the largest, most-readable text on the conversation card both in vertical tabs and in the conversation list panel — is the conversation's display title. It is fully derived today, with no user-editable input.
Display title resolution lives in `AIConversation::title()`:
```rust path=/Users/joey/dev/warp/app/src/ai/agent/conversation.rs start=1129
/// Get the auto-generated title of the given conversation
/// (falling back to the first query if the title is empty).
/// Get the title of the given conversation.
/// Priority: auto-generated task description > initial query > fallback_display_title.
pub fn title(&self) -> Option<String> {
    self.task_store
        .root_task()
        .and_then(|task| {
            if task.description().is_empty() {
                self.initial_query()
            } else {
                Some(task.description().to_owned())
            }
        })
        .or_else(|| self.fallback_display_title.clone())
}
```
The `fallback_display_title` field at `app/src/ai/agent/conversation.rs:208` is the only existing override mechanism, but it is set in a couple of internal paths only and is never user-editable; it is also a *fallback* (lowest priority), not an *override*. To support user rename we need a *highest-priority* per-conversation override.
The vertical-tabs primary line picks up `AIConversation::title()` through `selected_conversation_display_title` in the renderer at `app/src/workspace/view/vertical_tabs.rs:3074`, and `terminal_primary_line_data` (`vertical_tabs.rs:2951-2998`) chooses `conversation_display_title` over `terminal_title` and `last_completed_command` for any card backed by an agent conversation. The conversation list panel reads the same accessor through `ConversationEntry::title(ctx)` (called in `view.rs:1063-1066` for Delete; the analogous Rename code path will reuse this). The command palette conversation search reads it via `app/src/search/command_palette/conversations/data_source.rs:28`. So once `title()` returns the user-set value, every display surface picks it up automatically — no per-surface plumbing required.
Persistence for a conversation row already uses a JSON blob column (`agent_conversations.conversation_data`) carrying `AgentConversationData`, with every newer field added under `#[serde(default, skip_serializing_if = "Option::is_none")]` so legacy rows continue to deserialize and `None` values do not bloat the on-disk payload. See `crates/persistence/src/model.rs:1009-1044` and the `last_event_sequence` precedent at `:1042` plus its three roundtrip / legacy / skip-on-none unit tests at `:1340-1389`. We will follow the same pattern for the new field — no schema migration is needed.
The conversation list overflow menu currently exposes Share / Fork / Delete only (`app/src/workspace/view/conversation_list/view.rs:902-984`). It already gates each item on `is_ambient_agent_conversation` and supplies a tooltip when an item is disabled. We will mirror that pattern for Rename.
The vertical-tabs cards do not have a context menu today on the agent-card body — there is `ToggleVerticalTabsPaneContextMenu` in `WorkspaceAction` at `app/src/workspace/action.rs:126-130`, but it targets a pane locator and is wired to a different surface (the kebab on the card), not a right-click on the card body. The simplest option is to plumb a separate `ToggleAgentCardContextMenu` action and dispatch it from a `MouseListener` on the card's primary-line container in `vertical_tabs.rs`.
A related but separate setting already exists: `tab_settings::TabSettings::use_latest_user_prompt_as_conversation_title_in_tab_names` (`app/src/workspace/view/vertical_tabs.rs:3026-3032`, defined under `specs/APP-4080/`). It picks between two derived sources — auto title vs. latest user prompt. It is not user-editable per conversation. Our user-set title sits **above** that setting in the priority chain, so the setting continues to apply only when no user-set title exists.
The relevant shared event/action types we will extend:
- `BlocklistAIHistoryEvent` enum at `app/src/ai/blocklist/history_model.rs:2059-2185`, currently has variants such as `UpdatedConversationMetadata`, `UpdatedAutoexecuteOverride`, `UpdatedConversationArtifacts`. We will add `UpdatedConversationTitle`.
- `WorkspaceAction` enum at `app/src/workspace/action.rs:99-229+`. We will add `SetConversationUserTitle` and `ToggleAgentCardContextMenu`.
- `ConversationListViewAction` enum at `app/src/workspace/view/conversation_list/view.rs:111-143`. We will add `RenameConversation`.
- `Event` enum (`Event::ShowDeleteConfirmationDialog { … }`) at `app/src/workspace/view/conversation_list/view.rs:145-152`. We will add `Event::ShowRenameDialog { … }`.
The dialog plumbing has a clean precedent: `app/src/workspace/delete_conversation_confirmation_dialog.rs` is mounted from `workspace/view.rs` and listens for `Event::ShowDeleteConfirmationDialog`. We will add a sibling `rename_conversation_dialog.rs` and follow the same mount pattern.
See product spec for user-visible behavior.
## Proposed changes
### 1. New `user_set_title` field on `AIConversation`
In `app/src/ai/agent/conversation.rs`:
- Add `user_set_title: Option<String>` to the `AIConversation` struct (alongside `fallback_display_title` at `:208`). Initialize to `None` in `AIConversation::new` (`:248`) and `AIConversation::new_restored` (`:287`+).
- Add accessor `pub fn user_set_title(&self) -> Option<&str>`.
- Add setter:
```rust path=null start=null
pub fn set_user_title(
    &mut self,
    title: Option<String>,
    terminal_view_id: EntityId,
    ctx: &mut ModelContext<BlocklistAIHistoryModel>,
) {
    let normalized = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(200).collect::<String>());
    if normalized == self.user_set_title {
        return;
    }
    self.user_set_title = normalized.clone();
    self.write_updated_conversation_state(ctx);
    ctx.emit(BlocklistAIHistoryEvent::UpdatedConversationTitle {
        terminal_view_id,
        conversation_id: self.id,
        title: normalized,
    });
}
```
- Update `AIConversation::title()` (currently at `:1133`) to make `user_set_title` the highest-priority source:
```rust path=null start=null
pub fn title(&self) -> Option<String> {
    if let Some(t) = self.user_set_title.as_ref() {
        return Some(t.clone());
    }
    self.task_store
        .root_task()
        .and_then(|task| {
            if task.description().is_empty() {
                self.initial_query()
            } else {
                Some(task.description().to_owned())
            }
        })
        .or_else(|| self.fallback_display_title.clone())
}
```
The `fallback_display_title` chain is left byte-for-byte untouched.
### 2. Persistence — extend `AgentConversationData`
In `crates/persistence/src/model.rs:1009-1044`, add:
```rust path=null start=null
#[serde(default, skip_serializing_if = "Option::is_none")]
pub user_set_title: Option<String>,
```
This follows the exact pattern of `last_event_sequence` at `:1042` — backwards-compatible with legacy rows and small on-disk footprint when `None`. No DB schema migration is required because `agent_conversations.conversation_data` is a JSON column.
Add three unit tests in the existing `tests` mod (`:1335-1389`), mirroring the `agent_conversation_data_roundtrips_last_event_sequence` / legacy / skip-on-none trio:
- `agent_conversation_data_roundtrips_user_set_title`
- `agent_conversation_data_deserializes_legacy_payload_without_user_set_title`
- `agent_conversation_data_skips_serializing_none_user_set_title`
### 3. Conversation restore + write paths
In `AIConversation::new_restored` (`app/src/ai/agent/conversation.rs:287`+):
- Extend the destructured tuple at `:349-410` to include `user_set_title` from `AgentConversationData`. Default to `None` in the `else` branch where `conversation_data` is absent.
- Set `self.user_set_title = user_set_title;` alongside the other restored fields when constructing `AIConversation`.
In whatever path materializes `AgentConversationData` from an in-memory `AIConversation` for persistence (the constructor sites for `AgentConversationData { … }` in `app/src/ai/blocklist/history_model.rs`), add `user_set_title: conv.user_set_title().map(str::to_string),`.
### 4. New `BlocklistAIHistoryEvent::UpdatedConversationTitle` variant
In `app/src/ai/blocklist/history_model.rs:2059-2185`, add:
```rust path=null start=null
UpdatedConversationTitle {
    terminal_view_id: EntityId,
    conversation_id: AIConversationId,
    title: Option<String>,
},
```
Subscribers that already react to `UpdatedConversationMetadata` for refresh purposes (e.g. the conversation list view at `:204-211`, vertical-tabs at the call sites in `view/vertical_tabs.rs`) will gain matching handler arms that just call `ctx.notify()` to re-render. We do not need bespoke per-handler logic because every display surface reads through `AIConversation::title()`.
### 5. New `BlocklistAIHistoryModel::set_conversation_user_title` helper
In `app/src/ai/blocklist/history_model.rs`, add a model-level helper colocated with `toggle_autoexecute_override` (`:1818-1831`):
```rust path=null start=null
pub fn set_conversation_user_title(
    &mut self,
    conversation_id: AIConversationId,
    title: Option<String>,
    ctx: &mut ModelContext<Self>,
) {
    let Some(conversation) = self.conversations_by_id.get_mut(&conversation_id) else {
        return;
    };
    let Some(terminal_view_id) =
        self.terminal_view_id_for_conversation(&conversation_id)
    else {
        return;
    };
    conversation.set_user_title(title, terminal_view_id, ctx);
    // Also mirror onto the cached metadata so the conversation list reflects
    // the rename for not-currently-active conversations.
    if let Some(meta) = self.all_conversations_metadata.get_mut(&conversation_id) {
        meta.user_set_title = conversation.user_set_title().map(str::to_string);
    }
}
```
This requires adding a parallel `user_set_title: Option<String>` to `AIConversationMetadata` (already filtered for `is_ambient_agent_conversation()` in `:1887-1893`) so the conversation list panel still shows the override when the conversation is no longer in `conversations_by_id`. Initialize it from `AIConversation::user_set_title()` in `AIConversationMetadata::from(&AIConversation)`.
### 6. New `ConversationListViewAction::RenameConversation` + dialog event
In `app/src/workspace/view/conversation_list/view.rs`:
- Extend `ConversationListViewAction` (`:111-143`) with `RenameConversation { conversation_id: ConversationOrTaskId }`.
- Extend `Event` (`:145-152`) with:
```rust path=null start=null
ShowRenameDialog {
    conversation_id: AIConversationId,
    current_title: String,
    seed_is_user_set: bool,
    placeholder_auto_title: String,
    terminal_view_id: Option<EntityId>,
},
```
- In the overflow-menu construction (`:902-984`), insert a `MenuItemFields::new("Rename")` item between Share and Fork, with the same `with_disabled(is_ambient_agent_conversation)` + tooltip pattern Delete uses for ambient conversations. Skip the menu item entirely (do not render at all) if the underlying `AIConversation::is_viewing_shared_session()` is true (per product spec invariant 11).
- Add a `RenameConversation` arm in `handle_action` (`:847-1156`). Resolve the current title via `view_model.get_item_by_id(conversation_id, ctx).title(ctx)` (mirroring the Delete code path at `:1063-1066`) and the auto-only title via a new `ConversationEntry::auto_title(ctx)` accessor that calls `AIConversation::title()` while temporarily ignoring the `user_set_title` override (this is just `task.description()` → `initial_query()` → `fallback_display_title`, exposed as a new helper on `AIConversation` named `auto_generated_title_for_display()`). Emit `Event::ShowRenameDialog`.
### 7. New `WorkspaceAction::SetConversationUserTitle`
In `app/src/workspace/action.rs:99-229+`, add:
```rust path=null start=null
SetConversationUserTitle {
    conversation_id: AIConversationId,
    title: Option<String>,
},
```
Wire it in `app/src/workspace/view.rs` next to other AI-conversation workspace-action handlers: look up the model via `BlocklistAIHistoryModel::handle(ctx)`, call `set_conversation_user_title`, and trigger an active-views refresh (`ActiveAgentViewsModel::handle(ctx).update`) so the live list-view selections also re-render.
### 8. New `rename_conversation_dialog.rs`
Add `app/src/workspace/rename_conversation_dialog.rs`, modeled on `app/src/workspace/delete_conversation_confirmation_dialog.rs` (which is the existing dialog mounted from `workspace/view.rs`). The new file owns:
- A small `RenameConversationDialog` view with a single-line `EditorView` (mirroring the conversation-list search editor at `:213-231`), seeded with `current_title`, with `placeholder_auto_title` shown as placeholder text whenever the input is empty.
- Save / Cancel buttons. Enter dispatches Save; Esc dispatches Cancel.
- On Save: dispatch `WorkspaceAction::SetConversationUserTitle { conversation_id, title }` where `title` is `None` if the trimmed input is empty, else `Some(trimmed)`.
- On Cancel: close the dialog (no-op).
Mount it in `workspace/view.rs` next to the existing `DeleteConversationConfirmationDialog`. Subscribe to the `Event::ShowRenameDialog` from `ConversationListView` (the existing `subscribe_to_view` pattern at the conversation-list mount site) and, on receipt, set the dialog's target and focus it — the same mechanism Delete already uses.
### 9. Vertical-tabs right-click context menu
In `app/src/workspace/view/vertical_tabs.rs`, in the agent-card render path that uses `terminal_primary_line_data` (`:2951-2998`), wrap the existing card body in a `MouseListener` (or extend the existing one) so that a right-click dispatches a new action:
```rust path=null start=null
WorkspaceAction::ToggleAgentCardContextMenu {
    conversation_id: AIConversationId,
    position: Vector2F,
}
```
Only attach this listener for cards whose `terminal_agent_text(...).conversation_display_title.is_some()` and whose underlying `AIConversation::is_viewing_shared_session()` is `false`. Plain-terminal cards keep their existing right-click behavior.
The new context menu has at minimum a Rename item that emits `Event::ShowRenameDialog` (re-using the same dialog mounted in `workspace/view.rs`). The menu is rendered via the existing `Menu<WorkspaceAction>` infrastructure used by `ToggleVerticalTabsPaneContextMenu` (`workspace/action.rs:126-130`); follow that pattern for state ownership and dismiss-on-outside-click. The disabled / hidden semantics are the same as in change 6 (ambient = disabled with tooltip; shared-session viewer = hidden).
### 10. Fork preserves auto-only behavior
In the existing fork code path (search for `forked_from_server_conversation_token` + the `AgentConversationData` constructor used by forking, both already present in `history_model.rs` per the existing `insert_forked_conversation_from_tasks` at `:2003-2033`), explicitly **do not** copy `user_set_title` into the new conversation. The forked `AgentConversationData` arrives from the server with `user_set_title = None` because we never sync it to the server, but we should also assert in code that the forked-from-server token resets the field to `None`. Add a one-line `conversation.user_set_title = None;` in `insert_forked_conversation_from_tasks` for clarity / regression safety.
### 11. Hooking into `selected_conversation_display_title` (sanity check)
The vertical-tabs primary line and search fragment paths (`vertical_tabs.rs:2916-2949` and `:3059-3086`) read through `selected_conversation_display_title`/`title()`, so changes 1 and 2 are sufficient for those surfaces — no additional plumbing.
The conversation list (`conversation_list/view.rs`) reads via the `ConversationEntry::title(ctx)` accessor from `view_model`. Confirm that `ConversationListViewModel`'s sort key for the Active vs. Past sections is unaffected by a rename — it's keyed on `last_modified_at`, not title — so renames do not reorder rows.
The OS window title and tab title routing through `tab.rs:811-1187` already reads `selected_conversation_display_title`, so no additional changes are needed there.
The command palette conversation search at `app/src/search/command_palette/conversations/search_item.rs:84-180` reads `c.title()` for both display and match; no additional changes are needed.
## Testing and validation
Each numbered invariant in `specs/GH8642/product.md` maps to at least one test or manual step below.
### Unit tests
- `app/src/ai/agent/conversation_tests.rs` (sibling-file pattern per WARP.md) — `title_prefers_user_set_title_over_auto_description`. Builds an `AIConversation` with a non-empty `task.description()`, asserts `title()` returns the description, then calls `set_user_title(Some("My custom"))`, asserts `title()` returns `"My custom"`, then calls `set_user_title(None)`, asserts `title()` returns the description again. Guards invariants 1 and 6.
- `app/src/ai/agent/conversation_tests.rs` — `set_user_title_normalizes_whitespace_and_caps_length`. Saves `"   "` → cleared. Saves `"  hello  "` → `"hello"`. Saves a 250-character string → 200 characters preserved (`hello…<truncated>`). Saves the same value twice → second call is a no-op (no event emitted). Guards invariants 5 and 16.
- `crates/persistence/src/model.rs` `tests` mod — `agent_conversation_data_roundtrips_user_set_title`, `agent_conversation_data_deserializes_legacy_payload_without_user_set_title`, `agent_conversation_data_skips_serializing_none_user_set_title`. Mirror the existing `last_event_sequence` trio. Guards invariant 8.
### Render / model tests
- `app/src/workspace/view/vertical_tabs_tests.rs` — extend the existing test points around `:118-237` with a case where `user_set_title` is set: assert the primary-line text returned by `terminal_primary_line_data` is the user-set title, that the search fragments include the user-set title rather than the description, and that with the existing `use_latest_user_prompt_as_conversation_title_in_tab_names` flag toggled the user-set title still wins. Guards invariants 1, 7, 12.
- `app/src/ai/agent_conversations_model_tests.rs` — `rename_updates_conversation_list_entry_title`. Insert a conversation with a non-empty description, call `set_conversation_user_title`, assert the model emits `BlocklistAIHistoryEvent::UpdatedConversationTitle` and that the conversation list entry's `title(ctx)` returns the new title. Guards invariants 2 and 7.
- `app/src/workspace/view/conversation_list/view_tests.rs` (or extend the closest existing tests file) — assert the overflow menu now contains a Rename item between Share and Fork, that it's disabled for `ConversationOrTaskId::TaskId(_)` (ambient agents) with the documented tooltip, and that it's hidden when `is_viewing_shared_session()` is `true`. Guards invariants 2, 10, 11.
- New `app/src/workspace/rename_conversation_dialog_tests.rs` — assert that Save with empty input dispatches `SetConversationUserTitle { title: None }`; Save with whitespace dispatches `None`; Save with a real string dispatches `Some(trimmed)`; Esc dispatches no action. Guards invariants 4, 5, 6, 16.
### Integration / manual validation
- Start an agent conversation; confirm the auto title appears on the agent card. Open the conversation list overflow menu and confirm the new Rename item is visible above Delete. Right-click on the agent card body and confirm the Rename context menu appears. (Invariants 2, 3, 7.)
- From either entry point, open the dialog. Confirm the input is seeded with the current title, the placeholder displays the auto-generated title when the input is cleared, Save is disabled until the value differs, Enter saves, Esc cancels. (Invariant 4.)
- Save a custom title; confirm it appears on the agent panel card, the conversation list row, the command palette conversation search, and the OS window title. Restart Warp; confirm it persists. Clear the title; confirm fallback. (Invariants 5, 6, 7, 8.)
- Fork the renamed conversation; confirm the fork uses the auto-generated title, not the parent's user-set title. Rename the fork independently and confirm both retain their respective titles. (Invariant 9.)
- Open an ambient-agent conversation; confirm the Rename entry is visible but disabled with the documented tooltip in both menus. (Invariant 10.)
- Open a shared-session viewer for a conversation owned by another user; confirm Rename is hidden in both menus. (Invariant 11.)
- Toggle `Use latest user prompt as conversation title in tab names` on and off; confirm the user-set title wins in either state, and the setting still works for unrenamed conversations. (Invariant 12.)
## Risks and mitigations
### Risk: Stale per-card cache shows the old title after rename
The vertical-tabs renderer reads `terminal_agent_text(...)` and computes the primary line on every render. As long as the `UpdatedConversationTitle` event triggers `ctx.notify()` on the relevant view, the next render picks up the new title. The conversation list view already subscribes to model events at `view.rs:204-211` and re-syncs its list items; we will mirror the wiring in `vertical_tabs.rs` if the existing subscription doesn't already cover it (it does, via `BlocklistAIHistoryModel`-handle subscription at the workspace view).
Mitigation: explicit render-test coverage in `vertical_tabs_tests.rs` confirms the new title appears in the primary line on the next render frame.
### Risk: Rename racing with auto-title regeneration
The agent updates `task.description()` over time. Without an override, that update naturally surfaces in the card. With a user-set title, the update would be silently masked. This is the desired behavior (the user explicitly chose their title), but it is also a confusing failure mode if a user forgets they renamed something.
Mitigation: the rename dialog's placeholder always shows the current auto-generated title, so a user clearing their input can see what the auto fallback would say. No further mitigation needed for this iteration; we can revisit if user feedback suggests otherwise.
### Risk: Cloud-synced viewers see different titles
Two clients (or a shared-session viewer + the owner) will see different titles for the same conversation: the owner's machine shows the user-set title; everyone else sees the auto title.
Mitigation: explicit non-goal in the product spec. Follow-up to add a server field on `ServerAIConversationMetadata` is straightforward and can ship behind a separate feature flag once we have product alignment on the cloud-sync UX.
### Risk: Title length explosion / pathological input
Long titles or escape sequences could break the renderer's truncation/clipping. The vertical-tabs renderer already trims/clips long primary text via `Clipped` and the existing `Text` element handles overflow.
Mitigation: 200-character cap in `set_user_title`. Whitespace trimming on save. The renderer's existing clipping covers anything that gets through.
### Risk: Forking + rename order produces unexpected duplicates
A user who renames the parent and then forks: per spec the fork uses the auto title. A user who forks first and then renames the parent: parent change does not propagate to the fork. Both are intentional; both are easy to reason about.
Mitigation: explicit `user_set_title = None;` in `insert_forked_conversation_from_tasks` plus the no-server-sync invariant guarantee this is the only outcome.
## Follow-ups
- Cloud sync of user-set titles via `ServerAIConversationMetadata`. Behind a feature flag, with conflict resolution that prefers the most-recent client write.
- Inline rename (double-click on the agent card primary line to edit in place), as a UX shortcut for the dialog flow.
- Keyboard shortcut for "rename current conversation" in agent view.
- Telemetry event for renames (count, source = list-menu / vertical-tabs-context-menu, characters changed).
- Optional behavior: when an agent generates a much-longer or much-different `task.description()` after a user rename, surface a small "auto title differs" affordance in the Rename dialog so the user can re-adopt the new auto title with one click.
