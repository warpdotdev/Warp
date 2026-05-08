# Markdown `<details>` / `<summary>` Support (GH-10259)

## Summary

Warp's markdown renderer should support standard HTML5 `<details>` and
`<summary>` tags so collapsible disclosure widgets work in any rendered
markdown surface (agent output, README previews, conversation list previews).
Today these tags are stripped or rendered as plain text, which breaks a common
GFM convention used heavily in agent outputs and open-source READMEs.

## Problem

- `<details>` / `<summary>` is part of GitHub Flavored Markdown's de facto
  surface and standard HTML5. Many sources Warp renders (READMEs, agent
  outputs, copy-pasted content) rely on it for progressive disclosure.
- In the current renderer, the tags are dropped or escaped, producing flat
  output that is hard to scan — particularly in long agent responses where
  authors deliberately collapse supporting detail.
- Adding the two tags is a small, well-bounded addition to the existing
  sanitizer allowlist; the cost of leaving it out is disproportionate to the
  cost of adding it.

## Goals

- Render `<details>` / `<summary>` as collapsible disclosure widgets across
  every markdown surface the renderer is used in.
- Preserve the `open` attribute.
- Support nested `<details>` to arbitrary depth.
- Keep sanitization safe — no event handler attributes, no broader HTML
  whitelist expansion.
- Provide accessible behavior matching native `<details>` semantics.

## Non-Goals

- Not full HTML rendering. Only `<details>` and `<summary>` are added to the
  allowlist.
- No other tag whitelist expansion beyond what the renderer already supports.
- No animated disclosure transitions in V1.
- No persistence of open/closed state across re-renders in V1 (see B9 for
  V1.5 option).
- No new markdown surfaces — this only changes how existing surfaces render
  these tags.

## Behavior Contract

### B1. Recognized tags

- `<details>` — block-level container. May carry `open` attribute. No other
  attributes preserved.
- `<summary>` — exactly one is the visible always-rendered header. Additional
  `<summary>` siblings inside the same `<details>` are concatenated into the
  first.

### B2. Default state

- Collapsed unless `open` is present on `<details>`.

### B3. Toggle

- Click on the summary text or the chevron icon toggles open/closed.
- Both surfaces of the summary row are clickable; the entire row is the hit
  target.

### B4. Keyboard

- Tab focuses the summary.
- Enter and Space both toggle open/closed.
- Focus ring matches the existing focusable controls in the markdown surface
  (consistent with links, code-copy buttons, etc.).

### B5. Nested `<details>`

- Supported to arbitrary depth.
- Each level renders an independent disclosure with proper indentation.
- Toggling a parent does not alter the open/closed state of children.

### B6. Inside `<summary>`

- Inline markdown is rendered: bold, italic, inline code, links.
- Block markdown (lists, headings, code blocks) inside `<summary>` is NOT
  supported. Block tokens that appear within a summary collapse to inline
  rendering, matching the HTML spec's content model for `<summary>`.

### B7. Inside `<details>` body (after `<summary>`)

- Full markdown rendering, including code blocks, lists, blockquotes, nested
  details, links, images.

### B8. Sanitization

- Allowlist gains `<details>` and `<summary>` only.
- Attribute allowlist:
  - `details`: `[open]`
  - `summary`: `[]`
- All other attributes (notably any `on*` event handlers, `style`, `id`,
  `class` originating from input) are dropped.
- Existing sanitizer behavior for other tags is unchanged.

### B9. Open/close state persistence

- V1: state is local to the rendered instance and is NOT persisted across
  re-renders of the same content (e.g. scroll-out and back, theme switch).
- V1.5 (deferred — see Open Questions): persist via stable summary-text hash
  scoped to the rendering surface.

### B10. Accessibility

- Render as native `<details>` / `<summary>` element when the underlying view
  layer supports them, OR an equivalent ARIA pattern when using a custom
  widget:
  - `role="group"` on the disclosure container.
  - `aria-expanded` on the summary toggle.
  - `aria-controls` linking the summary to its body region.
- Screen readers must announce open/closed state and the accessible name from
  the rendered summary text.

## Settings / API surface

No new settings. No new API.

The change is internal to the markdown rendering pipeline:

- Parser allowlist update.
- Sanitizer attribute allowlist update.
- Renderer adds a `details` block component.

## Acceptance Criteria

- A1. `<details>` with no `open` attribute renders collapsed; only the
  summary text is visible.
- A2. `<details open>` renders expanded by default.
- A3. Click on summary toggles between collapsed and expanded.
- A4. Tab focuses the summary; Enter and Space each toggle the state.
- A5. Three-deep nested `<details>` renders correctly with independent state
  per level and proper indentation.
- A6. Inline markdown (bold, italic, inline code, links) inside `<summary>`
  renders correctly.
- A7. Block markdown (lists, headings, code blocks) inside `<summary>`
  collapses to inline rendering — no nested block tokens.
- A8. Full markdown — including code blocks, lists, links, images, and nested
  `<details>` — renders inside the body.
- A9. Sanitizer rejects `<script>`, event handler attributes (`onclick`,
  `onmouseover`, …), and any non-`open` attribute on `<details>` /
  `<summary>`.
- A10. Accessibility tree audit shows correct `aria-expanded` /
  `aria-controls` / accessible name (or native semantics when using
  `<details>`/`<summary>` directly).

## Implementation Pointers

- Parser config: `app/src/markdown/parser.rs` — extend the HTML allowlist
  with `details` and `summary`. Mark `details` as a block container that may
  contain block content; mark `summary` as inline-only.
- Renderer: `app/src/markdown/renderer.rs` — add a renderer entry for the
  `details` block. Render either:
  - Native `<details>`/`<summary>` if the host view supports it, or
  - A custom widget with `role="group"` + `aria-expanded` +
    `aria-controls` semantics (B10).
- Sanitizer: `app/src/markdown/sanitize.rs` — explicit attribute whitelist:
  `details: ["open"]`, `summary: []`. Drop everything else; never permit
  `on*`, `style`, raw `id`, raw `class`.
- Tokenizer note: ensure `<summary>` content is parsed inline regardless of
  whether the source contains block-level markdown — handle the collapse in
  the tokenizer or renderer, not via post-hoc string trimming.

## Tests

- T1. `<details>...</details>` with no `open` renders collapsed; only the
  summary is in the visible DOM.
- T2. `<details open>` renders expanded; body contents are visible.
- T3. Click on summary toggles open ↔ closed.
- T4. Tab navigation reaches the summary; Enter and Space each toggle.
- T5. Three-deep nested `<details>` parses and renders correctly with
  independent state per level.
- T6. Inline markdown inside `<summary>` (bold, italic, inline code, link)
  renders as expected.
- T7. Block markdown inside `<summary>` (e.g. a list) collapses to inline
  rendering — no `<ul>` / `<h1>` / `<pre>` emitted inside the summary.
- T8. Full markdown inside the body: code block, ordered list, link, image,
  nested `<details>`.
- T9. Sanitizer fuzz: payloads like `<details onclick=…>`,
  `<summary><script>…</script></summary>`,
  `<details style="display:none">` are stripped to safe output.
- T10. Accessibility tree audit on rendered output asserts correct
  `aria-expanded` / `aria-controls` / accessible name (or native
  `<details>` / `<summary>` semantics).
- T11. Markdown content outside any `<details>` block is unchanged versus
  baseline, ruling out renderer regressions.

## Open Questions

- Persist open/closed state across re-renders by stable summary-text hash?
  Suggested for V1.5; defer until V1 ships and we observe whether
  re-render flicker is a real complaint. Persistence scope would be the
  rendering surface (e.g. one agent message), not global.
- Should we render a chevron icon adjacent to the summary, or rely on the
  view layer's native disclosure indicator? Suggested: rely on the view
  layer when using native `<details>`; emit a chevron only in the custom
  ARIA widget path to keep visual parity.

## Telemetry

No new events. If usage signals are needed later, a single
`markdown.details.toggled` event scoped to user-initiated toggles (not
programmatic re-renders) would be sufficient. Out of scope for V1.
