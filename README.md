# twarp

**Tidier Warp** — a community fork of [Warp](https://github.com/warpdotdev/warp) for people who want a fast, modern terminal without the AI overlay, plus a few quality-of-life additions for keyboard-driven workflows and git review.

> **Status:** planning / pre-alpha. Forked from `warpdotdev/warp@d0f045c0` on 2026-04-29. None of the differences below are implemented yet — this README is the roadmap.

## Why fork?

Warp is an excellent terminal. twarp aims to be a leaner, AI-free distribution of it with a few focused additions. If you want an agent in your terminal, run one yourself (e.g., `claude`) — but the terminal itself shouldn't be an AI.

## What's different from Warp

### 1. No AI tools

twarp removes Warp's AI features — the agentic mode, cloud-agent surfaces, in-line AI suggestions, AI command help, anything that calls out to an LLM from inside the terminal. The terminal is the terminal.

In practice this means ripping out the agent UI, the cloud-mode codepaths, the AI command palette, and any LLM-backed completion. Telemetry that exists solely to support those features goes with them.

Build progress for all four sections below is tracked in [`roadmap/ROADMAP.md`](roadmap/ROADMAP.md).

### 2. Tab color shortcuts

A keyboard shortcut to assign a color to the active tab — useful for visually distinguishing workflows at a glance ("red tab is prod, green tab is local").

Tentative defaults (configurable):

| Shortcut | Color |
|---|---|
| `⌘⌥1` | Red |
| `⌘⌥2` | Orange |
| `⌘⌥3` | Yellow |
| `⌘⌥4` | Green |
| `⌘⌥5` | Blue |
| `⌘⌥6` | Purple |
| `⌘⌥7` | Pink |
| `⌘⌥8` | Gray |
| `⌘⌥0` | Reset |

Upstream is already exploring per-tab color indication on `oz-agent/APP-4321-active-tab-color-indication` — twarp will likely build on top of that work rather than re-invent it.

### 3. Custom command shortcuts

A declarative way to bind a keyboard shortcut to a sequence of terminal actions: open a new tab, type text, press keys, wait, type more. Lets you turn frequent multi-step workflows into one keystroke.

Two driving examples:

- **`⌘⇧D`** — open a new tab and auto-type `claude` (with enter).
- **`⌘⇧A`** — open a new tab, type `claude` and enter, wait a couple of seconds, then type `/address-code-review-comments ultrathink` and enter.

Sketch of the config format (final shape TBD):

```yaml
shortcuts:
  - keys: cmd+shift+d
    actions:
      - new_tab
      - type: "claude"
      - press: enter

  - keys: cmd+shift+a
    actions:
      - new_tab
      - type: "claude"
      - press: enter
      - wait: 2s
      - type: "/address-code-review-comments ultrathink"
      - press: enter
```

The intent is for the shortcut system to be powerful enough that "open Claude in a fresh tab and feed it a slash command" is one keystroke, and small enough that the config stays readable.

### 4. Open Changes panel (VS Code-style git review)

A built-in side panel for reviewing the current repo's changes — modeled directly on **VS Code's Source Control view**, behavior-for-behavior where it makes sense.

Goals:

- See **working-tree changes and staged changes separately**, with file counts at each level.
- Click a file to view the diff inline.
- Stage / unstage / discard at file or hunk level.
- Show the **git Timeline** (file history) for the focused file.
- Commit message input + commit / push / pull from the same panel.

The aim is parity with VS Code's panel for the operations a terminal user already does dozens of times a day, so you don't need to switch out of the terminal to review changes before committing.

## Tracking upstream Warp

twarp keeps `warpdotdev/warp` as `upstream` and **cherry-picks selectively** rather than bulk-merging. We deliberately don't run `git merge upstream/master`, because that re-fights the AI-deletion every cycle.

Workflow:

1. `git fetch upstream` periodically.
2. `git log upstream/master ^HEAD --oneline` to see what's new.
3. `git cherry-pick <sha>` for individual commits worth taking — perf, rendering, fixes, non-AI features. Skip AI-related commits.
4. Record integrated commits in `UPSTREAM_CHANGELOG.md` so we don't re-pick them.

Baseline (the state we forked from): `warpdotdev/warp@d0f045c0` (2026-04-28).

## Building twarp

Build process is unchanged from upstream Warp. See the original Warp README below for `./script/bootstrap`, `./script/run`, and `./script/presubmit`, and [WARP.md](WARP.md) for the full engineering guide.

## Acknowledgements

twarp is a fork of [Warp](https://github.com/warpdotdev/warp), open-sourced by Warp Inc. on 2026-04-28. All upstream credit goes to the Warp team — twarp's modifications are limited to the four areas above.

## Licensing

twarp inherits Warp's licensing unchanged: the `warpui_core` and `warpui` crates are MIT, the rest of the tree is AGPL v3. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-AGPL](LICENSE-AGPL).

---

> The section below is the **original Warp README, preserved unchanged** as part of the fork. Statements about Warp AI, OpenAI sponsorship, and GPT-powered agents describe upstream Warp, not twarp.

<a href="https://www.warp.dev">
    <img width="1024" alt="Warp Agentic Development Environment product preview" src="https://github.com/user-attachments/assets/9976b2da-2edd-4604-a36c-8fd53719c6d4" />
</a>

<p align="center">
  <a href="https://www.warp.dev">Website</a>
  ·
  <a href="https://www.warp.dev/code">Code</a>
  ·
  <a href="https://www.warp.dev/agents">Agents</a>
  ·
  <a href="https://www.warp.dev/terminal">Terminal</a>
  ·
  <a href="https://www.warp.dev/drive">Drive</a>
  ·
  <a href="https://docs.warp.dev">Docs</a>
  ·
  <a href="https://www.warp.dev/blog/how-warp-works">How Warp Works</a>
</p>

> [!NOTE]
> OpenAI is the founding sponsor of the new, open-source Warp repository, and the new agentic management workflows are powered by GPT models.

<h1></h1>

## About

[Warp](https://www.warp.dev) is an agentic development environment, born out of the terminal. Use Warp's built-in coding agent, or bring your own CLI agent (Claude Code, Codex, Gemini CLI, and others).

## Installation

You can [download Warp](https://www.warp.dev/download) and [read our docs](https://docs.warp.dev/) for platform-specific instructions.

## Licensing

Warp's UI framework (the `warpui_core` and `warpui` crates) are licensed under the [MIT license](LICENSE-MIT).

The rest of the code in this repository is licensed under the [AGPL v3](LICENSE-AGPL).

## Open Source & Contributing

Warp's client codebase is open source and lives in this repository. We welcome community contributions and have designed a lightweight workflow to help new contributors get started. For the full contribution flow, read our [CONTRIBUTING.md](CONTRIBUTING.md) guide.

### Issue to PR

Before filing, [search existing issues](https://github.com/warpdotdev/warp/issues?q=is%3Aissue+is%3Aopen+sort%3Areactions-%2B1-desc) for your bug or feature request. If nothing exists, [file an issue](https://github.com/warpdotdev/warp/issues/new/choose) using our templates. Security vulnerabilities should be reported privately as described in [CONTRIBUTING.md](CONTRIBUTING.md#reporting-security-issues).

Once filed, a Warp maintainer reviews the issue and may apply a readiness label: [`ready-to-spec`](https://github.com/warpdotdev/warp/issues?q=is%3Aissue+is%3Aopen+label%3Aready-to-spec) signals the design is open for contributors to spec out, and [`ready-to-implement`](https://github.com/warpdotdev/warp/issues?q=is%3Aissue+is%3Aopen+label%3Aready-to-implement) signals the design is settled and code PRs are welcome. Anyone can pick up a labeled issue — mention **@oss-maintainers** on an issue if you'd like it considered for a readiness label.

### Building the Repo Locally

To build and run Warp from source:

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run Warp
./script/presubmit   # fmt, clippy, and tests
```

See [WARP.md](WARP.md) for the full engineering guide, including coding style, testing, and platform-specific notes.

## Joining the Team

Interested in joining the team? See our [open roles](https://www.warp.dev/careers).

## Support and Questions

1. See our [docs](https://docs.warp.dev/) for a comprehensive guide to Warp's features.
2. Join our [Slack Community](https://go.warp.dev/join-preview) to connect with other users and get help from the Warp team.
3. Try our [Preview build](https://www.warp.dev/download-preview) to test the latest experimental features.
4. Mention **@oss-maintainers** on any issue to escalate to the team — for example, if you encounter problems with the automated agents.

## Code of Conduct

We ask everyone to be respectful and empathetic. Warp follows the [Code of Conduct](CODE_OF_CONDUCT.md). To report violations, email warp-coc at warp.dev.

## Open Source Dependencies

We'd like to call out a few of the [open source dependencies](https://docs.warp.dev/help/licenses) that have helped Warp to get off the ground:

* [Tokio](https://github.com/tokio-rs/tokio)
* [NuShell](https://github.com/nushell/nushell)
* [Fig Completion Specs](https://github.com/withfig/autocomplete)
* [Warp Server Framework](https://github.com/seanmonstar/warp)
* [Alacritty](https://github.com/alacritty/alacritty)
* [Hyper HTTP library](https://github.com/hyperium/hyper)
* [FontKit](https://github.com/servo/font-kit)
* [Core-foundation](https://github.com/servo/core-foundation-rs)
* [Smol](https://github.com/smol-rs/smol)
