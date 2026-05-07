# GH9729 image-preview — implementation TODO

Authoritative spec: `specs/GH9729/tech.md` (do **not** edit).
Branch: `spec/GH9729-image-preview` (this branch).

This file is the single source of truth for the Ralph loop driving the
implementation. The loop reads it each iteration, picks the first unchecked
item, does **only** that item, flips its checkbox in the same commit, and
stops. The next iteration sees the updated state and picks the next item.

## Hard rules for every iteration

- Touch only the files the current item requires. Never re-read the whole
  `tech.md` — open just the section the item points to.
- Use the `Explore` subagent for codebase lookups; do not grep from the main
  context window.
- Run only the narrowest tests relevant to the change. Save the
  full-workspace `cargo fmt` / `clippy -D warnings` / `nextest` for the FINAL
  step.
- Keep diffs small (≤ ~150 lines where reasonable). Split if larger.
- Commit prefix: `GH9729(impl):` followed by a one-line summary.
- Do not edit `specs/GH9729/product.md` or `specs/GH9729/tech.md`.
- After finishing the item, mark its checkbox `[x]` in the same commit and
  stop. Do **not** start the next item.

## Steps

- [x] 1a. Add `FileTarget::ImagePreview` variant — `tech.md` §74 (`### 1.`)
- [x] 1b. Resolver short-circuit in `resolve_file_target_with_editor_choice`
       (PNG/JPEG/GIF/WebP/SVG ahead of markdown probe and binary fall-through)
       — `tech.md` §74
- [x] 1c. Unit tests for resolver: each supported extension, precedence over
       markdown, non-image binary still `SystemGeneric` — `tech.md` §613
**Spec narrowing note (commit `112e581`).** v1 is single-image, no sibling
navigation. The `list_sibling_images_natural_sorted` helper, `MAX_SIBLING_IMAGES`
cap, hidden-file filter, and natural-sort tests that the original PR body
mentioned are explicitly out of scope per `tech.md` §119 ("Single-element Vec...
v1 always passes a one-element vec") and the descope listed in `product.md`.
The original 2a / 2c bullets are struck through and superseded.

- [x] 3a. `LightboxImageSource::Error { message }` variant in
       `crates/ui_components/src/lightbox.rs` — `tech.md` §182 (`### 3.`).
       Promoted ahead of the workspace arm because §119 references it.
- [x] 3b. Render the `Error` variant inline in `Lightbox::render` (filename
       on one line, message on the next; non-blocking, dismissal still works)
       — `tech.md` §182. Adopting `Error` at `app/src/ai/artifacts/mod.rs:362-365`
       is explicitly a follow-up per §182 and is NOT in this bullet.
- [x] 2-arm. Workspace `FileTarget::ImagePreview` arm: synchronous
       `metadata` size/regular-file check (`MAX_PREVIEW_FILE_BYTES = 64 MB`),
       `truncate_message` helper (`MAX_ERROR_MESSAGE_LEN = 256`), single-element
       `OpenLightbox` dispatch — `tech.md` §119
- [x] 2-tests. Workspace-arm tests per `tech.md` §613 (lines 623-626):
       `image_preview_arm_dispatches_resolved_when_under_size_cap`,
       `image_preview_arm_dispatches_error_when_over_size_cap`,
       `image_preview_arm_dispatches_error_when_metadata_fails`,
       `image_preview_arm_dispatches_error_for_non_regular_file`
- ~~[ ] 2a. `list_sibling_images_natural_sorted` helper + `MAX_SIBLING_IMAGES`
       cap (1024) + hidden-file filter — out of scope (single-image v1)~~
- ~~[ ] 2c. Unit tests for natural sort, hidden-file filter, sibling cap —
       out of scope (single-image v1)~~
- [x] 4a. Static-decode caps in `ImageType::try_from_bytes`:
       `image::Limits` (max dimension, max alloc) + `MAX_DECODE_PIXELS = 67M`
       — `tech.md` §234 (Static raster, under §217)
- [x] 4b. Animated decode caps: `MAX_ANIMATED_FRAMES`, `MAX_ANIMATED_TOTAL_PIXELS`
       — `tech.md` §259 (Animated WebP / GIF, under §217)
- [ ] 4c. SVG content-sanity prefix check + intrinsic-dimension cap —
       `tech.md` §321
- [ ] 4-tests. Decoder tests per `tech.md` §613 (lines 640-652).
- [ ] 5a. Bound the `LocalFile` asset-cache read with content-keyed cap
       (raster vs SVG by 1 KB content peek), post-open `is_file()` check,
       and `O_NONBLOCK` regression guard — `tech.md` §400 (`### 5.`)
- [ ] 5b. `lightbox_view.rs` post-load callback rewrites `FailedToLoad` /
       `Unrecognized` to `Error` — `tech.md` §613 (lines 660-661)
- [ ] 5-tests. Asset-cache tests per `tech.md` §613 (lines 628-636).
- [ ] 7.  Telemetry events for image-preview open / error / cap-hit —
       `tech.md` §517 (`### 7.`)
- [ ] FINAL. Run `cargo fmt --check`, `cargo clippy --workspace --all-targets
       --tests -- -D warnings`, and `cargo nextest run --no-fail-fast
       --workspace --exclude command-signatures-v2`. Fix any fallout in
       follow-up commits, then output `<promise>GH9729 IMPL COMPLETE</promise>`.

## Notes

- §6 in the tech spec is intentionally a no-op ("No change to
  `crates/warp_util/src/file_type.rs`"); it is omitted from this list.
- If a step's tests reveal the change is too large, split it into smaller
  follow-up commits but keep the checkbox unchecked until all sub-changes
  for that bullet land.
