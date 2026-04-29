# 04 — Open Changes panel

**Phase:** not-started
**Spec PR:** —
**Impl PRs:** —

## Scope

VS Code Source Control–style side panel: working/staged file separation, inline diffs, file & hunk staging, commit message + commit/push/pull, and file Timeline (history). See README §4. Match VS Code behavior-for-behavior where it makes sense.

## Sub-phases

- [ ] **4a — Panel scaffold + working/staged file lists** (read-only, no diff yet).
- [ ] **4b — Inline diff view** for the focused file.
- [ ] **4c — Stage / unstage / discard** at file and hunk granularity.
- [ ] **4d — Commit message input + commit / push / pull** controls.
- [ ] **4e — File Timeline (history view)** for the focused file.

## Notes

- 4e is the most likely scope-cut candidate if the surface area gets unmanageable; treat as a stretch goal — the feature can ship as `merged` without it if 4a–4d are solid.
- Spec phase produces one PRODUCT.md / TECH.md covering all of 4a–4e, then each sub-phase ships its own impl PR.
- Backed by the same git plumbing the terminal already uses (no new daemons, no LSP-style sidecar).

## Why this is feature 04 (last)

Largest surface area; most UI; benefits the most from a stable foundation (post-AI-removal). Saving it for last means we ship it onto a tree that already reflects the fork's identity.
