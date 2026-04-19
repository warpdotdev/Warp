# Notebook editor: render Mermaid blocks only when contents are valid

## Summary
When a notebook code block's language is set to `Mermaid`, Warp should only render it as a Mermaid diagram if the block's current contents are parseable as valid Mermaid. Blocks with unparseable contents must render as ordinary code blocks until their contents become valid, switching automatically as contents change.

Reference: GitHub issue `warpdotdev/warp-external#549`.

## Problem
The notebook code block language dropdown exposes `Mermaid` as a selectable language. Today, as soon as the user picks `Mermaid`, Warp switches the block into its Mermaid diagram rendering path regardless of whether the block contents are valid Mermaid. This makes ordinary code, plain notes, or work-in-progress diagrams appear as a broken or empty diagram frame instead of staying readable as normal text.

This hurts three common flows:
- A user selects `Mermaid` first and types the diagram into an empty block — the block spends the entire typing flow in broken-diagram mode.
- A user converts an existing non-Mermaid code block to `Mermaid` by mistake — the original content is hidden behind an unrenderable diagram view.
- A user has a nearly-valid Mermaid diagram with a syntax error — they lose the ability to see and edit the raw source while fixing it.

## Goals / Non-goals

**Goals:**
- A notebook code block only enters Mermaid-diagram rendering when its current contents are parseable as a Mermaid diagram.
- Non-parseable Mermaid-labeled blocks look and behave like a normal notebook code block (readable source, syntax highlighting, copyable text).
- Switching between invalid and valid Mermaid contents flips the block between code-block view and diagram view automatically without re-selecting the language.
- The authored Mermaid source is preserved in the buffer at all times, regardless of which visual mode is active.

**Non-goals:**
- Adding inline error diagnostics or red squiggles for invalid Mermaid source. Showing the raw code block when parsing fails is enough for this iteration.
- Reworking the code-block language dropdown, the list of supported languages, or the styling of non-Mermaid code blocks.
- Changing the Mermaid render pipeline itself (theme, sizing, caching, or the `mermaid_to_svg` crate's parsing rules).
- Changing Mermaid behavior in non-notebook surfaces beyond what is needed to keep behavior consistent.
- Introducing a separate "Mermaid source editing mode" toggle.

## Figma
Figma: none provided.

## Behavior

The following invariants apply to notebook code blocks whose language is set to `Mermaid` (via the dropdown or via markdown round-trip).

**Language selection**

1. The `Mermaid` option remains available in the code block language dropdown.
2. Picking `Mermaid` sets the block's language to Mermaid in the buffer; the block serializes as a ```` ```mermaid ```` fenced code block on markdown export, regardless of whether its contents are currently parseable.
3. Picking `Mermaid` does not, on its own, force diagram rendering.

**Rendering decision**

4. A Mermaid-labeled block renders as a diagram when, and only when, its current contents can be successfully parsed by the same Mermaid pipeline that produces the SVG.
5. A Mermaid-labeled block whose contents cannot be parsed renders as an ordinary notebook code block: monospaced source text, syntax highlighting as normally applied to code blocks, the standard code-block chrome (border, copy button, language dropdown), and `Mermaid` shown as the selected language.
6. An empty Mermaid-labeled block is treated as unparseable and renders as a normal empty code block.
7. The transition between code-block view and diagram view is automatic: as the user types, pastes, or deletes, the block switches views when the parseable/unparseable state flips — without the user re-selecting the language.

**While parse/render is in progress**

8. While a Mermaid block's parseability is being determined for the first time (async render still in progress), the block must not flicker into a broken diagram state. The block defaults to code-block view (raw source visible) until the async render resolves; this avoids the broken-diagram flash and is safe because the cache prevents re-parsing the same source twice. The choice must not rapidly alternate on every keystroke.

**Diagram state (when contents are valid)**

9. A successfully parsed Mermaid block renders the diagram inside the existing Mermaid block frame with the existing copy affordance and layout sizing behavior — unchanged from today.

**Editing and interaction**

10. When the block is in code-block view, all ordinary code-block editing behaviors apply: click to place a cursor, select, type, paste, copy, cut, and undo/redo.
11. When the block is in diagram view, editing behavior matches today's Mermaid diagram experience.
12. Switching between views as a result of an edit must not lose, reorder, or truncate the authored Mermaid source. The buffer content is the source of truth.
13. The language dropdown shows `Mermaid` as the current selection in both views at all times.

**Round-trip and export**

14. The block is persisted and exported as a ```` ```mermaid ```` fenced code block regardless of the current visual render mode.
15. Reopening a notebook produces the same view for the same contents: parseable contents render as a diagram; unparseable contents render as a code block.

**Feature flag**

16. This fix is gated by the existing Mermaid rendering feature flag. When the flag is off, the current behavior (no Mermaid rendering) is preserved.

**Scope of affected surfaces**

17. The fix is required for notebook editor code blocks. Any shared Mermaid layout path reused by other surfaces (plans, agent output) must either adopt the same "render only when parseable" behavior or be explicitly left unchanged without regressing today's behavior. Expanding the fix to other surfaces is acceptable but not required.
