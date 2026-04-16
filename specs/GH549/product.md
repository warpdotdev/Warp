# Notebook editor renders invalid Mermaid blocks instead of leaving them as code
## Summary
When a user changes a notebook code block's language to `Mermaid`, Warp should only switch that block into Mermaid-diagram rendering if the block's current contents are actually parseable as a Mermaid diagram. Blocks whose contents are not valid Mermaid should continue to render as an ordinary code block until their contents become parseable.
Reference: GitHub issue `warpdotdev/warp-external#549`.
## Problem
The notebook code block language dropdown exposes `Mermaid` as a selectable language. Today, as soon as the user picks `Mermaid`, Warp switches the block into its Mermaid diagram rendering path, regardless of whether the block contents are valid Mermaid. This makes ordinary code, plain notes, or work-in-progress diagrams behave like a broken or empty diagram (for example, a code-block frame with the "Rendering Mermaid diagram…" placeholder or a visibly failed render) instead of staying readable as normal text while the user writes or pastes their diagram.
This hurts three common flows:
* A user selects `Mermaid` first and types the diagram into an empty block. The block spends the entire typing flow in broken-diagram mode.
* A user converts an existing non-Mermaid code block to `Mermaid` by mistake, and the original content is hidden behind an unrenderable diagram view.
* A user has a nearly-valid Mermaid diagram with a syntax error. They lose the ability to see and edit the raw source while they fix it.
## Goals
* A notebook code block only enters Mermaid-diagram rendering when its current contents are parseable as a Mermaid diagram.
* Non-parseable Mermaid-labeled blocks look and behave like a normal notebook code block (readable source, syntax highlighting where applicable, copyable text).
* Switching between invalid and valid Mermaid contents flips the block between code-block view and diagram view automatically without the user needing to re-select the language.
* Keep the authored Mermaid source intact in the buffer at all times, regardless of which visual mode is active.
## Non-goals
* Adding inline error diagnostics or red squiggles for invalid Mermaid source. Showing the raw code block when parsing fails is enough for this iteration.
* Reworking the broader code-block language dropdown, the list of supported languages, or the styling of non-Mermaid code blocks.
* Changing the Mermaid render pipeline itself (theme, sizing, caching, or the `mermaid_to_svg` crate's parsing rules).
* Changing Mermaid behavior in non-notebook surfaces (agent output, plans, chat) beyond what is needed to keep behavior consistent. If a shared layout path is touched, it must not regress those surfaces.
* Introducing a separate "Mermaid source editing mode" toggle.
## Figma / design references
Figma: none provided.
## User experience
These rules apply to notebook code blocks whose language is set to `Mermaid` (via the dropdown or via markdown round-trip).
### Language selection
* The `Mermaid` option remains available in the code block language dropdown.
* Picking `Mermaid` sets the block's language to Mermaid in the buffer exactly as it does today, including when the content round-trips to markdown as a ```` ```mermaid ```` fence.
* Picking `Mermaid` does not, on its own, force a diagram render.
### Rendering decision
* A Mermaid-labeled block renders as a diagram when, and only when, its current contents can be successfully parsed as a Mermaid diagram by the same pipeline that produces the SVG.
* A Mermaid-labeled block with contents that cannot be parsed renders as an ordinary notebook code block: monospaced code, syntax highlighting as normally applied to code blocks, the standard code-block chrome (border, copy button, language dropdown), and a visible language of `Mermaid` in the dropdown.
* The transition between the two states is automatic: as the user types, pastes, or deletes inside the block, the block switches between code view and diagram view when the parseable/unparseable state flips.
* An empty Mermaid-labeled block is treated as unparseable and renders as a normal (empty) code block.
### Diagram state (unchanged from today, for reference)
* A successfully parsed Mermaid block renders the diagram inside the existing Mermaid block frame, with the existing copy affordance and layout sizing behavior.
### While parse/render is in progress
* While a Mermaid block's parseability is still being determined (for example the first time it becomes valid and the async render is still running), the block must not flicker into a visibly broken diagram state. The acceptable transitional visuals are:
  * the existing "Rendering Mermaid diagram…" placeholder inside the diagram frame, or
  * the code-block view with the raw source visible.
  The choice should be consistent for a given block and should not rapidly alternate between the two on every keystroke.
### Editing and interaction
* When the block is in code-block view (because the contents are not parseable), all ordinary code-block editing behaviors apply: the user can click into the text, position a cursor, select characters, type, paste, copy, cut, and undo/redo.
* When the block is in diagram view, editing behavior matches today's Mermaid diagram experience.
* Switching views as a result of an edit must not lose, reorder, or truncate the authored Mermaid source. The buffer content is the source of truth.
* The language dropdown must continue to show `Mermaid` as the current selection in both views.
### Round-trip and export
* The block is persisted and exported as a ```` ```mermaid ```` fenced code block whether or not its contents currently parse. Export behavior must not depend on the visual render mode.
* Reopening a notebook produces the same view for the same contents: parseable contents render as a diagram, unparseable contents render as a code block.
### Feature flag
* This fix is gated by the existing Mermaid rendering feature flag. When the flag is off, current behavior (no Mermaid rendering) is preserved.
### Scope of affected surfaces
* Required surface: notebook editor code blocks.
* Any shared Mermaid layout path reused by other surfaces (for example plans, agent output) must either adopt the same "only render when parseable" behavior or must be explicitly left unchanged by the fix without regressing today's behavior. Expanding the fix to other surfaces beyond notebooks is acceptable but not required.
## Success criteria
* Selecting `Mermaid` on a code block containing non-Mermaid text (for example shell commands, plain English, a partial diagram, or empty content) leaves the block rendered as a normal code block with the original text visible and editable. No Mermaid diagram frame, placeholder, or broken-diagram state is shown.
* Editing a Mermaid-labeled block so its contents become valid Mermaid automatically switches it into diagram rendering without requiring the user to re-open or re-select the language.
* Editing a Mermaid-labeled block so previously valid contents become invalid automatically switches it back to code-block rendering with the raw source visible.
* The language dropdown continues to show `Mermaid` as the selected value across both visual states.
* Saving and reopening a notebook, or exporting to markdown, preserves the ```` ```mermaid ```` fence and the authored source in both states.
* The existing behavior of a valid Mermaid block (successful render, sizing, copy affordance) is unchanged when the contents are valid.
* Toggling between valid and invalid contents repeatedly during typing does not cause content loss, cursor loss on invalid→valid transitions within the code view, or visibly stuck broken-diagram states.
## Validation
* Automated editor/layout tests:
  * A Mermaid-labeled block with empty contents lays out as a code block, not a Mermaid diagram.
  * A Mermaid-labeled block with non-Mermaid contents (for example `echo hi`) lays out as a code block.
  * A Mermaid-labeled block with known-valid Mermaid source (for example `graph TD\nA --> B`) lays out as a `BlockItem::MermaidDiagram`.
  * A Mermaid-labeled block with contents that start invalid and then become valid on edit re-lays out as a diagram on the next render pass.
  * A Mermaid-labeled block with contents that start valid and then become invalid on edit re-lays out as a code block on the next render pass.
* Automated notebook-level tests:
  * Changing a block's language to `Mermaid` while its contents are not valid Mermaid does not produce a diagram block in the render tree.
  * The dropdown shows `Mermaid` as selected after the language change in both the valid and invalid states.
  * Round-trip through markdown export preserves the ```` ```mermaid ```` fence in both states.
* Manual verification in a notebook:
  * Create an empty block, set language to `Mermaid`, confirm it looks like a normal code block and stays editable.
  * Paste `graph TD\nA --> B`, confirm the block switches to the rendered diagram automatically.
  * Delete characters until the source is no longer valid, confirm the block switches back to a code-block view with the current raw text.
  * Switch language away from `Mermaid` and back, confirm behavior is consistent in both directions.
  * Save the notebook, reopen it, confirm rendered state matches the contents.
## Open questions
* When parse validity is determined asynchronously (for example while `render_mermaid_to_svg` is still running for the first time on new valid content), should the block default to the code-block view until the render succeeds, or show the existing "Rendering Mermaid diagram…" placeholder inside the diagram frame? The tech spec should recommend one default and justify it.
* Should the fix also gate the Mermaid-diagram path in non-notebook surfaces that reuse the same layout code, or should it be scoped to notebooks only in this iteration? The current requirement is that the notebook behavior must be fixed and non-notebook behavior must not regress.
