# 06 — Rebrand to twarp

**Phase:** not-started
**Spec PR:** —
**Impl PRs:** —

## Scope

Rename every "Warp" / "warp" reference in the codebase to "Twarp" / "twarp" — crate names, binaries, bundle IDs, brand assets, UI strings, native bundles, installers, internal feature names, repo hygiene files. Preserve attribution to upstream Warp where AGPL or trademark law requires it.

## Why last

File and crate renames are the worst case for git merges; doing this last keeps upstream cherry-picks clean for as long as possible. By the time 06 starts, 02 has already removed the AI surface (a chunk of the brand-bearing code), so there's less to rename.

## Scale (snapshot, 2026-05-03)

- ~21,400 case-insensitive matches across ~2,540 files
- 17 `warp_*` / `warpui*` crates
- 5 release channels (stable, local, oss, dev, preview), each with bundle ID + .desktop + icon set
- ~25 brand SVG/PNG/BMP assets
- Native bundles: macOS DockTilePlugin, Linux RPM/deb/Arch/AppImage, Windows installer

## Sub-phases

- [ ] **6a — Audit doc.** Enumerate every file/identifier/asset, classify each as rename / replace / keep / regenerate. Output: `roadmap/06-rebrand/AUDIT.md`. No code change. Drives 6b–6j.
- [ ] **6b — Workspace + crate renames.** All `warp_*` → `twarp_*`, `warpui*` → `twarpui*`. Updates `Cargo.toml` workspace + every `use` import.
- [ ] **6c — CLI binary + URL scheme.** `app/Cargo.toml` binaries (`warp` → `twarp`, `warp-oss` → `twarp-oss`); `warp://` URI scheme → `twarp://`.
- [ ] **6d — Bundle IDs + native plists.** `dev.warp.*` → `dev.twarp.*` across all 5 channels; rename `WarpDockTilePlugin.{m,h}`; update `DockTilePlugin/Info.plist`, `CLI-Info.plist`.
- [ ] **6e — Brand assets.** Replace logo SVGs, regenerate channel icon sets, rename files (`warp-*` → `twarp-*`), update references.
- [ ] **6f — User-facing strings.** UI labels, error messages, help text, settings, about page, onboarding copy.
- [ ] **6g — Build scripts + installers.** Linux RPM/deb/Arch/AppImage, Windows `.iss`, macOS bundling, GitHub workflow filenames.
- [ ] **6h — Servers + telemetry.** Stub or redirect `app.warp.dev` URLs (auto-update, login, telemetry). After 02 the surface is much smaller.
- [ ] **6i — Internal feature renames.** `warp_drive` → `twarp_drive`, `warpify` → `twarpify`, `warp_pack` → `twarp_pack`, `open_in_warp` → `open_in_twarp`. Also `.warp/` → `.twarp/`, `.warpindexingignore` → `.twarpindexingignore`, `WARP.md` → `TWARP.md`.
- [ ] **6j — Cleanup sweep.** `rg` for any remaining `\b[Ww]arp\b`, manually classify. Update `Cargo.toml` workspace `authors`. Final pass.

## What stays "Warp"

- `LICENSE-AGPL`, `LICENSE-MIT` text — required by AGPL fork
- README's "fork of Warp" attribution and the `warpdotdev/warp@d0f045c0` provenance line
- Copyright lines on Warp-authored source files
- Explicit upstream-pointing URLs in CONTRIBUTING/SECURITY where appropriate
- Comments referencing the upstream repo for context

## Open decisions

1. **Bundle ID prefix.** `dev.twarp.Twarp` assumes ownership of `twarp.dev` — alternative: a domain you actually own (e.g., `com.timofeymakhlay.twarp`).
2. **Auto-update.** Disable entirely, or wire to a self-hosted release endpoint?
3. **Internal feature names** (`warp_drive`, `warpify`, `warp_pack`): rename for consistency, or keep as historical brand fragments?
4. **DockTilePlugin**: rename and keep, or remove entirely?
5. **Logo assets.** Need new SVGs/PNGs — derivative ("T" prefix on existing wordmark) or fresh mark?
6. **DB migration filenames** referencing `warp_drive`/`warp_pack`: rename (clean; breaks any existing dev DBs) or keep (uglier; safe).

## Notes

- Cherry-pick cost spikes after 6b — bias toward batching upstream pulls just before 6b lands.
- Trademark "Warp" is registered. README's framing ("a community fork of Warp") is the right form for nominative use; preserve that pattern, don't imply endorsement anywhere.
