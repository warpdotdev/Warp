# Dark/Light Theme Change Notifications — Tech Spec

GitHub issue: https://github.com/warpdotdev/warp-external/issues/9425

## Overview

Implement the Contour dark/light theme-change notification extension.  The feature
consists of three parts:

1. **Mode tracking** — the `?2031` private DEC mode tracks whether a terminal
   session has opted in to unsolicited notifications.
2. **Synchronous query** — `CSI ? 996 n` produces an immediate `CSI ? 997 ; Ps n`
   response reflecting the current Warp theme.
3. **Unsolicited notifications** — whenever the user changes the Warp theme and the
   classification (dark vs. light) changes, Warp writes `CSI ? 997 ; Ps n` to every
   session that has set mode `?2031`.

## Files changed

| File | Change |
|------|--------|
| `crates/warp_terminal/src/model/mode.rs` | New `TermMode::DARK_LIGHT_NOTIFICATIONS` bitflag (bit 23) |
| `crates/warp_terminal/src/model/ansi/control_sequence_parameters.rs` | New `Mode::DarkLightNotifications` variant mapped to private mode `2031` |
| `app/src/terminal/model/ansi/handler.rs` | New default-no-op `report_color_scheme<W>` method on `Handler` trait |
| `app/src/terminal/model/ansi/mod.rs` | New `('n', Some(b'?'))` match arm dispatching `996` → `report_color_scheme` |
| `app/src/terminal/model/grid/ansi_handler.rs` | `set_mode` / `unset_mode` arms for `DarkLightNotifications` |
| `app/src/terminal/model/terminal_model.rs` | `is_dark_mode: bool` field; `set_color_scheme`; `report_color_scheme` override |
| `app/src/terminal/view.rs` | `handle_theme_change` updates color scheme, sends notification if mode is set |

## Design decisions

### Mode storage

`DarkLightNotifications` is stored as a bitflag in `TermMode`, which lives in the
`State` struct inside `GridHandler`.  This mirrors the approach used by every other
DEC private mode (`BRACKETED_PASTE`, `FOCUS_IN_OUT`, etc.) and means the mode is
independent per terminal grid (main screen vs. alt screen).

### Color scheme storage

`is_dark_mode: bool` is stored on `TerminalModel` (not deep in the grid state).
This mirrors the existing `colors: color::List` field, which is also held at the
model level and updated from the view on every theme change.  The model-level field
is accessed by the `report_color_scheme` override without delegation, matching how
non-grid state is handled elsewhere.

The field is initialised to `true` (dark mode) on construction; the view calls
`set_color_scheme` on the very first `AppearanceEvent::ThemeChanged` subscription
fire (or on construction if we add an eager call there), so the initialisation value
is only visible if a query arrives before the first theme event.

### Query response (`CSI ? 996 n`)

The `Handler` trait gains a new `report_color_scheme<W: io::Write>` method with a
default no-op body.  The `TerminalModel` overrides this method without delegating to
the grid — it uses `self.is_dark_mode` to write the response directly.

The performer (`mod.rs`) dispatches `('n', Some(b'?'))` with parameter `996` to
`handler.report_color_scheme(writer)`.  All other private DSR parameters fall
through to the `unhandled!()` macro.

### Notification dispatch

Inside `TerminalView::handle_theme_change`:

1. Compute `is_dark` from `appearance.theme().inferred_color_scheme()`.
2. Lock the model, call `update_colors` and `set_color_scheme`, then check
   `is_term_mode_set(TermMode::DARK_LIGHT_NOTIFICATIONS)` — the result is stored in
   a local `bool`.
3. Release the lock.
4. If notifications are enabled, emit `Event::WriteBytesToPty` with the appropriate
   `CSI ? 997 ; Ps n` sequence via `self.write_to_pty(...)`.

The lock is released before calling `write_to_pty` to avoid a potential deadlock
(other `write_to_pty` callsites also do not hold the model lock).

### Dark/light classification

`ColorScheme::LightOnDark` (light foreground on dark background) maps to dark mode
(`Ps = 1`).  `ColorScheme::DarkOnLight` maps to light mode (`Ps = 2`).  This
matches the `inferred_color_scheme()` logic in `WarpTheme`.

Note: the comments inside the `ColorScheme` enum definition are misleading (the
labels appear swapped relative to common "dark mode / light mode" terminology).  The
implementation here is based on the actual `inferred_color_scheme()` logic, not the
comments.

## Testing

Manual verification steps:
1. Open Warp, run a program that sends `CSI ? 2031 h` and listens for `CSI ? 997`
   responses (e.g., a small shell script or Neovim with appropriate config).
2. Switch Warp theme between a dark theme and a light theme.  Confirm notifications
   are received with `Ps = 1` and `Ps = 2` respectively.
3. Run `printf '\033[?996n'` in the shell; confirm the response reflects the current
   theme.
4. Disable notifications with `CSI ? 2031 l` and switch themes; confirm no further
   notifications arrive.
