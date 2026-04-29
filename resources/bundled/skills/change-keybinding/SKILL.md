---
name: change-keybinding
description: Customize Warp keyboard shortcuts (keybindings, keymappings) by editing the user's keybindings.yaml file. Use when the user asks to remap a key combination, rebind an action, change a shortcut, or remove a default keybinding (e.g. "change ctrl+space to ctrl+s", "rebind the command palette to cmd+p", "remove the default for X").
---

# change-keybinding

Use this skill when the user wants to remap, rebind, or remove a Warp keyboard shortcut.

## Keybindings file

User customizations live in a YAML file at:

```
~/.warp/keybindings.yaml
```

Non-stable Warp builds use a channel-specific variant (`~/.warp-dev/`, `~/.warp-preview/`, `~/.warp-oss/`, etc.). To find the right one, list available Warp data directories:

```sh
ls -d ~/.warp*/
```

If multiple match and the active channel is unclear, ask the user. Create the file if it does not exist.

## File format

A flat YAML map of `action_name` → `key_trigger`. Action names contain a colon, so they **must be quoted**:

```yaml
"workspace:toggle_ai_assistant": ctrl-s
"editor:delete_all_left": cmd-shift-A
"editor:delete_all_right": cmd-shift-D escape
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

Defaults are compiled into Warp and are **not** discoverable from any file on disk. Only previously-customized bindings appear in `keybindings.yaml`. Pick the right strategy based on how the user described the change:

1. **By current key combo** ("change ctrl+space to ctrl+s"):
   - Read `keybindings.yaml` and look for an entry whose value contains the current trigger (e.g. `grep ctrl-space ~/.warp*/keybindings.yaml`).
   - If found, that's the action — rewrite its value to the new trigger.
   - If not found, the binding is a default and you cannot introspect it from disk. **Do not invent an action name from generic intuition.** Confirm with the user before writing — either via `ask_user_question` (you may propose a candidate as the recommended option only if you have a concrete documented source for it, e.g. an explicit example in this skill or in Warp's published docs), or by directing them to Settings → Keybindings (default `cmd-k`, action `workspace:toggle_keybindings_page`) to copy the canonical `namespace:action_name`.

2. **By action name** ("set workspace:toggle_command_palette to cmd-p"): the user already gave you the name — write it directly.

3. **By description** ("rebind the command palette to cmd-p"): if you don't already know the exact action name, do not guess. Direct the user to Settings → Keybindings (`cmd-k`), where they can search by description and copy the canonical `namespace:action_name`.

## Workflow

1. Determine which action to remap and the new trigger (see "Identifying the action").
2. Find the right `~/.warp*/` directory with `ls -d ~/.warp*/`. Ask the user if ambiguous.
3. Read the existing `keybindings.yaml` if present. **Preserve every existing entry** — only add or update the one you're changing.
4. Write the file. Make sure the action key is quoted and the value is normalized (see encoding rules).
5. Tell the user that **Warp must be restarted** for the change to take effect — `keybindings.yaml` is loaded only at app launch, unlike `settings.toml` which hot-reloads. They can quit with `cmd-Q` and reopen Warp.

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
"editor:delete_all_right": cmd-shift-D escape
```

Shift combined with a special key (note the special key stays lowercase):

```yaml
"workspace:toggle_ai_assistant": ctrl-shift-space
```

Cross-platform binding using the `cmdorctrl-` alias (resolves to `cmd` on macOS, `ctrl` elsewhere):

```yaml
"workspace:toggle_command_palette": cmdorctrl-shift-P
```
