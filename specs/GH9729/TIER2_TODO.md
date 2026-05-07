# GH9729 image-preview — Tier 2 follow-up TODO

Authoritative spec: `specs/GH9729/tech.md` §688-713 (do **not** edit).
Branch: `spec/GH9729-image-preview` (this branch).
Predecessor: `IMPLEMENTATION_TODO.md` (v1, complete; awaiting external review).

This file drives a fused ralph-loop for **Tier 2 (UX polish)** of the post-v1
follow-up list in `tech.md` §688-713. The post-v1 *Tier 1* items
(a11y plumbing §692, sibling navigation §693, background-executor decode/stat
§694) are **out of scope** for this loop and will be tracked separately.

## Loop semantics — fused

Each iteration:

1. Read this file. Locate the **Tracker** table.
2. Find the first row with any unchecked box across `Impl | R1 | R2`.
   - If `Impl` is `[ ]`: do just the implementation, commit
     (`GH9729(tier2-impl): <item> — <one-line>`), tick `Impl`, stop.
   - Else if `R1` is `[ ]`: spawn one R1-correctness reviewer, write
     `specs/GH9729/reviews/tier2-<item>-r1.md`, commit
     (`GH9729(tier2-review): <item> R1 — <verdict>`), tick `R1`, stop.
   - Else if `R2` is `[ ]`: same with R2-quality, suffix `-r2.md`.
3. If every row has all three boxes ticked, output
   `<promise>ALL TIER2 ITEMS DONE</promise>` and exit.

Hard rules:

- Touch only the files the current iteration requires. Use the `Explore`
  subagent for codebase lookups; do not grep from the main context window.
- Never edit `specs/GH9729/product.md` or `specs/GH9729/tech.md`.
- If an item's design is under-specified in `tech.md`, surface it in the
  reviewer findings rather than committing a guessed shape. If the impl
  agent cannot proceed without a design call, mark the row's `Impl` cell
  `[blocked]` (not `[x]`) and skip to the next row.
- Run only the narrowest tests for the change. The full presubmit lives in
  the `t2-FINAL` row at the bottom.
- Commit prefix as listed under loop semantics above.
- Reviews use the same frontmatter shape as v1 (`reviewer:`, `verdict:`,
  `spec_ref:`); see `REVIEW_LOOP_PROMPT.md` for the exact template.

## Steps (priority order from `tech.md` §688-713)

- [x] **t2-4.** Convert `ImageType::Unrecognized` to `Err` globally — audit
       every `try_from_bytes` caller, remove the variant, route the error
       through `Result`, update callers to handle the `Err` arm. — `tech.md` §695
- [x] **t2-5.** Adopt `LightboxImageSource::Error` at the artifacts call
       site (`app/src/ai/artifacts/mod.rs:362-365`) so screenshot fetch
       failures use `Error` instead of `Loading + "Failed to load"`. —
       `tech.md` §696
- [x] **t2-6.** Animated GIF/WebP continuous playback. Wire
       `Image::enable_animation_with_start_time(Instant)` into the Lightbox
       image element; drive a per-frame redraw on the focused entry. The
       play/pause sub-layer is deferred — see Deferred R2 follow-ups
       below — because GPUI's `Image` element has no
       `paused_at`/freeze-elapsed primitive today, so a real (continuity-
       preserving) pause needs an upstream API addition rather than a
       call-site hack. — `tech.md` §697
- [ ] **t2-7.** Zoom and pan. Extend `lightbox::Params` with zoom/pan state;
       add `+`, `-`, `0`, drag-to-pan keybindings in `lightbox_view.rs`. —
       `tech.md` §698
- [ ] **t2-8.** Status footer. Extend `lightbox::Params` with an optional
       metadata strip (filename, dimensions, file size, format string)
       rendered below the image. — `tech.md` §699
- [ ] **t2-9.** EXIF orientation + ICC color profile. Extend the agent-mode
       decoder in `app/src/util/image.rs` and wire into
       `ImageType::try_from_bytes`. — `tech.md` §700
- ~~**t2-10.** Visible thumbnail strip — **BLOCKED** on Tier 1 sibling
       navigation (`tech.md` §693). Out of scope for this loop.~~
- [ ] **t2-FINAL.** Presubmit (no R1/R2 rows): `cargo fmt`; `cargo clippy
       --workspace --exclude command-signatures-v2 --all-targets --tests --
       -D warnings`; `cargo nextest run --no-fail-fast --workspace
       --exclude command-signatures-v2`.

## Tracker

| # | Item | Impl commit | Impl | R1 | R2 |
|---|------|-------------|------|----|----|
| t2-4 | `Unrecognized` → `Err` globally | `7780d31` | [x] | [x] | [x] |
| t2-5 | adopt `Error` at artifacts call site | `5a8072a` | [x] | [x] | [x] |
| t2-6 | animated playback (continuous; pause deferred) | _pending_ | [x] | [ ] | [ ] |
| t2-7 | zoom and pan | | [ ] | [ ] | [ ] |
| t2-8 | status footer | | [ ] | [ ] | [ ] |
| t2-9 | EXIF orientation + ICC | | [ ] | [ ] | [ ] |
| t2-FINAL | presubmit | | [ ] | — | — |

Tick `[x]` only after the corresponding artifact (commit for `Impl`, review
file for `R1`/`R2`) exists and contains real content. Empty stubs do not
count.

## Deferred R2 follow-ups

Per the loop's "surface, don't fix" rule, R2-quality nits are recorded
here for an off-loop cleanup pass after the main tier-2 list lands.

- **t2-4-r2.** (1) No regression test loads garbage bytes through
  `ImageType::try_from_bytes` and asserts the resulting "could not
  detect image format" string — future wording drift would silently
  break `sanitize_load_error`'s prefix match. (2) The rewritten test
  `post_load_callback_rewrites_unrecognized_to_error` still carries
  the legacy variant name; rename to reflect the `FailedToLoad` path.
  (3) No direct unit test of `sanitize_load_error` proves the
  "could not detect" branch sits ahead of the generic
  "decode/format" branch — a swap regression would silently widen
  the bucket. — `reviews/tier2-t2-4-r2.md`.
- **t2-6-pause.** Play/pause control for the lightbox's animated
  playback. Real pause-resume needs `Image` (GPUI element in
  `crates/warpui_core/src/elements/image.rs`) to expose either a
  `paused_at: Option<Instant>` field or a frozen-elapsed-millis
  parameter so `paint_animated_image` can hold the current frame
  without skipping `ctx.repaint_after`. The two call-site-only
  workarounds (rebuild `started_at = now() - paused_elapsed`, or
  drop `enable_animation_with_start_time` while paused) either
  silently keep advancing the frame or jump back to frame 0 on
  resume — neither is acceptable as v1.x UX. Belongs in a separate
  PR that touches the GPUI element first; tracked as a sub-bullet
  here so t2-6 can land its primary deliverable.
- **t2-5-r2.** (1) Categorical `LightboxImageSource::Error` messages
  now live in two modules (`lightbox_view.rs::sanitize_load_error`
  plus the artifacts call site); consider a shared catalog once the
  third site lands. (2) `LightboxImage` lacks a constructor helper
  (e.g., `LightboxImage::error(message)`) so each call site uses
  verbose struct-literal form. (3) Test name
  `surfaces_error_variant_for_screenshot_load_errors` is accurate
  about the variant but does not capture the sanitization-of-error-text
  property the body asserts. — `reviews/tier2-t2-5-r2.md`.
