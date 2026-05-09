# Simplification Proposal: opkald-warp

## TL;DR
- **Archive the fork; use upstream Warp.dev directly.** There are zero OpkaldAI commits or "opkald" references in 5013 files — Effort S · Impact L
- **If launch-config / theme tweaks ever land, put them in a tiny separate repo** (`opkald-warp-config/`) shipping YAML + scripts, not a Rust fork — Effort S · Impact M
- **Document the Symphony ↔ Warp contract** (`warp://launch/<name>` URI + `~/.warp/launch_configurations/*.yaml`) as the only integration surface — Effort S · Impact M

## Current state

### Investigation method
- `git remote -v` → fork of `github.com/Opkald-AI/warp` from `warpdotdev/Warp`
- `git log --all --pretty="%an %ae %s"` → 100% Warp.dev authors. No `cboeg`, `opkaldai`, `opkald`, `boegelund`, or any OpkaldAI committer anywhere in history
- `git log master ^upstream/master` → all 143 "ahead" commits are Warp.dev authors (Safia Abdalla, Kevin Chevalier, Jason Keung, etc.) — these are merge artifacts, not OpkaldAI work
- `git rev-list --count upstream/master ^master` → fork is **1 behind** upstream; upstream has many newer commits the fork hasn't pulled
- `Grep -i "opkald"` across the entire 5013-file checkout → **no matches in any file**, path or content
- `README.md` is verbatim upstream Warp branding — no OpkaldAI mentions
- Branch list: `master`, `chore/simplification-analysis`, `feature/launch-uri-active-window`. The feature branch is `0` commits ahead of master — empty placeholder

### Confirmed: this is an unmodified fork
- Last commit: `2026-05-01` by Warp.dev contributor "B" (`Remove stray backticks from Windows installer README code blocks (#9691)`)
- Upstream `master` HEAD: `2026-05-09` — fork is ~8 days stale
- **No OpkaldAI customizations exist.** Period.

### Repo size and shape
- 5013 files, full Rust workspace (`Cargo.toml`, `crates/`, `app/`, AGPL + MIT dual license)
- Branch model: `master` tracks upstream `warpdotdev/Warp:master`. No long-lived OpkaldAI feature branches with content
- Last upstream sync: roughly the 2026-05-01 cutoff; ~143 upstream commits have landed since but were never pulled

### What does OpkaldAI actually use Warp for?
Per `opkald-symphony` simplification analysis and the `frontend-design` skill:
1. Symphony spawns Claude Code sessions and writes a `~/.warp/launch_configurations/<id>.yaml` describing the working directory, env, and command
2. Symphony then opens `warp://launch/<id>` so a real desktop Warp window attaches to that session
3. Both halves of that contract — the URI scheme and the YAML schema — are **public, documented Warp APIs**. No source-code coupling exists. Symphony does not import, link against, or build anything from this repo
4. Warp is also one of several supported terminals in `frontend-design`-style tooling; OpkaldAI does not depend on a custom build of it

**Conclusion:** the fork exists, is bit-rotting against upstream, and contributes nothing. Every OpkaldAI integration with Warp goes through public APIs that work with the off-the-shelf `warp.dev` download.

## Concrete proposals

### 1. Archive the repo; switch all references to upstream Warp
- **What:** Mark `Opkald-AI/warp` as archived on GitHub. Remove any internal docs/scripts that imply we build or maintain a Warp fork. Anywhere a developer needs Warp, they install it from `warp.dev/download` like any other user
- **Why:** A 5013-file Rust fork carries real costs even when untouched: clones eat dev disk, cloning into worktrees is slow, security scanners flag it, contributors assume customizations exist and waste time looking, and "is this fork doing something?" is a recurring question. Removing it deletes the question
- **Migration:**
  1. Search internal docs/skills/READMEs for `Opkald-AI/warp` references; replace with `warp.dev/download`
  2. Confirm no CI job, deploy script, or bootstrap script clones this repo
  3. Confirm Symphony's launch-config code path does **not** reference a path inside this checkout (it shouldn't — it writes to `~/.warp/launch_configurations/`)
  4. Archive the GitHub repo (preserves history, prevents accidental commits)
  5. Optionally: `git worktree remove` the local worktrees and delete the local checkout
- **Risk:** Near zero. Reversible by un-archiving. The only risk is discovering an undocumented script that clones it; the search in step 1 covers that
- **Effort:** S
- **Impact:** L (removes 5013 files, ~hundreds of MB of bit-rot, and a perpetual upstream-sync question that no one is answering)

### 2. If patches are ever needed, ship them as `opkald-warp-config/`, not a fork
- **What:** Pre-emptive guidance: when someone later proposes "let's tweak Warp for OpkaldAI" — themes, default keybindings, a starter set of launch configurations, a `warp_setup.sh` for new developers — that work belongs in a small new repo, not a fork of a 5013-file Rust application
- **Why:** Everything OpkaldAI plausibly wants from Warp is configurable from outside the binary:
  - Launch configs → `~/.warp/launch_configurations/*.yaml`
  - Themes → `~/.warp/themes/*.yaml`
  - Workflows → `~/.warp/workflows/*.yaml`
  - Keybindings, settings → Warp's settings UI / settings JSON

  A config repo of YAML + a one-shot install script is on the order of dozens of files and zero Rust. Maintaining it is trivial; maintaining a fork is not
- **Migration (if/when needed):**
  1. New repo `opkald-warp-config/` with `themes/`, `launch_configurations/`, `workflows/`, `install.sh`/`install.ps1`
  2. `install.ps1` symlinks/copies into `~/.warp/`
  3. Document in onboarding: "install Warp, run `opkald-warp-config/install.ps1`"
- **Risk:** None — this is greenfield, and only triggered if customization is actually requested
- **Effort:** S (when/if needed)
- **Impact:** M (avoids re-creating today's problem)

### 3. Document the Symphony ↔ Warp integration contract
- **What:** A short note (in `opkald-symphony`'s docs, not here) stating: "Symphony integrates with Warp through two public Warp APIs: the `warp://launch/<id>` URI scheme and `~/.warp/launch_configurations/<id>.yaml`. We do not fork, patch, or build Warp. Use the standard `warp.dev` install."
- **Why:** Right now the existence of `Opkald-AI/warp` actively misleads — a new developer seeing the fork reasonably assumes Symphony depends on a custom Warp build. Documenting the actual, narrow contract removes that confusion permanently
- **Migration:** One paragraph in `opkald-symphony/README.md` or its CLAUDE.md, linked from any place that mentions Warp
- **Risk:** None
- **Effort:** S
- **Impact:** M

## Cross-repo concerns

- **Coupling to opkald-symphony:** Symphony writes `~/.warp/launch_configurations/<id>.yaml` and shells out to `warp://launch/<id>`. This works against any installed Warp, no fork required. The `opkald-symphony` simplification analysis already noted this — it's confirmed here from the Warp side: nothing in this repo is referenced, imported, or specialized for Symphony
- **Maintenance burden of a 5013-file Rust fork:** Currently **nothing is being maintained** — the fork is 8 days behind upstream and accumulating. There is no person, schedule, or process doing upstream syncs. That's the right call given there's nothing to preserve, but it means the fork's only real "feature" is staleness. Archiving formalizes what's already true
- **Upstream-sync story:** Today: ad-hoc, never. Recommended: not applicable after archiving. If a fork is ever resurrected for a real reason (proposal #2 says: don't), the sync workflow would be `git fetch upstream && git rebase upstream/master` on a small patch series — but only if that patch series exists, which it currently does not

## Out of scope / explicit non-goals

- **Refactoring Warp itself.** Warp is a third-party product; OpkaldAI does not own it, ship it, or have any reason to modify its 5013 source files
- **Migrating to a different terminal** (kitty, alacritty, iTerm, Windows Terminal, etc.). Warp's `warp://launch/` URI scheme and launch-config YAML are exactly what Symphony needs; no other terminal offers an equivalent right now, and the `frontend-design` skill already accepts multiple terminals where it matters. This is a "leave Warp alone" proposal, not a "leave Warp" proposal
- **Resurrecting `feature/launch-uri-active-window`.** That branch is empty (0 commits ahead of master). If the work behind its name was ever needed, it would be a tiny patch — and per proposal #2, would belong in `opkald-warp-config/` or upstreamed to Warp.dev as a regular PR, not carried on a private fork
