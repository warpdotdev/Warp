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

### B7.1. Malformed `<details>` handling

Untrusted markdown commonly contains malformed HTML. The renderer must remain
robust and never drop content silently. The following cases are explicit:

- **`<details>` with NO `<summary>` child**: render with the default fallback
  summary text `"Details"`. Body content renders normally. The block is NOT
  dropped.
- **`<summary>` not as the first child of `<details>`**: hoist the FIRST
  `<summary>` element to the front. Any inline content that preceded it
  becomes part of the body, in original order. Subsequent `<summary>`
  siblings within the same `<details>` are concatenated into the first
  summary (with a single space separator), per B1.
- **Orphan `<summary>`** (a `<summary>` outside any `<details>` ancestor):
  rendered as inline plain text — the tag itself is stripped, but inner
  inline-markdown is preserved (e.g. `<summary>**hi**</summary>` becomes
  bold `hi`).
- **Unclosed `<details>`**: closed at parse time at the next surface boundary
  (end of input, end of containing block); the partial block emits normally
  with whatever body content was collected.
- **Unclosed `<summary>` inside `<details>`**: closed at the next sibling
  block or at `</details>`, whichever comes first.
- **Nested `<summary>` inside another `<summary>`**: the inner `<summary>` is
  treated as inline plain text within the outer summary (tag stripped, inner
  inline-markdown preserved).
- **Empty `<summary>`** (no text content): render with the fallback summary
  `"Details"`.
- **Block-level elements inside `<summary>`** (lists, headings, code blocks):
  collapse to inline per B6; preserve text content but strip block formatting.

These rules are enforced AT PARSE TIME, before sanitization, so the
sanitizer's allowlist (B8) operates on a normalized tree.

### B8. Sanitization

- Allowlist gains `<details>` and `<summary>` only.
- Attribute allowlist:
  - `details`: `[open]`
  - `summary`: `[]`
- All other attributes (notably any `on*` event handlers, `style`, `id`,
  `class` originating from input) are dropped.
- Existing sanitizer behavior for other tags is unchanged.

### B8.1. Resource limits (security)

`<details>` is a recursive container. Untrusted markdown can therefore embed
deeply nested or extremely high-cardinality `<details>` trees that could
exhaust the stack or starve rendering. The renderer MUST defend against this.

- **Maximum nesting depth: 32 levels** of `<details>` per rendered surface.
  Beyond that, additional nested `<details>` render as PLAIN TEXT — the
  literal `<details>` and `<summary>` opening / closing tags become visible
  text in the output, and any inner markdown body still renders normally
  through the existing markdown pipeline (no content is silently dropped).
- **Maximum `<details>` count per rendered surface: 1000** (soft cap).
  Beyond the 1000th `<details>`, additional `<details>` blocks render as
  plain text, same fallback as the depth cap.
- **Implementation MUST be iterative or guarded recursion**. The depth/count
  bookkeeping must be enforced inside the parser/renderer; it MUST NOT rely
  on unbounded native recursion that risks a stack overflow on adversarial
  input. Either implement with an explicit work stack, or use guarded
  recursion that checks the depth counter before recursing and falls
  through to the plain-text fallback when the cap is hit.
- **Enforcement boundary**: the depth and count limits are enforced AT or
  BEFORE the renderer — not solely in the sanitizer — so that a future
  change to the sanitizer can't accidentally remove the bound. This makes
  rendering a function of the post-parse tree, not of sanitizer state.
- **Mutual / template-style recursion** (e.g. content that, after expansion,
  would re-introduce nested `<details>`): bounded by the same depth counter
  that wraps the recursive render call, so it cannot escape via indirection.

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

> Paths verified against the worktree at spec time. Warp uses an in-house
> markdown parser (`crates/markdown_parser`) built on top of `html5ever`
> (see imports in `crates/markdown_parser/src/html_parser.rs`). The renderer
> is a Warp UI element (`FormattedTextElement`), not browser HTML. Modules
> that don't yet exist are marked `(new)` so reviewers can distinguish
> net-new files from edits to existing files.

- Markdown parser entry points (block-level pass; this is where
  `<details>` must be recognized as a block container and `<summary>` as
  inline-only):
  `crates/markdown_parser/src/markdown_parser.rs`.
- HTML-in-markdown pass (today's allowlist of inline/phrasing element tags
  lives here as `PHRASING_ELEMENT_TAGS`; top-level skips live as
  `TOP_LEVEL_ELEMENT_TAGS_TO_SKIP`. Add `details` as a recognized block
  container and `summary` as a recognized inline header; this is also where
  the malformed-input normalization in B7.1 is enforced):
  `crates/markdown_parser/src/html_parser.rs`.
- Markdown parser library entry / shared types
  (`FormattedText`, `FormattedTextLine`, `FormattedTextFragment` — extend
  with a `Details { open, summary, body }` variant or equivalent):
  `crates/markdown_parser/src/lib.rs`.
- Renderer / view-layer element (UI side of `FormattedText` — add a
  disclosure widget rendering for the new `Details` variant; this is where
  the click handler, focus ring, and `aria-expanded` / `aria-controls`
  hookup live):
  `crates/warpui_core/src/elements/formatted_text_element.rs`.
- Renderer call sites (no per-call changes expected; listed so reviewers can
  spot-check that the new variant is exercised everywhere `FormattedText` is
  rendered):
  `app/src/resource_center/section_views/changelog_section.rs`,
  `app/src/launch_configs/save_modal.rs`,
  `app/src/auth/login_failure_notification.rs`,
  `app/src/changelog_model.rs`.
- Sanitization / attribute allowlist (today's allowlist for HTML-in-markdown
  is implemented inside `html_parser.rs`; this is where `details: [open]`
  and `summary: []` are wired and where `on*` / `style` / `id` / `class`
  are dropped):
  `crates/markdown_parser/src/html_parser.rs`.
- `(new module)` Resource-limit guard for B8.1 — depth + count counters,
  single source of truth, used by both the parser block-handler and the
  renderer fallback path. Suggested location:
  `crates/markdown_parser/src/details_limits.rs`. The renderer's recursive
  rendering of `Details` MUST consult this guard (or accept depth as a
  parameter) and switch to the plain-text fallback once a cap is hit.
- Existing parser tests (extend with `<details>`/`<summary>` cases,
  including the malformed-input matrix from B7.1 and the resource-limit
  matrix from B8.1):
  `crates/markdown_parser/src/markdown_parser_tests.rs`,
  `crates/markdown_parser/src/html_parser_tests.rs`.
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
- T12. Depth cap (B8.1): a `<details>` tree nested 33 levels deep renders
  the first 32 levels as proper disclosure widgets; the 33rd level renders
  as plain text — its `<details>` and `<summary>` tags are visible literal
  text in the output and the inner body still renders through the standard
  markdown pipeline. The render call returns within bounded stack usage
  (no overflow on adversarial input).
- T13. Count cap (B8.1): a surface containing 1001 sibling `<details>`
  blocks renders the first 1000 as widgets; the 1001st renders as plain
  text per the same fallback as T12.
- T14. Mutual / template-style recursion (B8.1): an input crafted so that
  expansion would re-introduce nested `<details>` after depth-32 is
  bounded by the same depth counter; the run terminates and the over-cap
  level renders as plain text.
- T15. Malformed input matrix (B7.1): one assertion per case —
  `<details>` with no `<summary>` (fallback `"Details"`); `<summary>`
  preceded by inline content (summary hoisted, prefix becomes body);
  orphan `<summary>` outside any `<details>` (rendered inline, tag
  stripped); unclosed `<details>` (closed at boundary, body emits
  partially); unclosed `<summary>` (closed at sibling block); nested
  `<summary>` inside `<summary>` (inner treated as inline plain text);
  empty `<summary>` (fallback `"Details"`); block element inside
  `<summary>` (collapsed to inline).

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
