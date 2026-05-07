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
- [ ] 2a. `list_sibling_images_natural_sorted` helper + `MAX_SIBLING_IMAGES`
       cap (1024) + hidden-file filter mirroring the clicked file —
       `tech.md` §119 (`### 2.`)
- [ ] 2b. `Workspace::open_file_with_target` arm: build `Vec<LightboxImage>`
       and dispatch `WorkspaceAction::OpenLightbox` — `tech.md` §119
- [ ] 2c. Unit tests for natural sort, hidden-file filter, sibling cap —
       `tech.md` §613
- [ ] 3a. `LightboxImageSource::Error { message }` variant in
       `crates/ui_components/src/lightbox.rs` — `tech.md` §182 (`### 3.`)
- [ ] 3b. Render the new variant inline; drop the
       `app/src/ai/artifacts/mod.rs:362-365` "Failed to load" workaround —
       `tech.md` §182
- [ ] 4a. `image::Limits` (max dimension, max alloc) + `MAX_DECODED_PIXELS`
       cap in `ImageType::try_from_bytes` — `tech.md` §217 (`### 4.`)
- [ ] 4b. `MAX_SVG_BYTES` input cap — `tech.md` §321
- [ ] 4c. Unit tests: huge dimension rejected, normal photo accepted,
       garbage returns `Unrecognized`, oversize SVG rejected — `tech.md` §613
- [ ] 5.  Bound the `LocalFile` asset-cache read — `tech.md` §400 (`### 5.`)
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
