# 05 — Open Changes panel

**Phase:** not-started
**Spec PR:** —
**Impl PRs:** —

## Scope

VS Code Source Control–style side panel: working/staged file separation, inline diffs, file & hunk staging, commit message + commit/push/pull, and file Timeline (history). See README §4. Match VS Code behavior-for-behavior where it makes sense.

## Sub-phases

- [ ] **5a — Panel scaffold + working/staged file lists** (read-only, no diff yet).
- [ ] **5b — Inline diff view** for the focused file.
- [ ] **5c — Stage / unstage / discard** at file and hunk granularity.
- [ ] **5d — Commit message input + commit / push / pull** controls.
- [ ] **5e — File Timeline (history view)** for the focused file.

## Notes

- 5e is the most likely scope-cut candidate if the surface area gets unmanageable; treat as a stretch goal — the feature can ship as `merged` without it if 5a–5d are solid.
- Spec phase produces one PRODUCT.md / TECH.md covering all of 5a–5e, then each sub-phase ships its own impl PR.
- Backed by the same git plumbing the terminal already uses (no new daemons, no LSP-style sidecar).

## Why this is feature 05 (last user-facing scope)

Largest surface area; most UI; benefits the most from a stable foundation (post-AI-removal). Slotted just before the rebrand so it ships onto a tree that already reflects the fork's identity. The small markdown-render-default change at 03 is a default flip, not a structural feature, so it doesn't displace this one as the last big user-visible build.
