# Dark/Light Theme Change Notifications — Product Spec

GitHub issue: https://github.com/warpdotdev/warp-external/issues/9425

## Summary

Implement the Contour terminal extension for dark/light theme-change notifications.  Applications running in Warp can subscribe to receive a notification whenever the user switches between dark mode and light mode (i.e., changes the active Warp theme).  The feature is gated behind an opt-in escape sequence, so programs that do not opt in are completely unaffected.

## Problem

Many terminal applications (editors like Neovim and Helix, shells that show coloured prompts, TUI tools, etc.) want to automatically adapt their own colour scheme when the terminal's theme changes.  Because Warp never emits a colour-scheme-change signal today, those apps must rely on system-level mechanisms (if any) and cannot follow Warp theme changes.

## Goals

- Allow programs running in Warp to opt in to dark/light theme change notifications via `CSI ? 2031 h`.
- Allow programs to opt back out via `CSI ? 2031 l`.
- Allow programs to query the current colour scheme at any time via `CSI ? 996 n`, receiving an immediate `CSI ? 997 ; Ps n` response.
- Notify all opted-in programs whenever the active Warp theme changes between dark and light.

## Non-goals

- Emitting unsolicited notifications for programs that have not sent `CSI ? 2031 h`.
- Changing the palette query/update protocol (`CSI ? 2 $ p`, OSC 10/11, etc.).
- Supporting this feature in embedded/hosted sessions over SSH; this tracks mode in the local terminal grid only.
- Persisting the opt-in across process restarts or across alt-screen swaps.

## Behaviour

### Enabling notifications

`CSI ? 2031 h` — Enable dark/light notifications.  After receiving this sequence, the terminal will push a `CSI ? 997 ; Ps n` notification to the PTY whenever the Warp theme switches between dark and light.

### Disabling notifications

`CSI ? 2031 l` — Disable dark/light notifications.  The mode is disabled by default; sending this sequence without having enabled it is silently ignored.

### Querying the current colour scheme

`CSI ? 996 n` — Request the current colour preference.  The terminal responds immediately with `CSI ? 997 ; Ps n` regardless of whether notifications are enabled.

### Response format

`CSI ? 997 ; Ps n`
- `Ps = 1` — the current theme has a dark background (dark mode).
- `Ps = 2` — the current theme has a light background (light mode).

### Dark vs. light classification

A Warp theme is classified as **dark** (`Ps = 1`) when its foreground colour is light (i.e., text appears on a dark background). It is classified as **light** (`Ps = 2`) when its foreground colour is dark (text on a light background).  This matches the behaviour of other terminal emulators implementing the Contour extension.

### Interaction with the alt screen

Mode `?2031` is tracked per terminal grid (main screen and alt screen independently).  An application that sets the mode on the alt screen only receives notifications while that alt screen is active.

### When no theme change occurs

If the user applies a theme that has the same dark/light classification as the previous theme, no notification is sent.

## Success Criteria

1. A program that sends `CSI ? 2031 h` receives `CSI ? 997 ; 1 n` or `CSI ? 997 ; 2 n` each time the user changes the active Warp theme from dark to light or vice versa.
2. A program that sends `CSI ? 996 n` immediately receives the correct `CSI ? 997 ; Ps n` response reflecting the currently active Warp theme, whether or not notifications are enabled.
3. A program that has not sent `CSI ? 2031 h` receives no notifications when the theme changes.
4. A program that sends `CSI ? 2031 l` stops receiving notifications.
