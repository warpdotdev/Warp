# Image preview in the Code Editor file pane: Tech Spec

Product spec: `specs/GH9729/product.md`
GitHub issue: https://github.com/warpdotdev/warp/issues/9729

## Context

Clicking an image file in the Code Editor file tree currently routes through `is_binary_file` (true for `.png/.jpg/.jpeg/.gif/.bmp/.tif/.tiff/.webp/.ico`) and falls out to `FileTarget::SystemGeneric`, which hands the file to the OS default app via `ctx.open_file_path`. SVG is text-classified and instead lands in `FileTarget::CodeEditor`, opening as raw XML. Neither outcome matches the user expectation of "preview this image inline in Warp."

Maintainer guidance in the issue thread is to dispatch the existing `Lightbox` overlay rather than build a new tab variant or new image module. The Lightbox already supports the multi-image-array + initial-index + Loading-placeholder + Left/Right navigation + Escape/scrim/× dismissal model that v1 needs, and the `OpenLightbox` action handler already reuses an open `LightboxView` via `update_params` rather than stacking a second overlay. The asset cache and `ImageType` decoder already handle PNG, JPEG, GIF (animated), WebP (animated), and SVG (`usvg` + `resvg`).

v1 is therefore three things:

1. A new `FileTarget::ImagePreview` variant and an early-return branch in the file-target resolver so supported image extensions are captured ahead of both the markdown/code probe (so SVG is captured) and the `is_binary_file → SystemGeneric` fall-through (so raster formats are captured).
2. A new `Workspace::open_file_with_target` arm that scans sibling images, builds a `Vec<LightboxImage>` of `Resolved { LocalFile { path } }` entries, computes the initial index, and dispatches `WorkspaceAction::OpenLightbox`.
3. A small change to the `ImageType` decode path to enforce a maximum image dimension and decoded-pixel cap, plus a new `LightboxImageSource::Error { message }` variant so per-image decode/read failures surface in the Lightbox with the filename instead of spinning forever.

Everything else (zoom, pan, footer, animation control, EXIF, ICC, thumbnail strip, additional formats, magic-byte sniffing, context menu, drag-out, disk-backed thumbnail cache) is deferred to follow-ups.

## Relevant code

### Existing Lightbox plumbing (no changes needed beyond the new `Error` variant in change 5)

- `crates/ui_components/src/lightbox.rs:28-34`: `pub enum LightboxImageSource { Loading, Resolved { asset_source: AssetSource } }`. v1 adds an `Error { message: String }` variant here.
- `crates/ui_components/src/lightbox.rs:38-43`: `pub struct LightboxImage { source: LightboxImageSource, description: Option<String> }`.
- `crates/ui_components/src/lightbox.rs:55-85`: `pub struct Lightbox` and `pub struct Params<'a>` with `images: &[LightboxImage]`, `current_index`, `on_dismiss`, `current_image_native_size`, `options`. Scrim color RGBA(0,0,0,230) at line 22-24. Renders close button, prev/next chevrons (hidden at boundaries), image via `Image::new(asset_source).contain()`, loading indicator, optional description.
- `app/src/workspace/lightbox_view.rs:30-36`: `pub struct LightboxParams { images: Vec<LightboxImage>, initial_index: usize }`.
- `app/src/workspace/lightbox_view.rs:39-44`: `pub enum LightboxViewEvent { Close, FocusLost }`.
- `app/src/workspace/lightbox_view.rs:15-27`: keybindings `escape → Dismiss`, `left → NavigatePrevious`, `right → NavigateNext` registered in `init`.
- `app/src/workspace/lightbox_view.rs:69-129`: `LightboxView::new`, `update_params`, `update_image_at`, `start_asset_loads`, `start_asset_load`. `start_asset_loads` only kicks off loads for `Resolved` entries; `Loading` entries stay loading until `update_image_at` swaps them.
- `app/src/workspace/action.rs:602-612`: `WorkspaceAction::OpenLightbox { images: Vec<LightboxImage>, initial_index: usize }` and `WorkspaceAction::UpdateLightboxImage { index: usize, image: LightboxImage }`.
- `app/src/workspace/view.rs:1028`: `lightbox_view: Option<ViewHandle<LightboxView>>` field on `Workspace`.
- `app/src/workspace/view.rs:21710-21737`: `OpenLightbox` action handler. Already does the right thing: `if let Some(handle) = &self.lightbox_view { handle.update(...).update_params(...) } else { create + subscribe + focus }`. No change needed in v1.
- `app/src/workspace/view.rs:21739-21746`: `UpdateLightboxImage` handler delegates to `update_image_at`. Used by the artifacts call site for async URL fetches; v1's local-file path does not use this.
- `app/src/workspace/view.rs:22739-22740`: `lightbox_view` rendered as a child of the main render `Stack`. The new file-tree usage gets this for free.

### Existing image-decode pipeline

- `crates/warpui_core/src/assets/asset_cache.rs:66-87`: `pub enum AssetSource { Async, Bundled, LocalFile { path: String }, Raw }`. v1 builds `LocalFile` entries.
- `crates/warpui_core/src/image_cache.rs:271-365`: `impl Asset for ImageType` `try_from_bytes`. SVG handled at lines 273-282 via `usvg::Tree::from_data` then `resvg`. Raster formats matched at lines 320-364 via `image::guess_format` + `image::ImageReader::with_format(...).decode().into_rgba8()`. WebP and GIF return `AnimatedBitmap` if `decoder.has_animation()` / has multiple frames. Unknown formats return `ImageType::Unrecognized`. **No `image::Limits` or per-format size cap is applied today.** v1 adds caps here (change 4).
- `app/src/util/image.rs`: agent-mode resize/validation utilities (`MAX_IMAGE_PIXELS = 1.15M`, `MAX_IMAGE_DIMENSION = 2000`, `MAX_IMAGE_SIZE_BYTES = 3.75 MB`). NOT inherited by the asset-cache decode path; cited only as the reference point for v1's caps.

### Existing file-tree → workspace open flow (the integration points)

- `app/src/code/file_tree/view.rs:2174-2215`: `fn open_file()` (under `#[cfg(feature = "local_fs")]`). Calls `resolve_file_target_with_editor_choice`, sends `TelemetryEvent::CodePanelsFileOpened { entrypoint: ProjectExplorer, target }`, emits `FileTreeEvent::OpenFile { path, target, line_col: None }`. v1 needs no change here; the new `FileTarget::ImagePreview` flows through unchanged.
- `app/src/code/file_tree/view.rs:2853-2877`: `enum FileTreeEvent` including `OpenFile { path, target, line_col }`.
- `app/src/server/telemetry/events.rs:483-487`: `pub enum CodePanelsFileOpenEntrypoint { CodeReview, ProjectExplorer, GlobalSearch }`. The entrypoint stays `ProjectExplorer`; the new variant on `FileTarget` distinguishes the destination via the `target` field on `CodePanelsFileOpened` (events.rs:2308-2312). No telemetry-enum change needed.
- `app/src/workspace/view/left_panel.rs:758-768`: re-emits `LeftPanelEvent::OpenFileWithTarget`.
- `app/src/workspace/view.rs:5826-5838`: `LeftPanelEvent::OpenFileWithTarget` handler invokes `self.open_file_with_target(path, target, line_col, CodeSource::FileTree { path }, ctx)`.
- `app/src/workspace/view.rs:5715-5815`: `pub fn open_file_with_target(...)`. The match arms today cover `MarkdownViewer(layout)` (line 5739), `EnvEditor` (5744), `CodeEditor(layout)` (5796), `ExternalEditor(editor)` (5800), `SystemDefault` (5808), `SystemGeneric` (5811). The new arm slots in next to `MarkdownViewer`.
- `app/src/util/openable_file_type.rs:32-39`: `pub enum OpenableFileType { Markdown, Code, Text }`. Not changed.
- `app/src/util/openable_file_type.rs:42-57`: `pub enum FileTarget { MarkdownViewer(EditorLayout), CodeEditor(EditorLayout), ExternalEditor(Editor) [cfg local_fs], EnvEditor, SystemDefault, SystemGeneric }`. v1 adds `ImagePreview` here.
- `app/src/util/openable_file_type.rs:71-82`: `pub fn is_supported_image_file()` already matches `"jpg"|"jpeg"|"png"|"gif"|"webp"|"svg"` (case-insensitive). Reused as-is.
- `app/src/util/openable_file_type.rs:142-156`: `is_file_openable_in_warp()` returns `Option<OpenableFileType>`. Not changed; the resolver's new branch runs ahead of this.
- `app/src/util/openable_file_type.rs:194-233`: `resolve_file_target_with_editor_choice()`. v1 adds a new step 0 ahead of step 1.
- `crates/warp_util/src/file_type.rs:46-124`: `is_binary_file()`. Image extensions (`jpg/jpeg/png/gif/bmp/tiff/tif/webp/ico`) are listed as binary. Not changed; the new resolver branch short-circuits before this matters for the supported set.

## Proposed changes

### 1. Add `FileTarget::ImagePreview`

In `app/src/util/openable_file_type.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTarget {
    MarkdownViewer(EditorLayout),
    CodeEditor(EditorLayout),
    ImagePreview,
    #[cfg(feature = "local_fs")]
    ExternalEditor(Editor),
    EnvEditor,
    SystemDefault,
    SystemGeneric,
}
```

`ImagePreview` is a unit variant; no `EditorLayout` payload is needed because the Lightbox is an overlay, not a pane.

`OpenableFileType` is unchanged. The image probe is resolver-only; no caller of `OpenableFileType` would gain anything from a fourth `Image` variant.

### 2. Insert the image branch in `resolve_file_target_with_editor_choice`

In `app/src/util/openable_file_type.rs:194-233`, add a new step ahead of the markdown / code / binary chain:

```rust
pub fn resolve_file_target_with_editor_choice(
    path: &Path,
    editor_choice: EditorChoice,
    prefer_markdown_viewer: bool,
    default_layout: EditorLayout,
    layout: Option<EditorLayout>,
) -> FileTarget {
    // 0. Image preview takes precedence over text/binary classification so SVG
    //    (currently text-classified) and raster formats (currently binary) both
    //    land in the Lightbox.
    if is_supported_image_file(path) {
        return FileTarget::ImagePreview;
    }

    // ... existing steps 1-5 unchanged ...
}
```

Audit before merge: grep for any other call site that pattern-matches `FileTarget` exhaustively. The compiler will catch missing arms in `match`, but boolean-style `matches!(target, FileTarget::CodeEditor(_) | ...)` needs a manual sweep so `ImagePreview` is treated as "in-Warp, do not hand off to OS."

### 3. Add the `FileTarget::ImagePreview` arm in `Workspace::open_file_with_target`

In `app/src/workspace/view.rs:5738-5814`, add an arm next to `MarkdownViewer(layout)`:

First, extend `LightboxImage` in `crates/ui_components/src/lightbox.rs:38-43` with an optional `path` field so each entry carries enough information to lazily promote `Loading` to `Resolved` on navigation without reconstructing paths from filenames:

```rust
#[derive(Clone, Debug)]
pub struct LightboxImage {
    pub source: LightboxImageSource,
    pub description: Option<String>,
    /// The local filesystem path this entry was built from, if any.
    /// `Some` for the file-tree image-preview path; `None` for sources
    /// that don't have a path (artifact downloads, bundled assets, raw bytes).
    /// Used by `LightboxView` to lazily promote `Loading` entries to
    /// `Resolved { LocalFile { path } }` on navigation.
    pub path: Option<PathBuf>,
}
```

Existing call sites that construct `LightboxImage` (artifact screenshots in `app/src/ai/artifacts/mod.rs`, the agent block paths in `app/src/ai/blocklist/`, the markdown-block lightbox trigger) pass `path: None` and continue to work unchanged.

Then the file-tree dispatch arm:

```rust
const PRELOAD_RADIUS: usize = 1;

FileTarget::ImagePreview => {
    let siblings = list_sibling_images_natural_sorted(&path);
    let initial_index = siblings
        .iter()
        .position(|p| p == &path)
        .unwrap_or(0);
    let preload_lo = initial_index.saturating_sub(PRELOAD_RADIUS);
    let preload_hi = (initial_index + PRELOAD_RADIUS + 1).min(siblings.len());
    let images = siblings
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let description = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            let source = if i >= preload_lo && i < preload_hi {
                LightboxImageSource::Resolved {
                    asset_source: AssetSource::LocalFile {
                        path: p.to_string_lossy().into_owned(),
                    },
                }
            } else {
                LightboxImageSource::Loading
            };
            LightboxImage {
                source,
                description,
                path: Some(p.clone()),
            }
        })
        .collect::<Vec<_>>();
    self.dispatch_typed_action(
        WorkspaceAction::OpenLightbox {
            images,
            initial_index,
        },
        ctx,
    );
}
```

Notes on this shape:

- **Bounded preload window**. Only entries within `current ± PRELOAD_RADIUS` are built as `Resolved { LocalFile { path } }`; the rest are built as `Loading` placeholders that still carry the filename (in `description`) and the full path (in `path`). `LightboxView::start_asset_loads` (`lightbox_view.rs:108-114`) iterates only over `Resolved` entries and kicks off `AssetCache::load_asset` for each, so on initial open at most `2 * PRELOAD_RADIUS + 1 = 3` parallel reads/decodes are queued regardless of directory size. With `MAX_DECODED_PIXELS = 32M` (change 4) the per-decode envelope is `~122 MB` of RGBA, so the worst-case transient envelope is `3 * 122 MB ≈ 366 MB`. That is the budget for one file-tree click in pathological-fixture conditions; for normal photo directories actual allocation is a small fraction of that. The previous design (radius 2 + 64M-pixel cap) was ~1.2 GB worst case, which was the right shape but a steeper envelope than a preview UI warrants. The current values err on the side of the user's RAM and still give the immediately-visible image plus its two neighbours decoded ahead.
- **Lazy promotion on navigation**. The `LightboxView::handle_action` arms for `NavigatePrevious` / `NavigateNext` need a small extension: after advancing `current_index`, walk the `±PRELOAD_RADIUS` window around the new index and, for each slot whose `source == Loading` and whose `path == Some(p)`, dispatch `WorkspaceAction::UpdateLightboxImage { index, image: LightboxImage { source: Resolved { asset_source: AssetSource::LocalFile { path: p.to_string_lossy().into_owned() } }, description: <existing>, path: Some(p) } }`. The dispatched action runs through the existing `UpdateLightboxImage` handler at `app/src/workspace/view.rs:21739-21746`, which calls `update_image_at`, which calls `start_asset_load` for that single entry. Slots with `path: None` are skipped (never happens on the file-tree path; defensive for other call sites that might mix sources).
- **Why not artifact-style placeholders all the way down**. The artifact code path (`app/src/ai/artifacts/mod.rs:299-340`) keeps every entry as `Loading` until an async signed-URL fetch completes. Local files have a known `AssetSource` immediately, so the only reason to keep an entry as `Loading` is to defer the asset-cache load itself, which is exactly the bounded-preload behavior above.
- **Handler reuse via `update_params` is unrelated to the file-tree dispatch path.** The `OpenLightbox` handler at `view.rs:21718-21736` reuses an open `lightbox_view` via `update_params`. That path fires only when `OpenLightbox` is dispatched while the Lightbox already has focus, which does not happen from the file-tree click path: clicking the file tree shifts focus, fires `LightboxViewEvent::FocusLost`, and the existing handler at `view.rs:21728-21732` clears `self.lightbox_view` before the new `OpenLightbox` arrives. The reuse path is still useful for non-focus-stealing dispatch sites (artifact / screenshot Lightboxes triggered from agent block panes that do not steal focus from the overlay), so no handler change is needed; v1 simply does not rely on it for the file-tree case.

### 4. Add a decoder-limit guard in `ImageType::try_from_bytes`

In `crates/warpui_core/src/image_cache.rs`, the existing decode paths call `image::ImageReader::with_format(...).decode()` without `image::Limits`. A 65535×65535 PNG decompression bomb decodes to ~16 GB RGBA via `into_rgba8()` and OOMs the process. The same applies to `JPEG`. For `WebP` and `GIF`, `decoder.into_frames().collect_frames()` collects every frame into RAM up front; a tiny animated WebP can decode to gigabytes.

Add a shared decode helper that applies caps:

```rust
const MAX_IMAGE_DIMENSION_PX: u32 = 16_384;
const MAX_DECODED_PIXELS: u64 = 32_000_000; // ~122 MB at RGBA8 (4 bytes per pixel)

fn decode_with_limits(
    data: &[u8],
    format: image::ImageFormat,
) -> anyhow::Result<image::RgbaImage> {
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(data), format);
    reader.limits(image::Limits {
        max_image_width: Some(MAX_IMAGE_DIMENSION_PX),
        max_image_height: Some(MAX_IMAGE_DIMENSION_PX),
        max_alloc: Some(512 * 1024 * 1024),
        ..Default::default()
    });
    let img = reader.decode()?;
    let (w, h) = (img.width(), img.height());
    let pixels = (w as u64).saturating_mul(h as u64);
    if pixels > MAX_DECODED_PIXELS {
        anyhow::bail!(
            "image is too large to preview ({w}x{h}, max {MAX_DECODED_PIXELS} pixels)"
        );
    }
    Ok(img.into_rgba8())
}
```

Use it in the JPEG, PNG, and WebP-static arms.

**Animated WebP and animated GIF: decode only frame 0 in v1.** Product spec scopes animated formats to first-frame-only static preview (continuous playback is a follow-up that will need `Image::enable_animation_with_start_time` plumbing). Decoding every frame to build an `AnimatedBitmap` for a single rendered frame wastes memory and leaves a DoS surface: a 1 KB animated WebP can declare a few hundred 8000x8000 frames totalling many hundreds of MB of decoded RGBA, all of which the Lightbox would then ignore. So in v1 the animated arms collapse to "decode one frame, return `StaticBitmap`":

```rust
Ok(ImageFormat::WebP) => {
    // Always decode as a static image in v1: WebPDecoder yields the first
    // frame whether or not the file is animated, and we don't render
    // animation today.
    let img = decode_with_limits(data, image::ImageFormat::WebP)?;
    Ok(ImageType::StaticBitmap { image: Arc::new(StaticImage { img }) })
}
Ok(ImageFormat::Gif) => {
    // GifDecoder::next_frame returns the first frame without iterating
    // the rest of the file; collect_frames() would buffer everything.
    let mut decoder = GifDecoder::new(std::io::Cursor::new(data))?;
    let (w, h) = (decoder.dimensions().0 as u64, decoder.dimensions().1 as u64);
    if w > MAX_IMAGE_DIMENSION_PX as u64 || h > MAX_IMAGE_DIMENSION_PX as u64 {
        anyhow::bail!("gif frame is too large to preview ({w}x{h})");
    }
    if w.saturating_mul(h) > MAX_DECODED_PIXELS {
        anyhow::bail!("gif frame is too large to preview ({w}x{h})");
    }
    let img = DynamicImage::from_decoder(decoder)?.into_rgba8();
    Ok(ImageType::StaticBitmap { image: Arc::new(StaticImage { img }) })
}
```

`ImageType::AnimatedBitmap` becomes unconstructed in v1. Today no caller in the codebase actually drives animation (the GPUI `Image` element only animates when `enable_animation_with_start_time` is called, and no in-tree call site does), so this is a no-op for behavior beyond reducing memory pressure for animated inputs everywhere; the artifacts/screenshot Lightboxes also benefit. The variant is left in the enum (for the follow-up that re-introduces animation) with a `// Unconstructed in v1; see GH9729 follow-up "Animated GIF/WebP continuous playback"` comment, or removed entirely along with `AnimatedImage` and the `size_in_bytes` arm if the cleanup is preferred. Implementer's choice; leaving it is the safer diff for v1.

Peak per-decode allocation is now bounded by `MAX_DECODED_PIXELS * 4 bytes` for any input, animated or not: the WebP / GIF decoders only ever produce one frame's worth of RGBA before returning. The previous `collect_animated_frames_with_limits` helper is no longer needed and is dropped from the spec.

For SVG, `usvg::Tree::from_data` is bounded by the input bytes. Cap the input bytes at a sensible size (e.g. `MAX_SVG_BYTES = 8 MB`) before calling `usvg::Tree::from_data` so a pathological `<rect>` viewport or deep group nesting cannot allocate gigabytes during render. Render-time limits (`tiny_skia::Pixmap` size) are bounded by the same `MAX_DECODED_PIXELS` applied to the SVG's intrinsic size after parsing.

**Convert `Ok(ImageType::Unrecognized)` to `Err`.** Today the `_` arm of the `match image::guess_format(data)` block at `image_cache.rs:364` returns `Ok(ImageType::Unrecognized)` for any unknown format. That `Ok(...)` propagates through `AssetCache` as `AssetState::Loaded { data: Unrecognized }`, and the Lightbox render path checks `data.image_size()`, which is `None` for `Unrecognized`, so the render falls through to the loading element and the user sees a permanent spinner. Replace the catch-all arm with `Err`:

```rust
match image::guess_format(data) {
    Ok(ImageFormat::Jpeg) => { /* ... */ }
    Ok(ImageFormat::Png)  => { /* ... */ }
    Ok(ImageFormat::WebP) => { /* ... */ }
    Ok(ImageFormat::Gif)  => { /* ... */ }
    Ok(other) => anyhow::bail!("unsupported image format: {other:?}"),
    Err(_)    => anyhow::bail!("could not detect image format from header bytes"),
}
```

The `ImageType::Unrecognized` variant was unreachable in practice once this arm changes; it can be removed entirely (one-call-site cleanup in `size_in_bytes` at `image_cache.rs:378`). Removing it surfaces previously-silent classification failures as `AssetState::FailedToLoad` everywhere, which `LightboxView` then maps to the new `LightboxImageSource::Error { message }` variant from change 5; the user sees a per-entry "could not detect image format from header bytes" inline error with the filename instead of a permanent spinner. This also eliminates a class of failure where a `.png` file containing tarball bytes (extension/content mismatch) would silently degrade to "Loaded but invisible."

The existing agent-mode caps in `app/src/util/image.rs` are stricter (1.15M pixels, 2000 dim, 3.75 MB) because they target on-the-wire payloads to LLMs. The Lightbox preview can afford a larger envelope; the values above were picked to comfortably handle a 4000×3000 photo (12M pixels, well under the 32M cap) and a 5000×5000 PNG (25M pixels, just under the cap) while rejecting the Behavior-5 stress fixture (10000×10000 PNG = 100M pixels, well above the 32M cap). A "below the cap" reference fixture for manual validation should be ~5000×5000 (or smaller); the rejection-path fixture stays at 10000×10000.

These changes affect every consumer of `ImageType::try_from_bytes`, not just the Lightbox file-tree path. Audit:

- `app/src/ai/artifacts/mod.rs` (screenshot lightbox): screenshots are constrained server-side; the new caps do not regress real workloads.
- Any agent attachment / inline preview path: already bounded by `app/src/util/image.rs` caps before hitting `try_from_bytes`.
- UI assets (`Bundled` paths): all Warp-shipped assets are well within the cap.

If any caller needs the old uncapped behavior, factor `try_from_bytes_unbounded` and have only that caller use it; default callers go through the capped path.

### 5. Add `LightboxImageSource::Error` and surface it

In `crates/ui_components/src/lightbox.rs`, extend the enum:

```rust
#[derive(Clone, Debug)]
pub enum LightboxImageSource {
    Loading,
    Resolved { asset_source: AssetSource },
    Error { message: String },
}
```

In `Lightbox::render` (`crates/ui_components/src/lightbox.rs`, the per-image render branch around lines 158-176), add an `Error` arm that renders a non-blocking error panel showing the entry's `description` (filename) and the `message`. Do not throw; do not block prev/next.

In `app/src/workspace/lightbox_view.rs`:

- `start_asset_load` already delegates to `AssetCache::load_asset`. Watch the resulting `AssetState`: when it transitions to `AssetState::FailedToLoad(err)`, mutate `self.params.images[index]` to `LightboxImage { source: LightboxImageSource::Error { message: err.to_string() }, description: <existing> }`. The `ctx.spawn(future, ...)` callback at line 124 already runs on completion; extend it to read the post-load state and rewrite the entry on failure.
- `current_image_native_size` in `render` (lightbox_view.rs:150-165) is unaffected: `Error` entries return `None` for native size, which the existing render logic already tolerates.

This is the only `LightboxImageSource` change. Without it, Behavior 11's "non-blocking error state including filename" cannot render: today's render falls through to the loading element on `AssetState::FailedToLoad` and spins forever. The artifacts call site (`app/src/ai/artifacts/mod.rs:362-365`) silently works around this by stuffing "Failed to load" into the `description` while leaving `source: Loading`; that is a UX bug we inherit if we do not add `Error` here. (Updating the artifacts call site to use `Error` once it exists is a small follow-up included below.)

#### Accessibility label on the rendered image

Product spec requires the active image's filename to be exposed as the rendered Image's accessible label so screen readers announce it. Implement in `crates/ui_components/src/lightbox.rs` `Lightbox::render`, in the per-image render branch around lines 158-176:

- The current `Image::new(asset_source).contain()` element is replaced (or wrapped) with one that sets the GPUI element's accessibility label to `current_image.description.as_deref().unwrap_or("image")`. The exact GPUI API surface is `Element::accessible_label(...)` (or whatever the in-tree element trait calls it; verify against `crates/warpui_core/src/elements/image.rs` at implementation time and use the same accessor that other labeled elements in the codebase use, for example the close-button label at `lightbox.rs:128-143`).
- For `LightboxImageSource::Error { message }` entries, the rendered error panel gets an accessible label of `"image preview failed: {description}: {message}"` so the screen reader announces both the filename and the failure cause.
- For `LightboxImageSource::Loading` entries, the loading indicator gets an accessible label of `"loading {description}"`.

Manual validation step (added to the validation section): with macOS VoiceOver enabled (Cmd+F5), open `screenshot.png` from the file tree and confirm VoiceOver announces something like "image, screenshot.png" rather than a generic "image" with no filename. Repeat with a corrupt fixture and confirm the failure announcement includes the filename.

### 6. Sibling-listing helper

Add `list_sibling_images_natural_sorted` next to `is_supported_image_file` in `app/src/util/openable_file_type.rs`:

```rust
const MAX_SIBLING_IMAGES: usize = 1_024;

#[cfg(feature = "local_fs")]
pub fn list_sibling_images_natural_sorted(path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else {
        return vec![path.to_path_buf()];
    };
    let clicked_is_hidden = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false);
    let mut siblings: Vec<PathBuf> = match std::fs::read_dir(parent) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| is_supported_image_file(p))
            .filter(|p| {
                if clicked_is_hidden {
                    true
                } else {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_none_or(|n| !n.starts_with('.'))
                }
            })
            .collect(),
        Err(_) => vec![path.to_path_buf()],
    };
    if !siblings.iter().any(|p| p == path) {
        siblings.push(path.to_path_buf());
    }
    siblings.sort_by(|a, b| natural_cmp(file_name_lossy(a), file_name_lossy(b)));

    // Window the result around the clicked image so it is never dropped by
    // truncation in pathological directories. We keep up to MAX_SIBLING_IMAGES
    // entries: half before the clicked image, half after, clamped at both
    // ends. If the directory is already within the cap this is a no-op.
    if siblings.len() > MAX_SIBLING_IMAGES {
        let clicked_idx = siblings
            .iter()
            .position(|p| p == path)
            .expect("clicked image is in the sorted list");
        let half = MAX_SIBLING_IMAGES / 2;
        let mut start = clicked_idx.saturating_sub(half);
        let mut end = (start + MAX_SIBLING_IMAGES).min(siblings.len());
        // If we hit the right edge, pull start leftward to fill the window.
        if end - start < MAX_SIBLING_IMAGES {
            start = end.saturating_sub(MAX_SIBLING_IMAGES);
        }
        // If we hit the left edge, push end rightward to fill the window.
        if end - start < MAX_SIBLING_IMAGES {
            end = (start + MAX_SIBLING_IMAGES).min(siblings.len());
        }
        siblings = siblings[start..end].to_vec();
    }
    siblings
}

fn file_name_lossy(p: &Path) -> &str {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    // Split into runs of digits and non-digits; compare digit runs numerically
    // and non-digit runs case-insensitively. Stable for non-ASCII via byte fallback.
    use std::cmp::Ordering;
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek(), bi.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let na: u64 = take_digits(&mut ai);
                let nb: u64 = take_digits(&mut bi);
                match na.cmp(&nb) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
            (Some(ca), Some(cb)) => {
                let la = ca.to_ascii_lowercase();
                let lb = cb.to_ascii_lowercase();
                match la.cmp(&lb) {
                    Ordering::Equal => {
                        ai.next();
                        bi.next();
                    }
                    other => {
                        return other;
                    }
                }
            }
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars>) -> u64 {
    let mut n: u64 = 0;
    while let Some(&c) = it.peek() {
        if let Some(d) = c.to_digit(10) {
            n = n.saturating_mul(10).saturating_add(d as u64);
            it.next();
        } else {
            break;
        }
    }
    n
}
```

Notes:

- The `MAX_SIBLING_IMAGES = 1024` cap bounds the size of the sibling vector for pathological directories (e.g. `node_modules` icon dirs, generated thumbnail dirs). The window-around-clicked logic above guarantees the clicked image is always inside the kept slice: we center the window on the clicked sort index (up to 512 before, 512 after) and clamp at the directory's first/last entries. A naïve `siblings.truncate(MAX_SIBLING_IMAGES)` after sort would silently drop the clicked image if its sort position was beyond the first 1024 entries (so e.g. clicking `zzz.png` in a 2000-image directory would open `aaa.png` instead). The kept slice is also the bound on parallel decode pressure (see change 3 below).
- `natord` is **not** a transitive dependency of the workspace (verified against `Cargo.lock`); the inline `natural_cmp` above is the implementation. (The original draft of this spec claimed `natord` was already available; that claim was wrong and is removed.)
- `read_dir` does not follow symlinks during enumeration, so symlink loops in the parent directory are not a concern. Per-entry symlinks are followed by `is_supported_image_file` (via `path.extension`) and by the asset cache (via `async_fs::read`); a broken symlink surfaces as a per-entry decode/read error per change 5.
- The scan runs synchronously on the UI thread inside `open_file_with_target`. For typical project directories (<1000 files) this is well under one frame on a warm filesystem cache. For NFS / FUSE / very large directories on a cold cache it can stall the UI. v1 accepts this tradeoff; the follow-ups list a background-thread variant if telemetry shows visible freezes.

### 7. No change to `crates/warp_util/src/file_type.rs`

Image extensions in `is_binary_file()` (lines 53-61) stay binary. The new resolver branch in change 2 short-circuits before that check is reached for the supported set. SVG remains text-classified; the new branch routes it to `ImagePreview` first regardless. Audit before merging that no other call site assumes "binary ⇒ `SystemGeneric`" without going through `resolve_file_target_with_editor_choice` (the audit is the same one called out in change 2).

### 8. Telemetry

No telemetry-enum change. The new `FileTarget::ImagePreview` variant flows through the existing `TelemetryEvent::CodePanelsFileOpened { entrypoint: ProjectExplorer, target }` event at `app/src/code/file_tree/view.rs:2202-2208`. Verify before merge that the `target: FileTarget` field is serialized as the variant name (rather than dropped to a string before this site) so dashboards can filter on `image_preview`.

If product later wants to enumerate additional fields on the event (file extension, file size bucket, sibling count), those are additive changes to `TelemetryEvent::CodePanelsFileOpened` and out of scope here.

## End-to-end flow

1. User clicks `screenshot_2.png` in the file tree.
2. `FileTreeView::open_file` (`app/src/code/file_tree/view.rs:2174-2215`) calls `resolve_file_target_with_editor_choice`. Step 0 (change 2) matches `is_supported_image_file` and returns `FileTarget::ImagePreview`.
3. Telemetry is recorded as `CodePanelsFileOpened { entrypoint: ProjectExplorer, target: FileTarget::ImagePreview }`.
4. `FileTreeEvent::OpenFile { path, target: ImagePreview, line_col: None }` is emitted, re-emitted as `LeftPanelEvent::OpenFileWithTarget` by `left_panel.rs:758-768`, and handled by `Workspace::open_file_with_target` at `view.rs:5715-5815`.
5. The new `ImagePreview` arm (change 3) calls `list_sibling_images_natural_sorted`, builds `Vec<LightboxImage>` where:
   - entries within `current ± PRELOAD_RADIUS` get `source: Resolved { LocalFile { path } }`,
   - entries outside the preload window get `source: Loading`,
   - every entry carries `path: Some(p)` and `description: Some(filename)` so navigation can lazily promote `Loading` to `Resolved` later,
   then computes `initial_index` and dispatches `WorkspaceAction::OpenLightbox`.
6. The `OpenLightbox` handler (`view.rs:21710-21737`) creates `lightbox_view` and focuses it. (The handler's `update_params` reuse branch exists but is not exercised on the file-tree path because the file-tree click already moved focus, fired `FocusLost`, and cleared `lightbox_view`; see the Risks section.) `LightboxView::start_asset_loads` queues `AssetCache::load_asset` calls **only for the preload-window entries marked `Resolved`** (5 entries by default), not for every sibling.
7. The Lightbox renders as a child of the workspace's main `Stack` (`view.rs:22739-22740`). The image fits via `Image::new(asset_source).contain()`. `current_image_native_size` is queried on each render from the asset cache; until the active image's bytes have been decoded, the loading indicator is shown. Animated GIF and animated WebP render their first frame statically (continuous playback is a follow-up; `enable_animation_with_start_time` is not wired in v1).
8. Left/Right arrows step through `current_index`. After advancing, `LightboxView::handle_action` walks the new `±PRELOAD_RADIUS` window and, for each `Loading` entry that has `path: Some(p)`, dispatches `WorkspaceAction::UpdateLightboxImage` to promote it to `Resolved { LocalFile { p } }`. The existing `UpdateLightboxImage` handler (`view.rs:21739-21746`) calls `update_image_at`, which calls `start_asset_load` for that single entry. Already-promoted entries are served from the asset cache instantly. Decode failures surface as `LightboxImageSource::Error { message }` (change 5); `try_from_bytes` errors (including the `Unrecognized` → `Err` change) become `AssetState::FailedToLoad` and flow into the same `Error` variant.
9. Escape, scrim click, or × emits `LightboxViewEvent::Close`. The handler at `view.rs:21722-21726` clears `lightbox_view` and calls `focus_active_tab(ctx)` to restore focus to the previously-active pane.

## Risks and mitigations

### Decoder size / pixel cap (Critical)

Without change 4, a maliciously crafted or accidentally huge image file can OOM the process when the user clicks it. Risk applies to PNG, JPEG, animated WebP, animated GIF, and SVG (via render-time pixmap allocation). Change 4 caps both per-decode dimensions and total decoded pixels and bounds SVG input bytes. Mitigation: ship change 4 as part of v1; do not defer.

### Sibling scan on the UI thread for cold-cache slow filesystems

`std::fs::read_dir` on a stalled NFS / sshfs / FUSE / huge `~/Library/Caches`-style directory can block the UI for seconds. Mitigation v1: keep synchronous, accept the tradeoff for typical project directories, and rely on `MAX_SIBLING_IMAGES` to bound the post-`read_dir` work. Follow-up: spawn the scan on the background executor and dispatch `OpenLightbox` from the result, showing a single-image Lightbox first.

### Local-project DoS via parallel sibling decodes (addressed in change 3)

A naïve "build every sibling as `Resolved` and let the cache sort it out" dispatch would queue up to `MAX_SIBLING_IMAGES` parallel asset-cache loads on a single click. With change 4's per-decode cap (~122 MB peak per entry) that is up to ~125 GB of transient allocation pressure before the asset cache's size-budget LRU can evict, since `image::decode` allocates the full `RgbaImage` before returning. A user could trigger this just by clicking inside a directory full of large images.

Change 3 closes this by building only `current ± PRELOAD_RADIUS` entries as `Resolved` and the rest as `Loading`, with a navigation hook that promotes new neighbours to `Resolved` (one at a time) on Left/Right. Peak parallel decodes are therefore bounded by `2 * PRELOAD_RADIUS + 1 = 3` (with the chosen `PRELOAD_RADIUS = 1`), giving a worst-case transient envelope of `~3 * MAX_DECODED_PIXELS * 4 bytes ≈ 366 MB` regardless of directory size. Asset-cache eviction handles steady-state pressure for users who arrow through hundreds of images. The cap and radius were intentionally chosen to keep the worst-case decode envelope under ~400 MB, which is a defensible budget for an in-app preview triggered by a single click; tightening further (radius 0, smaller cap, or serializing decodes behind a semaphore) is enumerated as a follow-up if real-world telemetry shows the budget is still too generous.

### Existing `lightbox_view` collision (already-handled, but does not apply to file-tree path)

The `OpenLightbox` handler at `view.rs:21718-21736` already reuses an open `LightboxView` via `update_params`, so dispatching `OpenLightbox` while the overlay is focused does not stack two overlays. This benefits non-focus-stealing dispatch sites (artifact / screenshot Lightboxes invoked from agent block panes that do not move focus). It does **not** apply to the file-tree path: clicking the file tree shifts focus, fires `LightboxViewEvent::FocusLost`, and the existing handler at `view.rs:21728-21732` clears `self.lightbox_view` before any new `OpenLightbox` arrives. The file-tree click on a second image therefore goes through the cold-open branch every time, which is the documented behavior in the product spec. No handler change is needed.

### `LightboxImageSource::Error` is a public-API change in `ui_components`

Adding a new variant to a public enum is a breaking change for any external consumer. There are none today. All in-tree consumers are updated as part of change 5: `crates/ui_components/src/lightbox.rs` render, `app/src/workspace/lightbox_view.rs` asset-load callback, `app/src/ai/artifacts/mod.rs:362-365` (drop the `Loading + "Failed to load"` workaround in favor of the new `Error` variant).

### `SystemGeneric` regression for non-image binary files

The new resolver branch is gated strictly on `is_supported_image_file(path)`. Non-image binary extensions (`.zip`, `.mp3`, `.exe`, `.pdf`, `.bmp`, `.tiff`, `.ico`) skip the new branch and continue to fall through to `SystemGeneric` exactly as before. Covered by unit tests in the validation section.

### Telemetry distinguishability

Verified above (change 8): the existing event already carries `target: FileTarget`, so adding `ImagePreview` to the enum is sufficient. One-line audit: confirm the event serializer serializes the variant name and not a pre-flattened string; if it does flatten, add `ImagePreview` to the flattening map.

### SVG via `Image::new`

SVG is rendered via `usvg` 0.47 + `resvg` in `ImageType::Svg` (`image_cache.rs:271-282`). `usvg` 0.47 disables network and external-entity expansion by default. The remaining concrete risks (deep group nesting causing render-time stack pressure; pathological viewport causing huge `tiny_skia::Pixmap` allocation) are bounded by change 4's SVG-bytes cap and `MAX_DECODED_PIXELS` applied to the SVG's intrinsic size after parse. Smoke-test with one well-formed SVG fixture and one pathological fixture; do not defer SVG to a follow-up.

## Testing and validation

### Unit tests

`app/src/util/openable_file_type.rs` (new test module section):

- `resolve_file_target_image_preview_for_each_supported_extension`: each of `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg` (lower and upper case) resolves to `FileTarget::ImagePreview`, regardless of `prefer_markdown_viewer` and `editor_choice`.
- `resolve_file_target_image_preview_takes_precedence_over_markdown`: a `.svg` file resolves to `ImagePreview` even when `prefer_markdown_viewer = true`.
- `resolve_file_target_non_image_binary_still_system_generic`: `.zip`, `.mp3`, `.exe`, `.pdf`, `.bmp`, `.tiff`, `.ico` resolve to `SystemGeneric`.
- `natural_cmp_orders_numeric_runs`: `["a10.png", "a2.png", "A11.png", "a1.png"]` sorts to `["a1.png", "a2.png", "a10.png", "A11.png"]`.
- `natural_cmp_case_insensitive_for_letters`: `Image.png` and `image.png` compare equal.
- `list_sibling_images_filters_hidden_when_clicked_visible`: with a temp dir containing `a.png`, `.b.png`, `c.png`, clicking `a.png` returns `[a.png, c.png]`.
- `list_sibling_images_includes_hidden_when_clicked_hidden`: with the same temp dir, clicking `.b.png` returns `[.b.png, a.png, c.png]` in natural order.
- `list_sibling_images_window_keeps_clicked_at_left_edge`: with a fixture of 2000 image files (`img0001.png` through `img2000.png`), clicking `img0050.png` returns a slice that starts no later than `img0050.png` and contains it.
- `list_sibling_images_window_keeps_clicked_at_right_edge`: with the same fixture, clicking `img1990.png` returns a slice that contains `img1990.png` and is exactly `MAX_SIBLING_IMAGES` long (the right-edge clamp pulls `start` leftward).
- `list_sibling_images_window_centers_clicked_in_middle`: with the same fixture, clicking `img1000.png` returns a slice of length `MAX_SIBLING_IMAGES` with `img1000.png` near the center.
- `list_sibling_images_returns_full_list_when_under_cap`: with a fixture of 100 images, returns all 100 in natural order.
- `list_sibling_images_includes_clicked_when_directory_below_cap`: edge case; the clicked file is always in the result.

`crates/warpui_core/src/image_cache.rs` (new test module section):

- `decode_with_limits_rejects_huge_dimensions`: a synthesized PNG header declaring `20000 x 20000` returns `Err`.
- `decode_with_limits_accepts_normal_photo`: a 4000×3000 PNG decodes successfully.
- `try_from_bytes_returns_err_for_garbage`: `[0xff, 0xff, 0xff, 0xff]` returns `Err(...)` (was previously `Ok(ImageType::Unrecognized)`; the variant is removed in change 4).
- `try_from_bytes_decodes_only_first_frame_of_animated_webp`: a fixture animated WebP with N declared frames returns `Ok(StaticBitmap)` whose pixel count matches one frame, not N (asserted by checking `size_in_bytes()` against `width * height * 4`).
- `try_from_bytes_decodes_only_first_frame_of_animated_gif`: same shape for animated GIF.
- `try_from_bytes_caps_svg_input_bytes`: an SVG payload above `MAX_SVG_BYTES` returns `Err` rather than parsing.

### Manual validation

Behavior-to-step mapping (numbered against the product spec's User Experience and Success Criteria sections):

- **Opening, keyboard entry, multi-window**: click each of `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`. Confirm Lightbox opens, no new tab is created, no session-restore artifact appears on relaunch. Open the same image via tree-arrows + Enter. Open two workspace windows; confirm the Lightbox attaches to the originating window only.
- **Inside the Lightbox**: open one small image (smaller than viewport) and one large image (e.g. 5000×5000 PNG = 25M pixels, just below the 32M cap). Aspect is preserved; no upscale beyond native dimensions. Confirm scrim covers the whole workspace including any split panes; underlying panes are visible but inert.
- **Loading state and decode error**: open a slow-loading fixture (NFS or artificially throttled). Confirm the spinner shows and that pressing arrows during the load advances the index without blocking. Then open a corrupt PNG and confirm the Lightbox shows the per-entry error with the filename, arrows move to the neighbour normally, and no crash occurs.
- **Decoder cap**: open a 10000×10000 PNG (above the cap). Confirm the Lightbox shows the per-entry error citing the size, not a partial render or an OOM.
- **Navigation order and bounds**: open `image1.png` in a directory with `image1.png, image2.png, image10.png, IMAGE11.png`. Right arrow visits them in `1, 2, 10, IMAGE11` order. Left arrow at `image1.png` and right arrow at `IMAGE11.png` are no-ops (controls are hidden).
- **Hidden files**: open `a.png` in a directory containing `a.png, .b.png, c.png`: arrows visit `[a.png, c.png]` only. Open `.b.png`: arrows visit all three.
- **Open second image after first**: with the Lightbox open on `a.png`, click `b.png` in the file tree. The first Lightbox dismisses via `FocusLost` (file-tree click steals focus) and the second opens cold on `b.png`. No second scrim stacks. The user-visible result is "showing `b.png` now," achieved through dismiss-then-open rather than in-place replace.
- **Lazy preload window**: open `image500.png` in a directory of 1000 images. Confirm via inspector / log that only `current ± PRELOAD_RADIUS` siblings (with `PRELOAD_RADIUS = 1`, that means `image499.png`, `image500.png`, `image501.png`) have triggered `AssetCache::load_asset`. Press Right arrow; confirm the new edge of the window (`image502.png`) starts loading at that point, not earlier.
- **Identity click**: with the Lightbox open on `a.png`, click `a.png` again in the tree. Lightbox stays on `a.png`, no flicker, no error.
- **Non-image click while open**: with the Lightbox open, click `README.md` in the tree. Markdown viewer opens; Lightbox dismisses via `FocusLost` (handler at view.rs:21728-21732). Confirm focus is on the markdown viewer.
- **Dismiss paths and focus**: dismiss via Escape, scrim click, and × button; in each case focus returns to the previously-active tab pane.
- **SVG, animated GIF, animated WebP**: open one of each from a fixture. Confirm SVG renders (not raw XML, not blank). Confirm animated GIF and animated WebP render their first frame statically (no continuous playback in v1; the `Image` element is not given `enable_animation_with_start_time`). Document any rendering anomalies as follow-ups; do not block on them unless they crash.
- **Filesystem mutation**: open the Lightbox, delete the file from outside Warp. Active entry transitions to the per-entry error state; navigation works.
- **No regression for non-image binaries**: click `.zip`, `.mp3`, `.exe`, `.pdf` files. They open in the OS default app (`SystemGeneric`) exactly as today.
- **Telemetry**: with telemetry inspection enabled, click an image and confirm `CodePanelsFileOpened` fires with `target: ImagePreview`.
- **Accessibility**: enable macOS VoiceOver (Cmd+F5). Click `screenshot.png` from the file tree. Confirm VoiceOver announces the filename rather than a generic "image" label. Open a corrupt fixture and confirm the failure announcement includes both the filename and the failure cause.

### Runtime checks

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --tests -- -D warnings`
- `cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2`

## Follow-ups

- **Move sibling scan to the background executor** and dispatch `OpenLightbox` from the result for cold-cache or pathological directories. Open the clicked image immediately as a single-image Lightbox; replace in place once the scan completes.
- **Extend `LightboxImageSource::Error` adoption to the artifacts call site** (`app/src/ai/artifacts/mod.rs:362-365`) so screenshot fetch failures use `Error` instead of `Loading + "Failed to load"` description.
- **Zoom and pan controls**: extend `lightbox::Params` with zoom state and `lightbox_view.rs` keybindings (`+`, `-`, `0`, drag-to-pan).
- **Status footer** (filename, dimensions, file size, format string): extend `lightbox::Params` with an optional metadata strip rendered below the image and above the description slot.
- **Animated GIF / WebP continuous playback**: wire `Image::enable_animation_with_start_time(Instant)` (`crates/warpui_core/src/elements/image.rs:128`) into the Lightbox image element and drive a per-frame redraw loop on the focused entry. Track per-image start time so navigation restarts animation from frame 0 deterministically. **Play/pause control** (Lightbox button-strip toggle) is the next layer on top.
- **EXIF orientation and ICC color profile**: extend the agent-mode decoder in `app/src/util/image.rs` and wire into `ImageType::try_from_bytes` so the Lightbox receives oriented, color-correct frames.
- **Visible thumbnail strip**: new component sibling to `Lightbox`; populate from `list_sibling_images_natural_sorted`.
- **Additional raster formats** (HEIC, HEIF, AVIF, BMP, TIFF, ICO): depends on backend `image`-crate features and decoder availability; reclassify in `is_supported_image_file` and re-test the cap behavior.
- **Magic-byte content sniffing**: extend `crates/warp_util/src/file_type.rs` to read the first N bytes when the extension claims an image; route mismatches as a non-blocking warning rather than failing the open.
- **Right-click context menu**: wire `Copy Image`, `Copy file path` (relative/absolute), `Reveal in Finder/Files`, `Attach as Agent context` on the Lightbox image surface.
- **Drag-out to attach as Agent context**: share the payload type used by `app/src/terminal/input.rs::handle_pasted_or_dragdropped_image_filepaths`.
- **Disk-backed thumbnail cache and size-cap setting**: only relevant once the visible thumbnail strip lands.
- **SVG `size_in_bytes`** currently returns 0 (`image_cache.rs:370`), so SVGs do not count against the asset-cache eviction budget. Compute a reasonable proxy (e.g. `data.len()` or rasterized pixmap size) so the cache can evict them.
- **Image diff across git revisions**: render two `Lightbox`-style panes side by side, tied into the existing diff infrastructure.
- **Slideshow / fullscreen mode**: auto-advance with a configurable interval.
- **RAW formats** (CR2, NEF, ARW, DNG): pulls in a much larger decoder dependency; gate behind a feature flag.
- **Remote URL preview**: open `https://...` images directly from clipboard or terminal hyperlink without a local file round-trip.
