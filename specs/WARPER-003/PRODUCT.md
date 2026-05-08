# WARPER-003: clean up help menus and About branding

## Summary

Warper must not expose Warp-hosted feedback, Slack, What's New, or changelog entrypoints in the top-right gearbox menu or top-bar Help menu. The only retained project-support link in those menus is GitHub Issues, and it must point to the Warper repository issue tracker. Settings -> About must identify the app as Warper and use copyleft copy.

## Problem

The app still exposes hosted or upstream-branded support/community surfaces in menus and settings, including Feedback, Slack, changelog UI, and Warp-branded About copy. These surfaces conflict with the local-only product shape defined by WARPER-001 and make the fork look partially branded as upstream Warp.

## Goals / Non-goals

- Goal: remove Slack and feedback entrypoints from the top-right gearbox menu and top-bar Help menu.
- Goal: make any retained GitHub issue-reporting link point to the Warper repository issues page.
- Goal: make Settings -> About identify Warper, not Warp, and use copyleft wording.
- Goal: remove What's New and changelog UI from app menus, top-right menus, settings, resource-center surfaces, slash commands, and command palette entrypoints.
- Goal: keep local logs and local diagnostics available to the user.
- Non-goal: remove Warp-hosted documentation links before Warper-owned documentation exists.
- Non-goal: add a replacement hosted support system.
- Non-goal: remove local settings, local logs, keyboard shortcuts, or documentation.
- Non-goal: remove a Warper-owned GitHub Issues link.

## Behavior

1. The top-right gearbox menu may contain settings, keyboard shortcuts, documentation, and local diagnostics if available. It does not show `Feedback`, `Slack`, `Warp Slack Community`, `What's New`, hosted changelog entries, or equivalent hosted/community entries.
2. Documentation links may continue to open existing Warp documentation until Warper-owned documentation exists. Documentation links must not be labeled as feedback, Slack, What's New, changelog, or Warper-owned support.
3. The top-bar Help menu does not show `Send Feedback`, `Warp Slack Community`, `Slack`, `What's New`, hosted changelog entries, or equivalent hosted/community entries.
4. The top-bar Help menu may show `GitHub Issues` only when the action opens the Warper repository issues page. It must not open Warp issues, Warp feedback forms, Warp support, or a Slack invite.
5. Any GitHub issue-reporting link elsewhere in these menu surfaces uses Warper repository issue tracking and is labeled as a Warper/project issue link, not as Warp feedback.
6. Documentation/help menu items may point to Warp documentation. They must not point to Warp changelog, Warp support, Warp feedback forms, or Warp Slack.
7. Settings -> Features does not show `Show changelog toast after updates` or any other setting whose only purpose is a hosted or upstream Warp changelog.
8. Warper does not show a What's New section, changelog toast, release-notes modal, resource-center changelog item, or slash command that opens a changelog.
9. Search, command palette, keybindings, slash commands, app menus, and resource-center surfaces cannot resurrect removed feedback, Slack, What's New, or changelog entrypoints.
10. Existing persisted settings related to changelog display are ignored or removed locally. They do not produce a visible setting, toast, modal, or background network request.
11. Local logs remain reachable through a clearly local action such as `View Warper logs`.
12. Settings -> About is about Warper. It uses Warper name and Warper visual branding, not upstream Warp product branding.
13. Settings -> About uses copyleft wording instead of upstream Warp copyright wording.
14. Error states that previously offered hosted feedback show a local-only alternative, a Warper GitHub Issues link, or no link. They do not invite the user to contact Warp or join Warp Slack.
15. When outbound networking is blocked, opening menus and settings does not produce failed requests to feedback, Slack, changelog, release-note, resource-center, or hosted support endpoints.
16. User-visible labels in this area use `Warper` when they identify the forked app. `Warp` may appear only when referring to upstream compatibility concepts where renaming would be misleading.
