# 03 — Render markdown by default

**Phase:** not-started
**Spec PR:** —
**Impl PR:** —

## Scope

When the user opens a markdown file (`.md`, `.markdown`) inside twarp, render it by default rather than show raw text. Whatever toggle exists upstream to switch between rendered and raw stays available; only the **default** flips. Applies to whichever surface(s) twarp uses to display file contents — file-preview pane, "view file" command, hover preview on a file path, etc.

## Approach note

Spec phase pins down:

1. **Which surfaces.** Identify every place upstream Warp displays a markdown file's contents. Usual suspects: a file-preview side panel, a quick-look popover triggered from terminal output, the agent assistant's transcript renderer (now gone after 02), `cat`-style block output styling.
2. **Current default.** For each surface, confirm the current default is raw text and find the toggle / setting that flips it. The default-flip is the headline change; if a surface already defaults to rendered, document and skip it.
3. **Setting key.** Use the existing markdown-render setting if one exists; introduce a new key only if no upstream key fits. Either way, a user can override via settings to restore raw-default behavior.

## Sub-phases

Single phase — one PRODUCT.md + TECH.md, one impl PR. If multiple surfaces turn out to be controlled by independent settings (one per surface), revisit during spec writing and consider splitting.

## Why this is feature 03

Small scope, fast turnaround, useful as a daily-driver win for users reading READMEs and spec files in the terminal. Slotted after AI removal because some markdown rendering pipelines upstream are entangled with the AI assistant transcript renderer; running 02 first means the surface area for this change is whatever's left after the AI render path goes away.
