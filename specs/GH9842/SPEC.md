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

- B1. Each prompt-suggestion entry exposes two affordances:
  - **Primary click (existing):** sends the suggestion as-is.
    Pixel-equivalent to today.
  - **New: insert-as-draft.** Both entry points are required and
    dispatch the same action:
    - A small edit icon next to the suggestion.
    - Modifier-click on the suggestion (Cmd-click on macOS,
      Ctrl-click on Linux/Windows).
- B2. Insert-as-draft inserts the suggestion text at the current
  agent-input caret position without dispatching it. If the input
  is empty, this is equivalent to setting the input to the
  suggestion text. If the input already has user text, preserve
  that text and insert at the caret; V1 does not show a replace
  confirmation and does not overwrite the existing draft.
- B3. Cursor lands immediately after the inserted suggestion text.
- B4. The insert dispatches NO send action. The user must
  manually send via Enter / Cmd-Enter as usual.
- B5. The existing inline-banner prompt-suggestion model
  (`app/src/terminal/view/inline_banner/prompt_suggestions.rs`)
  is the data source; no schema change.
- B6. Telemetry: emit a single `prompt_suggestion_inserted_as_draft`
  event when the user takes the new affordance. No payload beyond
  the suggestion category. Derive the category only from existing
  `PromptSuggestion` fields, in this order:
  `static_prompt_suggestion_name = Some(name)` maps to
  `static:{name}`; otherwise `coding_query_context.is_some()` maps
  to `coding_query`; otherwise `should_start_new_conversation`
  maps to `new_conversation` when true and `follow_up` when false.
  Never derive the category from `prompt`, `label`, `id`, or the
  user's edited result.

## Acceptance criteria

- A1. Click the suggestion → it sends (existing behavior preserved).
- A2. Modifier-click and click the edit icon both insert text into
  the agent input, no send fires, cursor lands after the inserted
  text.
- A3. With existing draft text in the input, the suggestion inserts
  at the caret and preserves the surrounding text.
- A4. The new event fires exactly once per insert; respects
  global telemetry opt-out.

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

- T1. Modifier-click on a suggestion fixture inserts the text
  into the agent-input editor model, no send action fires.
- T2. Clicking the edit icon on the same fixture inserts the text
  through the same action path.
- T3. Plain click still dispatches the existing send action.
- T4. With pre-existing input, insert-as-draft inserts at the
  caret and preserves existing text.
- T5. New telemetry event fires exactly once per insert and uses
  the B6 category mapping without suggestion text.

## Out of scope

- Multi-suggestion compose (insert two suggestions in sequence
  to build a longer prompt).
- Drag-and-drop of suggestions into the input.
- A "save this edited suggestion" path (turning it into a
  reusable rule).
