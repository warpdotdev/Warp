# Local-to-Cloud Handoff: `&` Entrypoint — Product Spec
Linear: [REMOTE-1558](https://linear.app/warpdotdev/issue/REMOTE-1558)
## Summary
Add a fast `&` input entrypoint for local agent users to start a cloud run from their current flow. Typing `&` followed by a prompt lets the user choose a cloud environment from the local agent footer, then pressing Enter either hands off the current local conversation to cloud or starts a fresh cloud run when there is no local conversation history.
## Problem
The existing local-to-cloud handoff flow is discoverable through the footer chip and `/move-to-cloud`, but it is not optimized for keyboard-first use. Users who already know they want the next prompt to run in the cloud need a prefix-style flow that feels as lightweight as `!` shell-mode input while preserving handoff context, environment selection, attachments, and error safety.
## Goals
- A user in a local fullscreen agent view can type `& query` to start a cloud run without manually opening a handoff pane and pressing Enter again.
- The `&` flow exposes environment selection before submission by reusing the same visual and interaction patterns as the existing cloud environment selector.
- `/move-to-cloud query` starts the same auto-run behavior as `& query`; `/move-to-cloud` without a query remains a compose/open action.
- The feature never silently drops the user's prompt or pending file/image attachments when handoff preparation fails.
- Existing local-to-cloud handoff behavior remains available for users who want to review or edit before submitting.
## Non-goals
- Redesigning the environment selector, environment management, or cloud-mode input UI.
- Adding an environment argument syntax to `/move-to-cloud`.
- Adding new selected text/block/document context serialization for cloud-mode submit. `&` matches current cloud-mode support by carrying prompt text and pending file/image attachments.
- Changing cloud-to-cloud handoff behavior.
- Bidirectional sync after handoff. Local and cloud conversations still diverge after the handoff point.
- Making pasted `& query` activate handoff mode. The prefix is an explicit typed shortcut.
## Figma
Figma: none provided.
## Behavior
### `&` handoff-compose mode
1. When the user is in a local fullscreen agent view and types `&` as the very first character of an empty input, the input enters handoff-compose mode.
2. `&` only activates handoff-compose mode when it is typed by the user as the first character. It does not activate when:
   - The user pastes text beginning with `&`.
   - The input has leading whitespace before `&`.
   - `&` appears anywhere other than the first character.
   - The user is not in a local fullscreen agent view.
   - The input is already in `!` shell mode.
3. After activation, the visible input behaves like `!` shell-mode input: the literal `&` is removed from the prompt text and rendered as a visible input indicator/chip. The prompt content sent to the cloud does not include the leading `&`.
4. While handoff-compose mode is active and the prompt is empty, the input shows explanatory ghost/hint text so the user understands what the prefix does. The hint should communicate both the destination and the escape affordance, e.g. "Start a cloud run, or backspace to exit cloud mode." Once the user has typed any prompt text, this ghost/hint text is hidden.
5. The empty-state affordance for handoff-compose mode mirrors `!` shell mode: when the prompt is empty, the user can press Backspace to exit cloud handoff mode. Exiting this way removes the `&` indicator and hides the transient environment selector.
6. While handoff-compose mode is active, the agent footer shows a transient environment selector in the existing footer left-side chip area. The selector uses the same visual style, menu behavior, focus behavior, labels, and environment-management affordance as the existing cloud-mode environment selector.
7. The transient environment selector is shown only for `&` handoff-compose mode. It is not shown for ordinary local agent prompts, the footer chip handoff flow, or `/move-to-cloud query`.
8. If the user selects an environment from the transient selector, that selection applies to the next `&` submission and persists as the user's last selected cloud environment, matching the existing environment selector behavior.
9. If the user does not explicitly select an environment in the transient selector, submission uses the normal cloud environment defaulting behavior. For a non-empty handoff source, touched-repo overlap may choose a better default after handoff preparation; for an empty source, the run uses the saved/default environment if one exists.
10. An explicit user selection in the transient selector always wins over any touched-repo overlap default discovered later.
11. If no environment exists or no environment is selected, `& query` can still start a cloud run without an environment, matching normal cloud-mode behavior.
12. Clearing the prompt back to empty exits handoff-compose mode automatically, removes the `&` indicator, and hides the transient environment selector.
13. Pressing Escape while in handoff-compose mode exits handoff-compose mode, removes the `&` indicator, hides the transient environment selector, and keeps the prompt text in the local input.
14. Normal prompt editing continues to work while in handoff-compose mode. Pending file/image attachments remain visible and editable until the prompt is submitted or the user removes them.
### Submitting `& query`
15. Pressing Enter in handoff-compose mode with a non-empty prompt starts the cloud path. The user is not required to press Enter again in the cloud pane.
16. The submitted cloud prompt includes:
   - The prompt text without the leading `&`.
   - Pending file/image attachments.
   - The environment chosen in the transient selector, when the user chose one.
17. If the active local conversation is empty, `& query` starts a normal fresh cloud run:
   - No local conversation fork is created.
   - No local-to-cloud snapshot is prepared.
   - The selected/default environment is used when available.
   - The cloud pane opens through the normal cloud-mode flow.
18. If the active local conversation is non-empty, idle, and has synced cloud conversation identity, `& query` starts a local-to-cloud handoff:
   - The cloud pane opens in the same pane stack as the source pane.
   - The source local agent view exits behind the cloud pane, matching the current handoff navigation behavior.
   - The cloud pane shows the source conversation history as the handoff context.
   - The cloud run starts automatically once the conversation fork, environment selection, and local snapshot preparation are ready.
19. If the active local conversation is non-empty but currently running or blocked, submission is blocked. Warp shows a toast explaining that handoff can only start from an idle conversation, leaves the user in the source local agent view, and preserves the prompt and pending file/image attachments.
20. If the active local conversation is non-empty and idle but cannot be handed off because it lacks synced cloud conversation identity, submission is blocked. Warp shows a toast, leaves the user in the source local agent view, and preserves the prompt and pending file/image attachments.
21. If handoff preparation fails before a cloud pane exists, such as a failed conversation fork request, Warp stays in the source local agent view, shows a toast, and preserves the prompt and pending file/image attachments.
22. If handoff preparation fails after a cloud pane has opened, including a snapshot failure that prevents the snapshot from being used, Warp leaves the handoff pane open with the prompt and pending file/image attachments intact, shows a toast, and lets the user retry manually from that pane.
23. Warp must not auto-start a non-empty local-to-cloud handoff without the intended local context. If the handoff cannot safely carry the forked conversation and usable prepared snapshot, it falls back to the retryable handoff pane state rather than silently starting a reduced-context cloud run.
24. Once a cloud run is successfully dispatched, the source input no longer shows the submitted prompt or pending attachments. The cloud pane becomes the active surface for that run.
25. Closing a cloud pane while auto-start preparation is still in progress cancels that pending auto-start from the user's perspective. The source local conversation is unaffected.
### `/move-to-cloud`
26. `/move-to-cloud` with no query keeps the existing compose behavior for non-empty eligible conversations: it opens a handoff compose pane where the user can review the restored conversation, choose an environment from the pane's existing environment selector, edit the prompt, and press Enter manually.
27. `/move-to-cloud` with no query from an empty local agent conversation opens a normal fresh cloud compose pane, because there is no local conversation context to hand off.
28. `/move-to-cloud query` starts the same auto-run behavior as `& query`.
29. `/move-to-cloud query` from an empty local agent conversation starts a normal fresh cloud run.
30. `/move-to-cloud query` from a non-empty local conversation attempts local-to-cloud handoff with automatic run start, using touched-repo overlap/default environment selection. It does not show the transient `&` environment selector before starting.
31. `/move-to-cloud query` follows the same blocked and failure behavior as `& query`: running/blocked conversations are blocked with a toast; missing synced cloud conversation identity is blocked with a toast; user input and pending file/image attachments are not silently discarded.
32. If `/move-to-cloud query` fails before a pane opens, the slash-command text or extracted prompt remains available in the source input so the user can retry or edit it. The exact text representation may follow existing slash-command input conventions, but the user's prompt must not be lost.
### Existing handoff chip
33. The existing "Hand off to cloud" footer chip remains a compose/open entrypoint. It does not auto-start a cloud run because the chip provides no prompt.
34. The chip continues to open the handoff compose pane for eligible non-empty local conversations and a fresh cloud compose pane for empty conversations.
35. The chip does not show the transient `&` environment selector in the source local footer. Environment selection happens in the opened cloud or handoff pane.
### Environment behavior
36. Environment selection has three priority levels for auto-started local-to-cloud handoff:
   1. User's explicit transient `&` selection, if present.
   2. Touched-repo overlap default, if a non-empty source conversation identifies one.
   3. Existing cloud environment defaulting behavior.
37. Auto-started fresh cloud runs from empty conversations use only the explicit transient `&` selection or existing cloud environment defaults, because there is no touched local conversation to score for overlap.
38. Environment selector labels, disabled states, keyboard navigation, menu closing behavior, and "New environment" behavior should match the existing selector wherever the selector appears.
### Focus, keyboard, and accessibility
39. Activating `&` mode keeps focus in the prompt editor so the user can continue typing immediately.
40. Opening the transient environment selector follows the existing selector's focus and keyboard behavior. Closing it returns focus to the prompt editor.
41. The `&` indicator and the handoff-compose hint text are part of the input chrome and should be understandable to screen readers at least as well as the existing `!` input indicator and mode hints.
42. Submitting a successful `& query` moves focus to the opened cloud pane, matching the existing handoff/cloud-mode navigation behavior.
### Invariants
43. The prompt sent to the cloud never includes the leading `&`.
44. `&` handoff-compose mode never changes normal AI-vs-shell classification semantics outside the active prompt.
45. `&` handoff-compose mode and `!` shell mode are mutually exclusive. Activating one must prevent or exit the other; the input must never show both indicators or dispatch both mode behaviors for the same prompt.
46. The user's explicit environment choice is never overwritten by automatic touched-repo overlap.
47. User-entered prompt text and pending file/image attachments are never silently dropped on blocked or failed handoff attempts.
48. Empty local conversations use fresh cloud-run behavior; non-empty eligible local conversations use local-to-cloud handoff behavior.
49. Local and cloud conversations remain independent after handoff. Continuing in one does not update the other.
