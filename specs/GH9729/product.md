# Image preview in the Code Editor file pane: Product Spec

GitHub issue: https://github.com/warpdotdev/warp/issues/9729
Figma: none provided

## Summary

Clicking an image file in the File Tree currently routes through the binary-file fallback and hands the file to the OS default app, or for SVG opens raw XML in the Code Editor. v1 fixes this by routing supported image extensions through the existing **Lightbox** overlay component, which already knows how to render an image, dim the workspace behind a scrim, dismiss on Escape / scrim-click / × button, and step through a list of images with Left and Right arrows. The Lightbox is a transient overlay over the workspace, not a new tab variant; it is not draggable, splittable, or restored across sessions.

## Problem

Warp's File Tree treats every clicked file as a candidate for the Code Editor or for the OS default app. For image files this means the user either gets bumped out to Finder/Preview (raster formats hit `is_binary_file` and fall through to `FileTarget::SystemGeneric`) or sees raw XML (SVG is text-classified and lands in the Code Editor). There is no way today to glance at an image asset (an icon, a screenshot, a logo, an SVG) from inside Warp without leaving the window.

The maintainer-preferred shape for v1 is the existing Lightbox component. It already renders an image with `Image::new(asset_source).contain()`, has open/close/replace lifecycle driven by `WorkspaceAction::OpenLightbox` and `UpdateLightboxImage`, supports a multi-image array with prev/next navigation, and overlays the active workspace without disturbing tab and split state. Reusing it gives users a fast preview affordance in one release without committing the codebase to a parallel "image tab" surface that would need its own restore, dragging, splitting, and persistence story.

## Goals

- Clicking a supported image file (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`) in the File Tree opens the existing Lightbox overlay over the active workspace window.
- The Lightbox uses the existing image rendering pipeline (`Image::new(...).contain()` backed by `ImageType` in `crates/warpui_core/src/image_cache.rs`); no new image-rendering code is introduced.
- Left / Right arrow keys, with the Lightbox focused, step through other supported image files in the same directory, sorted by case-insensitive natural filename order. Navigation does not wrap.
- Loading and decode-error states for the active image surface inside the Lightbox without crashing or blocking neighbour navigation.
- Telemetry distinguishes the image-preview open path from the existing `MarkdownViewer`, `CodeEditor`, and `SystemGeneric` open paths, via the existing `CodePanelsFileOpened.target` field.
- The decode path enforces a maximum image dimension and a maximum decoded-pixel cap so a maliciously large or malformed file cannot exhaust process memory.

## Non-goals

- Zoom (`Cmd+=`, `Cmd+-`, `Cmd+0`), trackpad pinch-zoom, click-drag pan.
- Status footer (filename, pixel dimensions, file size, format string).
- Animation play/pause control and persistent pause state across navigation.
- EXIF orientation rotation and embedded ICC color profile honoring.
- A visible thumbnail strip in the Lightbox (the sibling list exists internally to drive arrow navigation; it is not rendered).
- Format support beyond `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`. HEIC, HEIF, BMP, TIFF, AVIF, ICO are out of scope for v1.
- Magic-byte content sniffing when extension and content disagree.
- Right-click context menu on the image (Copy Image, Copy file path, Reveal in Finder, Attach as Agent context).
- Drag-out from the Lightbox into the Agent input as an attachment.
- Disk-backed thumbnail cache and configurable cache size cap.
- Image diff across git revisions, remote URL preview, image editing, PDFs.
- Terminal-grid inline images (tracked separately in #1693, #5286).
- Retroactive replacement of the OS handoff for image extensions outside the supported set; those continue to land in `SystemGeneric`.

## User Experience

### Opening

- Single-clicking a supported image file in the File Tree opens the Lightbox over the active workspace window. No Code Editor tab is opened and no persistent tab of any kind is created.
- Keyboard-only entry uses the same path: navigating to the file in the tree with the arrow keys and pressing Enter opens the Lightbox identically to a click. (The File Tree's keyboard activation already resolves through the same `open_file` function as the click path.)
- The Lightbox attaches to the workspace window that initiated the click; multi-window users see the overlay only on the originating window, since `Workspace.lightbox_view` is per-workspace.

### Inside the Lightbox

- The image renders centered, fit to the viewport via `Image::new(...).contain()`: aspect ratio preserved, never upscaled past the image's native pixel dimensions.
- The Lightbox renders as a child of the workspace's main render `Stack`, so the scrim covers the whole workspace (all panes are dimmed, not just the active pane). This matches today's screenshot/artifact Lightbox behavior.
- Underlying panes remain visible behind the scrim and do not receive input while the Lightbox is open.
- While bytes are still being read or decoded, the existing loading indicator is shown. Arrow-key navigation remains responsive; an in-flight decode never blocks navigation.
- If an image fails to read or decode, or fails the size/dimension cap, the Lightbox shows a non-blocking error state for that entry that includes the filename. Neighbour navigation still works; no other tab or pane is affected.
- SVG renders via the existing `usvg` + `resvg` pipeline used by `ImageType`. Animated GIF and animated WebP play in whatever mode `ImageType::AnimatedBitmap` produces today; v1 does not add a play/pause control and does not promise any specific frame-timing accuracy beyond what the existing pipeline does.

### Navigation

- Left and Right arrow keys, with the Lightbox focused, navigate to the previous and next supported image files in the same directory as the opened image.
- Sibling list construction:
  - The directory is scanned at open time; entries are filtered by `is_supported_image_file` and sorted in case-insensitive natural order so `image2.png` precedes `image10.png`.
  - Hidden files (those whose name starts with `.`) are included only if the originally clicked file is itself hidden; otherwise they are excluded, matching the File Tree's default visibility.
  - Symlinks are followed for the entry-type check; broken symlinks are surfaced as the per-entry decode/read error described above and do not block neighbour navigation.
- Navigation does not wrap. The previous-arrow control is hidden or disabled when the first image is shown; the next-arrow control is hidden or disabled when the last image is shown.
- Holding an arrow key produces standard OS key-repeat: index advances per repeat event; in-flight decodes for skipped indices are not cancelled but their results are ignored if the index has moved on.
- If only one supported image exists in the directory, both arrow controls are hidden; the Lightbox renders that single image.

### Re-open and replace

- Clicking another supported image while the Lightbox is already open replaces the current image set and current index in place by dispatching a fresh `WorkspaceAction::OpenLightbox`. The existing handler reuses the open `LightboxView` via `update_params` rather than stacking a second overlay. (The action is `OpenLightbox`, not `UpdateLightboxImage`, since `UpdateLightboxImage` only mutates one entry.)
- Clicking the same image whose Lightbox is already open is a no-op (a fresh `OpenLightbox` is dispatched but the view's `update_params` produces an equivalent state and the user sees no change).
- Clicking a non-image file while the Lightbox is open opens that file by its normal target (Code Editor, Markdown viewer, external editor, etc.). The Lightbox does not auto-dismiss on this path; it dismisses only via Escape, scrim click, or × button. Focusing another pane fires `LightboxViewEvent::FocusLost`, which tears down the view in the existing handler.

### Dismiss and focus restoration

- The Lightbox dismisses on Escape, on a click outside the image (scrim click), and via its close (×) button. All three paths emit `LightboxViewEvent::Close`.
- On dismiss, focus returns to the previously-active tab pane via the existing `focus_active_tab` call in the `OpenLightbox` close handler. If the previously-active pane has been closed in the meantime, focus falls back to whatever `focus_active_tab` resolves to at that moment.

### Filesystem mutations during preview

- If the file or its parent directory is deleted, renamed, or moved while the Lightbox is open, the active entry surfaces as a per-entry read or decode error per the rules above. Already-loaded images in the asset cache continue to render. The Lightbox does not refetch on its own; the user can navigate or dismiss.

### Error and limit handling

- Decode errors, read errors, and dimension/pixel-cap rejections all surface as the same per-entry error state with the filename shown. The Lightbox never crashes the workspace.
- Files whose extension is in the supported set but whose decoded pixel count or maximum dimension exceeds the cap (see Goals and the tech spec for exact values) render as the per-entry error state. They do not get a partial render.

### Accessibility

- The filename of the active image is exposed as the accessible label of the rendered image so screen readers announce it.
- The keyboard-only entry path described above (tree navigation + Enter, then arrows in the Lightbox, then Escape) is the documented a11y flow.
- High-contrast mode: the existing scrim color (RGBA 0,0,0,230) and existing close/prev/next button styling are unchanged in v1; behavior is identical to today's Lightbox usage in screenshot/artifact previews.
- Reduced-motion preference: v1 does not add new motion. Any open/close fade is whatever the existing Lightbox already does.

### Unaffected surfaces

- Terminal escape-sequence handling, inline-image protocols on the terminal grid, and the agent-mode image-attach pipeline are unchanged.
- Files outside the supported extension set continue to follow their existing target (Code Editor for text, `SystemGeneric` for binary, external editor per user preference, etc.).

## Success criteria

- Clicking each of `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg` in the File Tree opens the Lightbox.
- Left / Right arrow keys step through siblings in case-insensitive natural order, do not wrap, and remain responsive while a decode is in flight.
- Escape, scrim click, and × button all dismiss the Lightbox and restore focus to the previously-active tab pane.
- Opening a second image while the Lightbox is open replaces the image set in place; no second overlay stacks.
- A corrupt PNG, an unreadable file, and an oversize PNG (above the dimension/pixel cap) all surface as per-entry errors with the filename shown; navigation continues; no crash.
- An animated GIF and an animated WebP play; SVG renders via `usvg` / `resvg`.
- Telemetry events for image opens are distinguishable from `MarkdownViewer` / `CodeEditor` / `SystemGeneric` opens via the `target` field on `CodePanelsFileOpened`.
- Non-image binary files (`.zip`, `.mp3`, `.exe`, `.pdf`, etc.) continue to route to `SystemGeneric` exactly as before; no regression.

## Validation

- Unit tests for the natural-sort helper covering case differences, numeric runs (`a1.png`, `a2.png`, `a10.png`, `A11.png`), and the leading-dot hidden-file filter.
- Unit tests for the resolver-priority change: `resolve_file_target_with_editor_choice` returns `FileTarget::ImagePreview` for each supported extension, ahead of both the markdown/code probe (so SVG is captured) and the binary fallthrough (so PNG/JPEG/etc. are captured).
- Unit tests confirming non-image binary extensions (`.zip`, `.mp3`, `.exe`, `.pdf`) still resolve to `SystemGeneric`.
- Unit tests for the decoder-limit guard: a synthesized PNG header declaring dimensions above the cap is rejected with an error type the Lightbox surfaces as the per-entry error state.
- Manual: each behavior listed under User Experience above, against fixtures including a small image, a 10000×10000 PNG, an animated GIF, an animated WebP, an SVG, a corrupt PNG, a broken symlink, a directory with 5000 images (smoke test for scan latency), and a directory with mixed hidden/visible images.

## Alternatives considered

- **A new `ImagePreview` tab variant** with its own restore/drag/split story. Rejected by the maintainer in the issue thread; the Lightbox overlay matches the v1 surface area better and avoids committing the codebase to a parallel image-tab system.
- **Routing image files through the existing Code Editor with a binary-buffer renderer.** Rejected because the Code Editor's tab/pane model does not have an image-rendering element and would need substantially more new code than dispatching the existing Lightbox.
- **Always opening through the OS default app.** This is today's behavior for raster formats and is precisely the user complaint in the issue; rejected.
- **Only fixing SVG (since SVG is the worst-looking case today, opening as raw XML).** Rejected as a half-fix; the issue explicitly calls out raster formats and SVG together.

## Open questions

- Exact dimension and pixel-count caps for the decoder-limit guard. The tech spec proposes values aligned with the existing agent-mode caps in `app/src/util/image.rs`; product is comfortable with whatever the tech spec lands on as long as a 10000×10000 PNG is rejected gracefully and a 4000×3000 photo opens.
- Whether the sibling scan should ever be moved off the UI thread for cold-cache cases (very large directories on NFS / FUSE / `~/Library/Caches`-scale `read_dir`). v1 keeps it synchronous; the tech spec lists a follow-up for moving it to the background executor if telemetry or QA shows visible UI freezes.
