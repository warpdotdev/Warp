# PRODUCT.md — Support Kiro CLI Agent Integration in Warp

Issue: https://github.com/warpdotdev/warp/issues/9066

## Summary

Add Kiro CLI (`kiro`) as a first-class supported CLI agent in Warp, alongside
Claude Code, Codex, Gemini CLI, and the other agents already integrated. When a
user runs `kiro` in a Warp terminal, Warp detects the session, displays the
Kiro-branded agent footer, enables the rich input composer (Ctrl-G), and tracks
session status (in-progress, blocked, success) using the same plugin event
protocol used by other agents.

Figma: none provided.

## Goals / Non-goals

In-scope:

- Detecting `kiro` as a CLI agent session in the terminal.
- Displaying the Kiro-branded footer and toolbar chip while a `kiro` session is
  active.
- Enabling the rich input composer (Ctrl-G / footer button) for composing
  prompts to send to Kiro.
- Showing plugin install/update instructions when the Warp plugin is not
  installed or is out of date.
- Tracking session status (InProgress, Blocked, Success) via the Warp plugin
  event protocol.
- Surfacing Kiro in the "Third party CLI agents" settings page.
- Telemetry for Kiro sessions consistent with other agents.
- macOS support (primary). Linux support follows the same code path and is
  included. Windows support is not in scope for this spec.

Out of scope:

- Changes to the Warp built-in agent, execution profiles, or onboarding.
- A new `SkillProvider::Kiro` variant — Kiro CLI supports the existing
  `SkillProvider::Agents` skill format; no Kiro-specific skill provider is
  needed at this time.
- Custom toolbar commands or user-configured regex patterns for Kiro.
- Remote (SSH) session support beyond what the existing plugin infrastructure
  already provides generically.

## Behavior

1. When a user runs `kiro` (or a command whose first token is `kiro`) in a Warp
   terminal pane, Warp detects an active Kiro CLI session and begins tracking it
   in `CLIAgentSessionsModel`.

2. While a Kiro session is active, the Kiro-branded agent footer is displayed at
   the bottom of the terminal pane. The footer shows the Kiro logo, the session
   status, and the rich input button. Layout, interaction model, and dismiss
   behavior are identical to the Claude Code and Gemini footers.

3. Pressing Ctrl-G (or clicking the rich input button in the footer) opens the
   rich input composer. The composer accepts free-form text and slash-command
   skills from `SkillProvider::Agents`. The skill command prefix is `/`.

4. Submitting a prompt from the rich input composer sends it to the running
   `kiro` process via the terminal PTY, exactly as it does for other agents.

5. When the Warp plugin for Kiro is installed and active, session status updates
   (InProgress, Blocked, Success) are reflected in the footer in real time.
   When the plugin is not installed, the footer still appears (via command
   detection) but status tracking is unavailable.

6. When the Warp plugin is not installed, Warp displays plugin install
   instructions in the footer's plugin pane. The instructions guide the user
   through installing the plugin so that status tracking becomes available.

7. When the installed plugin version is below the minimum required version, Warp
   displays plugin update instructions in the same pane.

8. The Kiro agent entry appears in the "Third party CLI agents" settings page
   under Settings → Agents → Third party CLI agents, alongside Claude Code,
   Codex, Gemini, and the other supported agents. The entry shows the Kiro logo,
   name, and a link to the Kiro CLI documentation.

9. Session start, status changes, and session end are reported to telemetry
   using the same `CLIAgentType` telemetry event structure used by other agents,
   with `CLIAgentType::Kiro` as the agent discriminant.

10. The Kiro footer and rich input are not shown when the terminal pane is in a
    remote SSH session where the Warp plugin cannot be confirmed as installed,
    consistent with the behavior of other agents in that context.

11. Kiro sessions are included in shared session state sync (the `cli_agent`
    field in the shared session protocol) using the serialized name `"Kiro"`.

12. The `kiro` command is not treated as a custom/unknown agent — it maps
    directly to `CLIAgent::Kiro` and receives the full first-class treatment
    (branded footer, logo, install instructions) rather than the generic
    "CLI Agent" fallback.

13. No existing agent's behavior, settings, or telemetry is changed by this
    addition.
