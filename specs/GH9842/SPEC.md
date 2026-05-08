# Spec: Editable prompt suggestions (GH-9842)

## Problem

Prompt suggestions in Warp are all-or-nothing: clicking one
sends the suggested prompt as-is. There's no way to insert it
into the input as a *draft* the user can edit, refine, and then
send themselves. This makes suggestions feel rigid for the
common case where the suggestion is "almost right."

## Goal

Add an alternate affordance on each prompt-suggestion entry that
inserts the suggestion into the agent input as editable text
without dispatching it. The user edits, then submits manually.

## Behavior contract

- B1. Each prompt-suggestion entry exposes two affordances. **Both
  entry points are required in V1** — shipping only one is
  insufficient. They dispatch the same `InsertPromptSuggestionAsDraft`
  action:
  - **Primary click (existing):** sends the suggestion as-is.
    Matches today's interaction; no pixel-parity requirement, but
    the existing card padding and icon-button base style are
    preserved.
  - **New: insert-as-draft entry points (V1 ships BOTH):**
    - **B1a. Edit icon.** A small pencil glyph rendered at the
      trailing edge of each suggestion card. See B7 for affordance
      details.
    - **B1b. Modifier-click.** Alt-click on macOS and Linux,
      Ctrl-click on Windows. Plain (no modifier) click continues
      to send.
- B2. Existing-draft handling. Insert-as-draft REPLACES any existing
  draft in the agent input. If the input is empty, the suggestion
  text becomes the input. If the input has user text (length > 0),
  show a confirm dialog: *"Replace your current draft with this
  suggestion?"* with **[Replace]** and **[Cancel]** buttons.
  - Replace clears the existing draft and inserts the suggestion
    text. The caret lands at the end of the inserted text.
  - Cancel makes no change to the input or draft.
  - The choice is NOT persisted across inserts ("don't ask again"
    is intentionally absent in V1). Each insert with non-empty
    draft prompts again.
  - A future TECH may relax to caret-insert or append-with-newline;
    V1 is replace-with-confirm only.
- B3. Cursor lands immediately after the inserted suggestion text.
- B4. The insert dispatches NO send action. The user must
  manually send via Enter / Cmd-Enter as usual.
- B5. The existing inline-banner prompt-suggestion model
  (`app/src/terminal/view/inline_banner/prompt_suggestions.rs`)
  is the data source; no schema change.
- B6. Telemetry. Insert-as-draft reuses the existing
  `prompt_suggestion.action` event (same category, same surface
  payload, same private-field rules) with a new `action_type`
  enum value `insert_as_draft` alongside the existing `send`.
  - `category = "suggestion"` (existing).
  - `source` = the existing source field already populated by the
    suggestion's origin (banner, autocomplete, etc.) — no new field.
  - The suggestion category is derived only from existing
    `PromptSuggestion` fields, in this order:
    `static_prompt_suggestion_name = Some(name)` maps to
    `static:{name}`; otherwise `coding_query_context.is_some()` maps
    to `coding_query`; otherwise `should_start_new_conversation`
    maps to `new_conversation` when true and `follow_up` when false.
    Never derive the category from `prompt`, `label`, `id`, or the
    user's edited result.
  - No new event type, no new private payload fields.
- B7. Edit-icon affordance & accessibility.
  - Rendered as a 16×16 pencil glyph at the trailing edge of each
    suggestion card.
  - Keyboard-focusable; participates in the suggestion list's
    existing roving-focus / `tabindex` model.
  - `aria-label="Edit suggestion before sending"`.
  - On focus or hover, surfaces a tooltip: *"Click to send,
    [⌥/Ctrl]-click or this icon to edit first"* (with the modifier
    matching the host OS).
  - Activating via Enter / Space while focused dispatches
    `InsertPromptSuggestionAsDraft`, identical to mouse click.
  - Pixel parity with the existing card layout is NOT required; the
    icon uses the existing icon-button base style and the card's
    existing padding.
- B8. Sequential composition (NOT out of scope in V1). Users may
  insert multiple suggestions in a row. Each insert applies the
  B2 replace-with-confirm rule against the current draft. The
  suggestion banner remains visible after insert-as-draft; it
  does NOT auto-dismiss. The banner dismisses only on:
  - The user manually sending the prompt (Enter / Cmd-Enter), OR
  - The user manually closing the banner, OR
  - Context loss (conversation switch, tab close, model change).

## Acceptance criteria

- A1. Plain click on a suggestion → sends (existing behavior).
- A2. **Edit icon affordance.** Clicking the pencil icon on a
  suggestion card inserts the suggestion text into the agent
  input, fires no send action, and lands the caret at the end of
  the inserted text. The icon is keyboard-focusable, has
  `aria-label="Edit suggestion before sending"`, and Enter/Space
  while focused dispatches the same insert action.
- A3. **Modifier-click affordance.** Alt-click on macOS/Linux and
  Ctrl-click on Windows on a suggestion card inserts the
  suggestion text identically to A2.
- A4. **Both affordances ship together.** A V1 build with only one
  affordance present fails this spec.
- A5. Existing-draft replace flow. With a non-empty draft in the
  input, insert-as-draft shows the replace-confirm dialog. Choosing
  Replace clears the draft and inserts the suggestion at the end;
  choosing Cancel makes no change.
- A6. Empty-draft insert. With an empty input, insert-as-draft
  inserts directly with no confirm dialog.
- A7. **Sequential composition.** After insert-as-draft, the
  suggestion banner remains visible; clicking another suggestion
  prompts the replace-confirm dialog again. Banner dismisses only
  on send, manual close, or context loss.
- A8. Telemetry. The `prompt_suggestion.action` event fires exactly
  once per insert with `action_type = "insert_as_draft"`. Existing
  `category` and `source` fields are reused; no new payload field
  is added. Respects global telemetry opt-out.

## Implementation pointers

- Suggestion render is in
  `app/src/terminal/view/inline_banner/prompt_suggestions.rs`.
- `TerminalAction::ResolvePromptSuggestion(...)` (search:
  `app/src/terminal/view/init.rs`) is today's "send-it-now" path.
  Add a sibling `InsertPromptSuggestionAsDraft(PromptSuggestion)`
  action that targets the agent input editor's insert-at-caret
  path. Keep the full `PromptSuggestion` available until telemetry
  is emitted so the category can be derived from existing fields.
- The agent-input editor lives in
  `app/src/ai/blocklist/agent_view/agent_input_footer/...`.

## Test plan

- T1. Modifier-click (Alt/Ctrl per OS) on a suggestion fixture
  dispatches `InsertPromptSuggestionAsDraft`; no send action fires.
- T2. Clicking the edit icon on the same fixture dispatches the
  same action through the same path.
- T3. Edit icon: keyboard activation via Enter and Space when the
  icon is focused dispatches the insert action; `tabindex` and
  `aria-label` present.
- T4. Plain click still dispatches the existing send action.
- T5. Empty-draft path: insert-as-draft against an empty input
  fills the input with the suggestion text, caret at end, no
  confirm dialog shown.
- T6. Non-empty-draft path: insert-as-draft against a non-empty
  input shows the replace-confirm dialog. Replace clears the
  draft and inserts; Cancel preserves the existing draft.
- T7. Sequential composition: two consecutive insert-as-draft
  actions each prompt confirm (when draft non-empty); banner
  remains visible across both.
- T8. Telemetry: `prompt_suggestion.action` event fires exactly
  once per insert with `action_type = "insert_as_draft"`, reusing
  existing `category` and `source` fields and using the B6
  category derivation without suggestion text. Respects global
  telemetry opt-out.

## Out of scope

- Caret-insert / append-with-newline behavior for non-empty drafts
  (V1 is replace-with-confirm; future TECH may relax).
- A "don't ask again" persistence option for the replace-confirm
  dialog.
- Drag-and-drop of suggestions into the input.
- A "save this edited suggestion" path (turning it into a
  reusable rule).
- Pixel parity with any specific design mock — the icon and card
  use the existing icon-button base style and card padding.
