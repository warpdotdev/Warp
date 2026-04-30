# Renaming Conversations — Product Spec
GitHub issue: https://github.com/warpdotdev/warp/issues/8642
Figma: none provided
## Summary
Let users set a custom title for any agent conversation. The custom title overrides the auto-generated conversation title everywhere a conversation's display title appears: vertical-tabs primary line, the conversation list panel, the command palette conversation search, and the macOS / Windows window title that follows the active conversation. The auto-generated title (`task.description()`, falling back to the first user query) is preserved underneath, so clearing the custom title returns to the previous behavior. Custom titles persist across restarts on the same machine.
## Problem
The conversation card on the agent panel renders three lines: a small label ("WARP" / "OZ" / agent kind), a large primary title, and a working-directory line. The primary title is the only line on the card the user cannot change.
- The working directory comes from the user's `cd`.
- The agent badge comes from the active conversation's agent kind.
- The primary title is fully derived: it's the agent's auto-generated `task.description()`, falling back to the first user prompt, falling back to a rare internal default.
This is a recurring complaint — a long-open feature request (#8642) plus three closed near-duplicates (#8860, #8856, #8697). Users want to keep multiple agent conversations open and recognize them at a glance, but the only line tall enough to read is the one they can't influence. A user who finishes a piece of work and starts another conversation in the same context loses the older conversation in a sea of agent-generated descriptions that don't reflect how they actually think about each thread.
A related setting (`use_latest_user_prompt_as_conversation_title_in_tab_names`) already lets users swap which derived source is shown — auto-generated description vs. latest user prompt — but neither source is user-editable, and that setting applies globally rather than per-conversation. Renaming a single conversation is a different need: a per-conversation override that the user owns.
## Goals
- Add a per-conversation user-editable title that overrides the auto-generated conversation title when set.
- Surface a Rename action from two entry points: (a) the conversation list panel overflow menu, and (b) right-click on the agent card in vertical tabs.
- Persist user-set titles locally so they survive restarts and conversation restore on the same machine.
- Preserve the auto-generated title underneath so clearing the custom title falls back to the existing behavior unchanged.
- Apply the custom title consistently anywhere the conversation's display title is shown (vertical tabs, conversation list, command palette, window/tab title).
- Leave the existing `use_latest_user_prompt_as_conversation_title_in_tab_names` setting untouched: when a user-set title exists it wins; otherwise the existing setting continues to choose between the two derived sources.
## Non-goals
- Syncing the user-set title across machines or to other clients viewing the same shared session. The custom title is local-only in this iteration. Other machines / shared-session viewers continue to see the auto-generated title.
- Renaming ambient-agent (task-only) conversations that don't have a backing local `AIConversation` record. The Rename entry is shown but disabled for these, mirroring how Delete is disabled today.
- Renaming a conversation while it is actively streaming a response. We don't restrict the action — but the behavior is just "save the new title; the next stream completion does not overwrite it" because the auto-generated title is no longer the source of truth for display.
- Inline rename (e.g. double-click on the title to edit it in place). Out of scope; the dialog covers the entry-point UX in this iteration.
- Adding a "rename" slash command. Out of scope; can be a follow-up.
- Search treating user-set titles as a separate index. The existing conversation search reads through the same `title()` accessor and will pick up the override automatically.
- Migrating any existing manually-set tab name (`tab_settings.tab_name`) into the new conversation title. Tab names and conversation titles remain independent; the existing tab-rename UX is unchanged.
## Behavior
1. **Default behavior is unchanged.** A conversation with no user-set title continues to display the auto-generated title (auto `task.description()` → first user prompt → internal default). Vertical-tabs, conversation list, command palette, and window/tab title all show the same string they show today.
2. **Conversation list overflow menu adds a "Rename" item.** In the conversation list panel (left panel), opening the overflow menu (kebab) on a conversation row exposes Rename above the existing Delete item. Order from top to bottom of the menu is: Share conversation (when shareable) → Rename → Fork in new pane → Fork in new tab → Delete.
3. **Right-click on the agent card adds a "Rename" item.** In vertical tabs, right-clicking the card body of an agent conversation (any card whose primary line currently comes from the conversation title) opens a context menu containing at minimum a Rename item. Plain-terminal cards (no associated conversation) do not show this entry.
4. **Rename opens a small modal dialog.** The dialog seeds a single-line text input with the current title (the user-set title if one exists, otherwise the auto-generated title). It has Save and Cancel buttons. Pressing Enter saves; pressing Esc cancels. Save is disabled while the input matches the seeded value.
5. **Saving a non-empty value sets the user-set title.** Trim leading/trailing whitespace before persisting. Internal whitespace and emoji are preserved as-is. Length cap: 200 characters; the input truncates input above that and shows no error.
6. **Saving an empty / whitespace-only value clears the user-set title.** The conversation reverts to the existing derived chain. The input shows the auto-generated title as a placeholder so users can see what they're falling back to.
7. **A successful save updates every display surface immediately.** All of the following reflect the new title without a restart: vertical-tabs primary line, the conversation list row text, the command palette conversation search results, the OS window title (when this conversation is active), and any tab title that derives from the conversation title.
8. **Custom titles persist across restarts.** Closing and reopening Warp restores the user-set title. Restoring a conversation from history (re-opening it in a tab via the conversation list) also restores the user-set title.
9. **Forking does not inherit the source's user-set title.** When a conversation is forked, the new conversation starts with no user-set title of its own and uses the forked source's auto chain — i.e. the user explicitly renames the fork if they want a custom name on it. Rationale: fork means "branch the work," and a name like "Fix login bug" rarely fits both branches.
10. **Ambient-agent (task-only) conversations show the Rename entry as disabled.** The disabled item carries the same tooltip pattern Delete uses for ambient conversations today ("Ambient agent conversations cannot be renamed"). This applies in both the conversation list overflow menu and the vertical-tabs context menu.
11. **The Rename entry is hidden in shared-session viewer mode.** When a user opens a conversation as a shared-session viewer (`is_viewing_shared_session = true`), the local `AIConversation` exists but the conversation is not theirs. Rename is hidden in both menus.
12. **The existing `use_latest_user_prompt_as_conversation_title_in_tab_names` setting is unchanged.** When a user-set title exists, it overrides everything regardless of the setting. When no user-set title exists, the setting continues to choose between the auto description and the latest user prompt.
13. **Cloud sync does not write the user-set title.** Other machines or shared-session viewers continue to see the auto-generated title. This is an explicit non-goal in this iteration; following up to sync is a separate change.
14. **No new keyboard shortcut.** This iteration does not add a global "rename current conversation" shortcut. Follow-up if it proves useful.
15. **No telemetry change.** This iteration does not add a new telemetry event for renames. Follow-up if maintainers request it.
16. **Empty / whitespace input is treated identically to clearing.** A user who selects all and deletes, then hits Save, lands in the same state as a user who never set a title.
