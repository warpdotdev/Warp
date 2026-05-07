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
- [ ] **t2-5.** Adopt `LightboxImageSource::Error` at the artifacts call
       site (`app/src/ai/artifacts/mod.rs:362-365`) so screenshot fetch
       failures use `Error` instead of `Loading + "Failed to load"`. —
       `tech.md` §696
- [ ] **t2-6.** Animated GIF/WebP continuous playback. Wire
       `Image::enable_animation_with_start_time(Instant)` into the Lightbox
       image element; drive a per-frame redraw on the focused entry. Adds
       a basic play/pause control as a follow-on sub-step. — `tech.md` §697
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
| t2-4 | `Unrecognized` → `Err` globally | (see commit message) | [x] | [ ] | [ ] |
| t2-5 | adopt `Error` at artifacts call site | | [ ] | [ ] | [ ] |
| t2-6 | animated playback (+ play/pause) | | [ ] | [ ] | [ ] |
| t2-7 | zoom and pan | | [ ] | [ ] | [ ] |
| t2-8 | status footer | | [ ] | [ ] | [ ] |
| t2-9 | EXIF orientation + ICC | | [ ] | [ ] | [ ] |
| t2-FINAL | presubmit | | [ ] | — | — |

Tick `[x]` only after the corresponding artifact (commit for `Impl`, review
file for `R1`/`R2`) exists and contains real content. Empty stubs do not
count.
