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
  - **New: insert-as-draft.** Discoverable via either:
    - A small "edit" icon next to the suggestion (preferred, low
      friction).
    - Modifier-click on the suggestion (Cmd-click / Ctrl-click).
- B2. Insert-as-draft replaces the current agent-input contents
  with the suggestion text. If the input already has user text,
  show a one-time confirmation tooltip ("Replace existing
  draft?") OR insert at the current caret position — TECH spec
  picks one.
- B3. Cursor lands at the end of the inserted text.
- B4. The insert dispatches NO send action. The user must
  manually send via Enter / Cmd-Enter as usual.
- B5. The existing inline-banner prompt-suggestion model
  (`app/src/terminal/view/inline_banner/prompt_suggestions.rs`)
  is the data source; no schema change.
- B6. Telemetry: emit a single `prompt_suggestion_inserted_as_draft`
  event when the user takes the new affordance. No payload beyond
  the suggestion category (e.g., "zero_state", "follow_up") —
  never the suggestion text or the user's edited result.

## Acceptance criteria

- A1. Click the suggestion → it sends (existing behavior preserved).
- A2. Modifier-click OR click the edit icon → text inserts into
  the agent input, no send fires, cursor at end.
- A3. With existing draft text in the input, the chosen behavior
  from B2 fires (replace-with-confirmation OR insert-at-caret).
- A4. The new event fires exactly once per insert; respects
  global telemetry opt-out.

## Implementation pointers

- Suggestion render is in
  `app/src/terminal/view/inline_banner/prompt_suggestions.rs`.
- `TerminalAction::ResolvePromptSuggestion(...)` (search:
  `app/src/terminal/view/init.rs`) is today's "send-it-now" path.
  Add a sibling `InsertPromptSuggestionAsDraft(String)` action
  that targets the agent input editor's set-text path.
- The agent-input editor lives in
  `app/src/ai/blocklist/agent_view/agent_input_footer/...`.

## Test plan

- T1. Modifier-click on a suggestion fixture inserts the text
  into the agent-input editor model, no send action fires.
- T2. Plain click still dispatches the existing send action.
- T3. With pre-existing input, the chosen B2 behavior is honored.
- T4. New telemetry event fires exactly once per insert.

## Out of scope

- Multi-suggestion compose (insert two suggestions in sequence
  to build a longer prompt).
- Drag-and-drop of suggestions into the input.
- A "save this edited suggestion" path (turning it into a
  reusable rule).
