# Notebook editor: Raw/Rendered toggle for Mermaid code blocks

## Summary
When a notebook code block's language is set to `Mermaid`, Warp should show a Raw/Rendered segmented control (identical to the one used in the Markdown file viewer) in the block footer. The control defaults to Raw, which shows the Mermaid source as an editable code block. Selecting Rendered attempts to render the source as a diagram; if rendering fails, an error frame is shown instead.

Reference: GitHub issue `warpdotdev/warp-external#549`.

## Problem
The notebook code block language dropdown exposes `Mermaid` as a selectable language. Today, as soon as the user picks `Mermaid`, Warp unconditionally switches the block into its Mermaid diagram rendering path. This makes ordinary code, plain notes, or work-in-progress diagrams appear as a broken or empty diagram frame instead of staying readable and editable as normal text.

## Goals / Non-goals

**Goals:**
- Mermaid code blocks default to Raw (source text) when the language is set to Mermaid.
- A Raw/Rendered segmented control lets the user explicitly opt in to diagram rendering per block.
- Rendered mode shows the rendered SVG diagram, or an error frame when the source is invalid.
- The authored Mermaid source is always preserved in the buffer, regardless of display mode.

**Non-goals:**
- Making the Raw/Rendered choice persistent across sessions or round-trips to markdown.
- Reworking the code-block language dropdown, the list of supported languages, or the styling of non-Mermaid code blocks.
- Changing the Mermaid render pipeline itself (theme, sizing, caching, or `mermaid_to_svg`).
- Changing Mermaid behavior in non-notebook surfaces (agent output, plans).

## Figma
Figma: none provided. The segmented control appearance matches the existing Raw/Rendered toggle used in the markdown file viewer (screenshot attached to the GitHub issue).

## Behavior

The following invariants apply to notebook code blocks whose language is set to `Mermaid` (via the dropdown or via markdown round-trip).

**Language selection**

1. The `Mermaid` option remains available in the code block language dropdown.
2. Picking `Mermaid` sets the block's language to Mermaid in the buffer; the block serializes as a ```` ```mermaid ```` fenced code block on markdown export, regardless of display mode.
3. Picking `Mermaid` does not, on its own, trigger diagram rendering — the block opens in Raw mode.

**The Raw/Rendered toggle**

4. A Mermaid-labeled block displays a Raw/Rendered segmented control in the block footer, using the same visual style as the toggle in the markdown file viewer.
5. The toggle defaults to Raw whenever the language is set to Mermaid (including on first open, on markdown round-trip, and when the language dropdown is changed to Mermaid).
6. The toggle is visible whenever the block's language is Mermaid, regardless of which mode is active — including when the diagram is successfully rendered. The toggle must not disappear when the block switches to Rendered mode.
7. The Raw/Rendered choice is per-block and per-session only — it is not persisted to the notebook file or round-tripped through markdown export.

**Raw mode**

8. In Raw mode the block renders as an ordinary notebook code block: editable source text, the standard code-block chrome (border, copy button, language dropdown), and `Mermaid` shown as the selected language.
9. All ordinary code-block editing behaviors apply in Raw mode: click to place a cursor, select, type, paste, copy, cut, and undo/redo.
10. The buffer content is the source of truth and is preserved regardless of display mode.

**Rendered mode — successful render**

11. When the user selects Rendered, Warp attempts to render the block's current source as a Mermaid diagram using the existing SVG rendering pipeline.
12. While the async render is in progress the block shows a "Rendering Mermaid diagram…" placeholder inside the diagram frame.
13. On a successful render the block shows the rendered diagram inside the diagram frame, with the existing copy affordance and layout sizing behavior.

**Rendered mode — failed render**

14. If the Mermaid source cannot be parsed or rendered, the block shows an error frame in place of the diagram. The error frame displays a message such as "Error rendering Mermaid diagram. Please check syntax."
15. The error frame uses the same dimensions and border style as the diagram frame — it does not fall back to code-block view.
16. The Raw/Rendered toggle remains visible and functional in the error state. The user can switch back to Raw to edit and fix the source.

**Round-trip and export**

17. The block is persisted and exported as a ```` ```mermaid ```` fenced code block regardless of the current display mode.
18. Reopening a notebook always opens Mermaid blocks in Raw mode (the toggle resets to Raw on every open).

**Feature flag**

19. This behavior is gated by the existing `FeatureFlag::MarkdownMermaid` flag. When the flag is off, Mermaid blocks render as ordinary code blocks with no toggle and no diagram rendering.
