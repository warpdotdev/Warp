---
name: change-keybinding
description: Customize Warp keyboard shortcuts (keybindings, keymappings) by editing the user's keybindings.yaml file. Use when the user asks to remap a key combination, rebind an action, change a shortcut, or remove a default keybinding (e.g. "change ctrl+space to ctrl+s", "rebind the command palette to cmd+p", "remove the default for X").
---

# change-keybinding

Use this skill when the user wants to remap, rebind, or remove a Warp keyboard shortcut.

## Keybindings file

User customizations live at:

```
{{keybindings_file_path}}
```

This is the exact path Warp reads at launch — it is platform- and channel-specific (e.g. under `~/.warp*/` on macOS, under XDG config dirs like `~/.config/warp-terminal/` on Linux, and under `%LocalAppData%` on Windows). Use this path verbatim — do not infer a different one from the user's home directory layout. Create the file (and any missing parent directories) if it does not exist.

## File format

A flat YAML map of `action_name` → `key_trigger`. Action names contain a colon, so they **must be quoted**:

```yaml
"workspace:toggle_ai_assistant": ctrl-s
"editor_view:delete_all_left": cmd-shift-A
"editor_view:delete_all_right": cmd-shift-D escape
"workspace:toggle_command_palette": none
```

## Keystroke encoding rules

Triggers use Warp's normalized form — get this exactly right or the binding silently fails to load.

- **Modifiers** (in this order when combined): `ctrl-alt-shift-cmd-meta-`. Cross-platform alias: `cmdorctrl-` (becomes `cmd` on macOS, `ctrl` elsewhere).
- **Letter casing**: applies only to single-letter keys. Without `shift`, the letter is lowercase (`ctrl-s`). With `shift`, the letter is **uppercase** (`shift-A`, never `shift-a`). Mixing them is invalid.
- **Special keys**: `space`, `enter`, `escape`, `tab`, `backspace`, `delete`, `insert`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `f1`–`f20`, `numpadenter`. Always lowercase, even with `shift` (`ctrl-shift-space`, `shift-tab` — never `ctrl-shift-SPACE`). Use the literal word `space` — not `" "`.
- **Punctuation** is the bare character: `cmd-=`, `cmd-,`, `cmdorctrl-/`.
- **Multi-keystroke chord**: separate keystrokes with a single space inside the value, e.g. `cmd-shift-D escape`.
- **Remove a default binding**: set the value to the literal string `none`. The action becomes unbound.

Translate user phrasing into this form: `Ctrl+S` → `ctrl-s`, `Cmd+Shift+P` → `cmd-shift-P`, `Ctrl+Space` → `ctrl-space`.

## Identifying the action

Defaults are compiled into Warp and are **not** discoverable from the keybindings file on disk. Only previously-customized bindings appear there. Pick the right strategy based on how the user described the change:

1. **By current key combo** ("change ctrl+space to ctrl+s"):
   - Read the keybindings file with your filesystem read tool and scan for an entry whose value matches the current trigger (e.g. a line like `"some:action": ctrl-space`). Do not shell out for this — the templated path is user-local and can contain spaces, quotes, or other shell metacharacters depending on the user's platform/config dir, and quoting around it is fragile.
   - If found, that's the action — rewrite its value to the new trigger.
   - If not found, the binding is a default and you cannot introspect it from disk. **Do not invent an action name from generic intuition.** Confirm with the user before writing — via `ask_user_question` (you may propose a candidate as the recommended option only if you have a concrete documented source for it), or by directing them to the **keybindings editor** (action `workspace:show_keybinding_settings`, default `cmd-ctrl-k` on macOS; on other platforms open it from the **Settings → Keyboard Shortcuts** menu), where they can search by description and either edit the binding directly or copy the canonical `namespace:action_name` for you.

2. **By action name** ("set workspace:toggle_command_palette to cmd-p"): the user already gave you the name — write it directly.

3. **By description** ("rebind the command palette to cmd-p"): if you don't already know the exact action name, do not guess. Direct the user to the **keybindings editor** (`workspace:show_keybinding_settings`, default `cmd-ctrl-k` on macOS; **Settings → Keyboard Shortcuts** on other platforms) — they can search by description there and either edit the shortcut in place or share the canonical `namespace:action_name` so you can write it.

## Workflow

1. Determine which action to remap and the new trigger (see "Identifying the action").
2. Read the existing keybindings file at `{{keybindings_file_path}}` if present. **Preserve every existing entry** — only add or update the one you're changing.
3. Write the file (creating parent directories if necessary). Make sure the action key is quoted and the value is normalized (see encoding rules).
4. Tell the user that **Warp must be restarted** for the change to take effect — the keybindings file is loaded only at app launch, unlike `settings.toml` which hot-reloads. They can quit with `cmd-Q` (macOS) or the equivalent on their platform and reopen Warp.

## Examples

Remap an existing custom binding by old trigger:

```yaml
# before
"workspace:toggle_ai_assistant": ctrl-space
# after
"workspace:toggle_ai_assistant": ctrl-s
```

Remove a default shortcut:

```yaml
"workspace:toggle_keybindings_page": none
```

Two-keystroke chord:

```yaml
"editor_view:delete_all_right": cmd-shift-D escape
```

Shift combined with a special key (note the special key stays lowercase):

```yaml
"workspace:toggle_ai_assistant": ctrl-shift-space
```

Cross-platform binding using the `cmdorctrl-` alias (resolves to `cmd` on macOS, `ctrl` elsewhere):

```yaml
"workspace:toggle_command_palette": cmdorctrl-shift-P
```
