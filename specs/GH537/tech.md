# TECH.md — Honor user-defined shell bindkeys in Warp's input editor

Issue: https://github.com/warpdotdev/warp/issues/537
Product spec: [`product.md`](./product.md)

## Context

Warp's input editor receives raw keystrokes, matches them against the
`Keymap` table, and dispatches `InputAction` variants. Today that table
knows nothing about the user's shell bindings, so user customizations
(`bindkey '^X^E' edit-command-line`, readline `bind`, fish `bind`) are
ignored. See `product.md` for the user-visible behavior we want.

Relevant code, with line ranges:

- **Input editor and actions** — `app/src/terminal/input.rs (1072-1149)`.
  `InputAction` is the dispatched action type. Today it covers
  Warp-flavored actions (`FocusInputBox`, `CtrlR`, `CtrlD`,
  `MaybeOpenCompletionSuggestions`, etc.) but does **not** have the
  fine-grained editor verbs ZLE / readline expose
  (`backward-kill-word`, `transpose-chars`, `kill-line`, `yank-pop`,
  `up-history`, `vi-cmd-mode`, …). The buffer model lives in
  `InputBufferModel` in the same file. `crates/editor/src/editor.rs
  (18-55)` exposes the underlying `EditorView` trait.
- **Keymap** — `crates/warpui_core/src/keymap.rs (25-38, 44-49, 72-150)`.
  `Keymap { fixed_bindings, editable_bindings }` indexed by name, with
  `Trigger::{Keystrokes(Vec<Keystroke>), Standard(StandardAction),
  Custom(CustomTag)}` and `ContextPredicate` for context-scoped layering.
  Resolution: `editable_bindings` (user-overridden) wins over
  `fixed_bindings` (Warp defaults). Matching lives in
  `crates/warpui_core/src/keymap/matcher.rs`.
- **Shell type and session** — `crates/warp_terminal/src/shell/mod.rs
  (58-96, 250-255)`. `ShellType { Zsh, Bash, Fish, PowerShell }`,
  `Shell { type, version, options, plugins, shell_path }`,
  `ShellStarter::init()` at line 79. `app/src/terminal/local_tty/shell.rs
  (1-200)` for spawn details.
- **Bootstrap and DCS hooks** — `app/src/terminal/bootstrap.rs (1-150)`
  injects a per-shell init script from `bundled/bootstrap/{zsh,bash,fish,
  pwsh}.sh`. Script-to-app communication uses
  `app/src/terminal/model/ansi/dcs_hooks.rs (1-150)`: `DProtoHook`
  variants (`Bootstrapped`, `Precmd`, `Preexec`, `InputBuffer`,
  `InitShell`, …) carry hex-encoded JSON payloads
  (`HEX_ENCODED_JSON_MARKER = 'd'`). DCS dispatch arrives as
  `ModelEvent::PluggableNotification` in
  `app/src/terminal/model_events.rs (468-472)`. **There is no live
  "invisible command exec" primitive today**; bootstrap-emitted DCS
  payloads are the right plumbing to extend.
- **Settings** — `app/src/terminal/keys_settings.rs (15-71, 26-34)`.
  `define_settings_group!` macro is the pattern for new boolean toggles
  (see `quake_mode_enabled`). Feature flags live in
  `crates/warp_features/src/lib.rs (9+)`.
- **Telemetry** — `app/src/server/telemetry/events.rs (1237+, 2920)`.
  `TelemetryEvent` enum + `send_telemetry_from_ctx!` macro.
- **Vi-mode tracking** — none today. The `vim` crate is Warp's own
  in-editor vi emulation, not shell awareness.

## Proposed changes

The implementation has five logical pieces. Each maps cleanly to one
subsystem above.

### 1. Bootstrap-side binding query

Extend the bootstrap scripts to dump the user's binding table to Warp
via a new DCS hook variant. Doing the query in bootstrap (rather than
adding a runtime invisible-exec primitive) avoids polluting history,
scrollback, and last-status; it also runs before the first prompt so
bindings are available when the user starts typing.

- `bundled/bootstrap/zsh.sh`: for each keymap (`main emacs viins vicmd
  vivis viopp command isearch menuselect`), run `bindkey -L -M
  $keymap` and emit a JSON object `{ keymap: [ { keys, widget }, … ] }`.
  Also emit `KEYMAP` so the active keymap is known.
- `bundled/bootstrap/bash.sh`: `bind -p` for the current keymap and
  `bind -p -m emacs / vi-insert / vi-command` for the others. Detect vi
  vs emacs via `set -o | grep -E '^(vi|emacs)'`.
- `bundled/bootstrap/fish.sh`: this requires reworking the existing
  bootstrap, which currently sets
  `fish_key_bindings = fish_default_key_bindings` (line 306) and then
  installs four Warp-required binds (`\cP`, `\ep`, `\ew`, `\ei`) on
  top — clobbering any user `fish_vi_key_bindings` setting and any
  user-installed binds. To honor user fish bindings without losing
  Warp's required reporting binds, we change the bootstrap to:

  1. Capture the user's `fish_key_bindings` value at the very top of
     the bootstrap, and stop the unconditional reset at line 306. The
     user's chosen scheme runs as configured.
  2. After the user's scheme runs, install Warp's four reserved binds
     (`\cP`, `\ep`, `\ew`, `\ei`) explicitly in every bind mode the
     user uses (default, insert, visual for vi mode; default for
     emacs; plus any custom modes discovered via `bind -L`). Those
     four keys are reserved for Warp and intentionally shadow user
     bindings on them — the explicit precedence boundary from
     PRODUCT #14.
  3. Snapshot the resulting `bind` output per mode and emit it as the
     `ShellBindings` payload. The vi-mode-vs-input-reporting conflict
     that originally motivated the reset is resolved here because the
     reporting bind is reinstalled in whichever mode is active, instead
     of the scheme being reset wholesale.

  Mode tracking uses `$fish_bind_mode` for the initial snapshot and
  the in-app vi state machine described in the open-questions section
  for transitions.

The payload is emitted as a new `DProtoHook::ShellBindings` variant in
`dcs_hooks.rs` carrying `{ shell, keymaps: Vec<KeymapTable>,
active_keymap, schema_version, nonce }`. Reuse `HEX_ENCODED_JSON_MARKER`.

The `ShellBindings` payload is a privileged terminal-control message
(it can rewrite local key handling) and is only accepted from the
bootstrap context:

- Each Warp-spawned shell receives a per-session, per-tab nonce in its
  environment (`WARP_BOOTSTRAP_NONCE`, generated when the tab spawns
  and not exported beyond it). The bootstrap embeds this nonce in
  every `ShellBindings` and `Precmd` payload it emits. The app-side
  handler rejects any payload whose nonce does not match the expected
  value for that tab — `cat`'d files, curl responses, and other
  process output that happens to contain a DCS sequence cannot spoof
  bindings because they cannot read the nonce.
- Payloads exceeding a fixed total size cap (256 KiB across all
  keymaps combined) are rejected and logged. Individual binding
  entries exceeding a per-key cap (4 KiB) are dropped from the
  payload before parsing.
- Schema validation is strict: any field type mismatch, unknown
  `schema_version`, or malformed Keystroke string causes the entire
  payload to be discarded — partial application is never attempted.
- The same nonce check applies to the binding-hash field on the
  existing `Precmd` hook; an unsigned or mismatched hash leaves the
  previous binding table in place.

### Re-query mechanism

Re-queries are driven entirely shell-side; the app never has to mutate
shell state to trigger a re-emit (which the running shell can't observe
anyway — flipping an env var from outside has no effect on the live
session). The bootstrap script keeps a shell-scoped variable
`__warp_bindings_hash` initialized at startup to the hash emitted
alongside the first `ShellBindings` payload. On every `precmd` the
script:

1. Recomputes the 64-bit hash of the current binding table.
2. Emits the hash in the `Precmd` DCS payload (informational; the app
   uses it for telemetry and to detect mid-session resyncs).
3. If the new hash differs from `__warp_bindings_hash`, emits a fresh
   `ShellBindings` payload with the full table and updates
   `__warp_bindings_hash` to the new value.

The app-side handler simply consumes whatever arrives. Steady state is
one hash computation per prompt; the full payload is re-emitted only on
real changes (new `bindkey`, mode switch via `bindkey -v`, sourcing a
new rc file, plugin rebind). PRODUCT #26 holds because the work runs
inside `warp_precmd` after the user's command output, asynchronously to
keystrokes; PRODUCT #27 holds because the entire flow is DCS-only with
no visible shell state mutation.

### 2. Shell-bindings storage on `Shell`

Add `bindings: Option<ShellBindings>` and `active_keymap: KeymapMode` to
the `Shell` struct in `crates/warp_terminal/src/shell/mod.rs`. New
types:

```rust
pub struct ShellBindings {
    pub schema_version: u32,
    pub keymaps: HashMap<KeymapMode, Vec<ShellBinding>>,
    pub table_hash: u64,
}

pub struct ShellBinding {
    pub keys: Vec<Keystroke>,           // parsed from "^X^E", "\C-x\C-e", "\\cx\\ce"
    pub widget: ShellWidget,            // see #3
    pub raw_widget_name: String,        // for telemetry/debug UI
}

pub enum KeymapMode { Emacs, ViInsert, ViCommand, ViVisual, Other(String) }
```

Mutation flows through a new `ModelEvent::ShellBindingsUpdated { tab_id,
bindings }` raised when a `ShellBindings` DCS hook arrives.
`active_keymap` is updated from the `Precmd` payload.

### 3. Widget mapping

`ShellWidget` is an enum covering the widgets enumerated in PRODUCT.md
#10 — e.g. `BackwardKillWord`, `KillLine`, `AcceptLine`, `Yank`,
`HistorySearchBackward`, `ViCmdMode`, `CompleteWord`,
`SelfInsert(String)`, `Unsupported(String)`. Parsing
`bindkey -L` / `bind -p` / fish `bind` happens in a new
`crates/warp_terminal/src/shell/bindings.rs` with three small parsers
(one per shell) feeding a common normalizer.

This forces a real expansion of `InputAction` in
`app/src/terminal/input.rs`. Today's coarse actions are not granular
enough; we add fine-grained verbs that match ZLE/readline semantics
(`BackwardKillWord`, `KillLine`, `TransposeChars`, `UpHistory`,
`HistorySearchBackward`, `Yank`, `YankPop`, `ViChange`, …) and route
them through `InputBufferModel`. Many of these are small additions
because the buffer already supports the underlying mutations
(word-aware cursor motion, kill-ring) — they just lack public action
entry points.

A widget→`InputAction` map (`shell/widget_dispatch.rs`) is the bridge:
honored widgets dispatch the matching `InputAction`,
`SelfInsert(string)` writes the literal string to the buffer at the
cursor, `Unsupported(name)` returns a sentinel that tells the matcher
to fall through (PRODUCT #11, #16).

### 4. Keymap matcher integration

Extend `Keymap` in `crates/warpui_core/src/keymap.rs` with a third
binding tier that lives outside the persisted user keymap:

```rust
pub struct Keymap {
    pub fixed_bindings: Vec<Binding>,
    pub editable_bindings: Vec<Binding>,
    pub shell_bindings: Vec<ShellTabBinding>,   // new
}
```

`ShellTabBinding` carries a tab id and the parsed `ShellBinding`. The
matcher consults bindings in this order (PRODUCT #14):

1. `editable_bindings` scoped to tabs of any kind (user Warp overrides)
2. `shell_bindings` for the current tab's `tab_id` and `active_keymap`
3. `fixed_bindings` (Warp defaults)

`shell_bindings` are populated by the `ShellBindingsUpdated` event and
cleared on tab close. Multi-tab independence (PRODUCT #5, #17) falls
out of tab-scoping naturally. Switching tabs swaps which
`shell_bindings` set is consulted via the existing
`ContextPredicate`-style filtering.

Mid-sequence handling for multi-key bindings (`^X^E`, `gg`) reuses the
existing `Matcher::match_keystrokes` prefix logic — the shell bindings
participate in the same sequence machine, so PRODUCT #8 needs no
special case.

### 5. Settings, feature flag, debug surface

- New boolean setting in `app/src/terminal/keys_settings.rs` via
  `define_settings_group!`: `honor_shell_bindkeys` (default `true`)
  with `toml_path: "terminal.input.honor_shell_bindkeys"`. The matcher
  short-circuits the `shell_bindings` tier when this is off (PRODUCT
  #24).
- New `FeatureFlag::HonorShellBindkeys` in
  `crates/warp_features/src/lib.rs` so we can stage rollout
  (default off → dogfood → preview → stable). Resolves PRODUCT
  open-question #23.
- Read-only debug view (PRODUCT #25): a small panel under the
  Keybindings settings section that lists the active tab's
  `ShellBindings` as `key → widget (status)` rows. Status is derived
  by walking the matcher precedence chain. No new persistence.
- Telemetry events in
  `app/src/server/telemetry/events.rs`:
  - `HonorShellBindkeysToggled { enabled: bool }`
  - `ShellBindkeysQueryFailed { shell_type, reason }`
  - `UnsupportedShellBindkeyWidget { shell_type, widget_name }` — the
    `widget_name` field is sent verbatim only when it appears in the
    shell-vocabulary allowlist (the well-known ZLE/readline/fish
    widget names enumerated in PRODUCT #10). Names outside the
    allowlist (user-defined functions, plugin-private widgets) are
    redacted to the literal string `user-defined`. Key contents and
    binding bodies are never sent. The allowlist lives in
    `crates/warp_terminal/src/shell/bindings.rs` so it is the same
    source of truth used by the parser.
  - `ShellBindkeysApplied { shell_type, honored_count,
    unsupported_count }` once per tab on first apply.

### Open questions carried from PRODUCT.md

- **#11 (user-defined named widgets)** — v1 marks them `Unsupported` and
  falls through. Forwarding the keystroke to the shell so it can run
  the widget is feasible (write the key on the PTY) but introduces
  ordering hazards with Warp's input editor; deferred.
- **#13 (vi-mode signal)** — vi mode is tracked by an in-app state
  machine, not by polling the shell. Reading the shell's mode only at
  `precmd` would miss every transition that fires inside the input
  editor (Esc → command, `i` → insert, `v` → visual, etc.) because no
  prompt hook runs between those keystrokes. Concretely:

  - `active_keymap: KeymapMode` lives on each tab's `Shell` struct
    (see Proposed Changes #2).
  - **Initial state and resync** come from the shell. The bootstrap
    payload includes the current mode (zsh `$KEYMAP`, bash
    `bind -v | grep editing-mode`, fish `$fish_bind_mode`); each
    `Precmd` payload also includes the mode and is treated as
    authoritative — if it disagrees with the in-app state, the
    in-app state is corrected to the shell's value, since the shell
    just observed whichever sequence of widgets actually executed.
  - **Transitions between prompts** are driven by the dispatched
    widget. The widget dispatcher maintains a small transition table:
    `vi-cmd-mode` / Esc → `ViCommand`, `vi-insert` /
    `vi-add-next` / `vi-add-eol` / `vi-substitute` /
    `vi-change-whole-line` → `ViInsert`, `vi-replace` → `ViReplace`,
    `vi-visual` → `ViVisual`, `accept-line` → reset to shell-reported
    mode at next prompt. The dispatcher updates `active_keymap`
    synchronously *before* the next keystroke is matched, so the
    next keystroke resolves against the new keymap.
  - This is the only feasible model: any per-keystroke shell roundtrip
    would require an invisible-exec primitive (we don't have one) or
    block on the PTY (violates PRODUCT #26).
- **#22 (AI prompt input)** — v1: not honored. The matcher's tab-scoped
  `shell_bindings` tier only activates on tabs whose focus is the shell
  command input editor, not on the AI prompt input.
- **#23 (rollout)** — gated by `FeatureFlag::HonorShellBindkeys` (above).

## Risks and mitigations

- **Bootstrap script size and shell start latency.** The query adds a
  burst of work at shell start. Mitigation: dump in a single
  invocation per keymap, drop output through DCS without invoking
  external binaries, and benchmark on the slowest of our supported
  shells. Budget: < 30 ms added to shell start; if a real shell blows
  this we move that keymap behind on-demand fetch.
- **Plugin / framework interactions** (oh-my-zsh, prezto, fzf widgets,
  zsh-vi-mode). These rebind heavily and often dynamically. The hash
  re-query in `Precmd` (#1) catches any rebind that's settled before
  a prompt redraws. Vi-mode plugins that swap keymaps reactively are
  tracked through the `KEYMAP` payload field.
- **Widget coverage gaps.** Many widgets have no Warp equivalent
  initially. The `Unsupported(name)` fallthrough plus telemetry on
  hit count tells us which to prioritize.
- **Privacy.** Telemetry never includes key contents or widget bodies.
  Widget names are sent verbatim only when in the shell-vocabulary
  allowlist; user-defined or otherwise unknown names are redacted to
  the bucket `user-defined` (see Proposed changes #5).
- **DCS spoofing.** Arbitrary process output containing a DCS sequence
  could otherwise rewrite local key handling. Mitigated by the per-tab
  nonce gate, size cap, and strict schema validation described in
  Proposed changes #1.
- **Bootstrap parsing fragility.** `bindkey -L`, `bind -p`, and fish
  `bind` outputs are stable but quoting differs. Each parser has a
  property-test fixture set covering edge cases (escapes, multi-byte,
  bound to nothing, named widgets).

## Testing and validation

Tests are organized to map to numbered PRODUCT invariants. Use
`rust-unit-tests` for new crate-level coverage and
`warp-integration-test` for end-to-end flows.

- **Bootstrap parsers** — unit tests in
  `crates/warp_terminal/src/shell/bindings.rs` per shell, covering
  fixtures generated from real `bindkey -L` / `bind -p` / `bind`
  output. Asserts widget normalization. Covers PRODUCT #2, #9, #10,
  multi-key sequences for #8.
- **Matcher precedence** — unit tests in
  `crates/warpui_core/src/keymap/matcher.rs` that assert resolution
  order across fixed / editable / shell tiers. Covers PRODUCT #14, #15.
- **Tab independence** — unit test that two `Shell` instances carry
  independent `bindings`; matching one tab's keystroke does not
  consult another tab's shell bindings. Covers PRODUCT #5, #17.
- **Lifecycle** — integration test (`crates/integration`) that boots a
  zsh shell with a known rc file declaring `bindkey '^X^E' kill-line`,
  starts a Warp tab, types `^X^E`, asserts the buffer was killed.
  Repeat for bash and fish with shell-appropriate equivalents. Covers
  PRODUCT #1, #2, #7.
- **Dynamic rebind** — integration test that types
  `bindkey '^X^E' beginning-of-line` at the prompt, presses Enter,
  then `^X^E` on the next prompt and asserts the new behavior. Covers
  PRODUCT #4.
- **Vi mode** — integration test that runs `bindkey -v`, switches to
  command mode, presses `gg`, asserts cursor at buffer start. Covers
  PRODUCT #13.
- **Unsupported widget fallthrough** — integration test binding a key
  to a user-defined named widget; assert Warp default fires on that
  key and a telemetry event is recorded. Covers PRODUCT #11, #16.
- **Conflict precedence with user Warp keybinding** — set a Warp
  keybinding for `^A`, also have shell `bindkey '^A' kill-whole-line`,
  assert Warp keybinding wins. Covers PRODUCT #14 #1.
- **Shell start failure** — integration test where the bootstrap
  errors mid-script: bindings are absent, default keymap applies, no
  crash. Covers PRODUCT #3, #28.
- **Pre-bootstrap keystroke** — type before the `Bootstrapped` payload
  arrives; assert the keystroke is handled with Warp defaults and not
  buffered. Covers PRODUCT #26.
- **Setting toggle** — flip `honor_shell_bindkeys` off mid-session;
  assert shell bindings stop applying without restart; flip on; assert
  re-query happens. Covers PRODUCT #24.
- **Manual** — run Warp against a developer's real zsh+oh-my-zsh
  config, a real bash with a populated `~/.inputrc`, and a real fish
  with `bind` declarations in `~/.config/fish/`. Capture a short loom
  walkthrough showing each shell's bindings honored.

## Follow-ups

- Forward unsupported user-defined widgets back to the shell (PRODUCT
  #11 follow-up).
- Honor remote-shell bindings over SSH (PRODUCT #18).
- Re-query on subshell transitions (PRODUCT #19).
- Optional opt-in: honor shell bindings in the AI prompt input
  (PRODUCT #22).
- Extend to PowerShell, nushell, xonsh once the core lands.
