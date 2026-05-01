# PRODUCT.md — Honor user-defined shell bindkeys in Warp's input editor

Issue: https://github.com/warpdotdev/warp/issues/537

Figma: none provided.

## Summary

Warp's input editor currently ignores user-defined keybindings declared in the
user's shell — `bindkey` in zsh, `bind` / `~/.inputrc` in bash readline, and
`bind` in fish. When a user types in a Warp prompt, those customizations have
no effect, even though the same keys work in any other terminal running the
same shell. This spec covers honoring those user bindings inside the Warp
input editor (the prompt where shell commands are typed) for zsh, bash, and
fish, sourced from the user's actual running shell so that whatever the shell
reports is what Warp respects.

## Goals / Non-goals

In scope:

- Honoring user-defined keybindings in Warp's shell command input editor for
  zsh, bash, and fish sessions.
- Discovery via the user's live shell session (querying the shell for its
  current binding table), so dynamic and conditionally-declared bindings are
  picked up — not by parsing rc files.
- Best-effort coverage of the action set: any shell widget / readline function
  / fish input function that has a Warp-input equivalent is honored. Widgets
  with no clean Warp equivalent degrade gracefully (see below) rather than
  silently stealing the keystroke.
- Keymap modes: emacs vs vi (insert/command/visual) for the shells that
  expose them. Mode switches initiated by the user (e.g. `bindkey -v`,
  `set -o vi`, vi-mode plugins, fish bind modes) take effect without restart.
- Conflict policy with Warp's own keybindings (see Behavior #14).

Out of scope for this spec:

- Bindings inside surfaces other than the shell command input editor — the
  AI prompt input, command palette, search, settings — keep their existing
  Warp keybindings unchanged. (See Open question at #5.)
- Other shells (PowerShell, nushell, xonsh, csh family). Adding more shells
  follows the same shape but is not required for this issue to land.
- A Warp-native keybinding-import config surface where users redeclare their
  bindings inside Warp settings. The intent of this issue is "the bindings I
  already have should just work" — not "give me yet another config".
- Static parsing of `~/.zshrc`, `~/.inputrc`, `~/.config/fish/`, etc. The
  source of truth is the live shell.
- Honoring shell-level abbreviations, aliases, completions, syntax
  highlighting, or autosuggestion plugins. Only key-to-action bindings.

## Behavior

### Discovery and lifecycle

1. When a Warp tab starts a supported shell (zsh, bash, fish), Warp queries
   that shell for its current keybinding table once the shell is ready to
   accept commands but before the first user keystroke is processed by the
   input editor. Until the table arrives, the input editor uses Warp's
   default keymap; once the table arrives, user bindings take effect on the
   next keystroke.

2. The query mechanism is shell-native and visible only to Warp internals —
   the user does not see the query command echoed in their scrollback, in
   history, or in any block. Equivalents in spirit (not literal):
   - zsh: `bindkey -L` for each keymap (`main`, `emacs`, `viins`, `vicmd`,
     `vivis`, `viopp`, `command`, `isearch`, `menuselect`, plus any
     user-defined keymaps).
   - bash: `bind -p` for the current keymap and `bind -p -m emacs` /
     `-m vi-insert` / `-m vi-command` for the others.
   - fish: `bind` with no args, plus `bind -M insert` / `default` /
     `visual` / etc.

3. If the shell fails to start, exits before the query completes, or returns
   an unparseable response, Warp logs a diagnostic and falls back to its
   default keymap for that tab. The tab remains usable; no user-facing error
   toast is required.

4. When the user changes their bindings inside an existing session
   (`bindkey '^X^E' edit-command-line`, `bind '"\C-x\C-e": edit-and-execute-command'`,
   sourcing a new rc file mid-session, switching emacs/vi mode), Warp picks
   up the change without requiring a restart of the tab. Discovery is
   driven shell-side at every `precmd`, so the change is detected when
   the prompt next redraws. The user-visible invariant: a binding
   declared at the shell prompt is honored starting with the first
   keystroke after Warp has parsed the next `ShellBindings` payload
   from that prompt. Keystrokes pressed during the small async window
   between the prompt firing and the payload being parsed use the
   previous keymap (consistent with the non-blocking guarantee in #26);
   declarations never block typing.

5. Each tab tracks its own bindings independently. Changing bindings in one
   tab does not affect another tab, even if both run the same shell.

6. Closing and reopening a tab re-queries from scratch. Warp does not cache
   bindings across tab restarts; the user's current shell state is always
   the source of truth.

### Honoring bindings in the input editor

7. While the user is typing in the shell command input editor, every key
   press is resolved against the precedence ladder defined in #14
   (reserved infrastructure keys → user-customized Warp keybindings →
   user shell bindings for the active keymap → Warp's default
   keybindings → default character insertion). Shell bindings are
   consulted only after the two higher tiers have been checked and have
   not produced a match. When the matched action is a shell binding and
   the bound widget has a Warp equivalent, Warp performs that action
   and consumes the keystroke. When the bound widget is unsupported
   (#11), the keystroke continues down the ladder to Warp's defaults.

8. Multi-key sequences (`^X^E`, `^[f`, `gg`, fish `\\cx\\ce`) are honored
   as a single action. Resolution rules:

   - **Mid-sequence buffering.** While Warp has received one or more
     keys that match a prefix of a longer binding but not yet a complete
     binding, no action fires and no character is inserted; Warp waits
     for the next key.
   - **Ambiguous bindings (prefix is also a complete binding).** When
     a key sequence matches both a complete binding and a prefix of a
     longer binding (the canonical example: `^[` is `vi-cmd-mode` *and*
     a prefix of `^[f`), Warp uses a 500 ms ambiguity timeout. If
     another key arrives within 500 ms that extends the prefix, the
     longer match wins. If no key arrives within the timeout, the
     complete short binding fires.
   - **Pure-prefix timeout.** When a sequence matches a prefix but no
     complete binding (e.g. partial `^X` of `^X^E`), pending keys are
     held without timeout — readline / ZLE both wait indefinitely on
     pure prefixes. The user pressing any non-extending key abandons
     the prefix immediately (next rule).
   - **Abandonment.** When a non-matching key arrives mid-sequence,
     the prefix is abandoned: Warp replays the buffered keys plus the
     just-received key through normal handling, in arrival order. Any
     of those replayed keys may itself trigger a single-key binding;
     none of them re-enter prefix accumulation until the replay
     finishes. This matches readline / ZLE behavior.
   - **No keystroke is ever silently dropped.** Either a binding fires,
     or the buffered keys are replayed.
   - **Focus loss / window blur** mid-sequence abandons the prefix
     (replay path); on refocus, accumulation starts fresh.

   The 500 ms ambiguity timeout is the standard readline default; it
   may be made configurable in a follow-up but is fixed for v1.

9. "Insert literal string" bindings (e.g. zsh `bindkey -s '^X' 'echo hi\n'`,
   readline `"\C-x": "echo hi\n"`) inject the bound text into the input
   stream as if the user had typed each character — matching shell
   input-queue semantics, not literal text insertion. A newline in the
   bound string therefore submits the line (it triggers `accept-line`),
   `^A` moves the cursor to the start, and so on. The injected
   characters are processed through the same key-resolution chain as
   real keystrokes, including any other bindings they happen to trigger.
   Plain `self-insert` (no string macro) inserts the literal key
   character at the cursor as expected.

10. The full set of widgets / functions whose semantics Warp must honor when
    bound includes, at minimum:
    - Cursor motion: `forward-char`, `backward-char`, `forward-word`,
      `backward-word`, `beginning-of-line`, `end-of-line`,
      `beginning-of-buffer-or-history`, `end-of-buffer-or-history`.
    - Deletion: `backward-delete-char`, `delete-char`, `backward-kill-word`,
      `kill-word`, `kill-line`, `backward-kill-line`, `kill-whole-line`,
      `unix-word-rubout`, `unix-line-discard`.
    - Yank / kill ring: `yank`, `yank-pop`, `kill-region`, `copy-region-as-kill`.
    - History: `up-line-or-history`, `down-line-or-history`, `up-history`,
      `down-history`, `history-incremental-search-backward`,
      `history-incremental-search-forward`,
      `history-search-backward` / `-forward`, fish's history-pager bindings.
    - Editing: `transpose-chars`, `transpose-words`, `upcase-word`,
      `downcase-word`, `capitalize-word`, `quoted-insert`, `tab-insert`,
      `overwrite-mode`, `undo`, `redo` (where supported).
    - Submission and abort: `accept-line`, `accept-and-hold`,
      `accept-and-infer-next-history`, `accept-search`, `send-break`
      (`^C`), `eof` / `delete-char-or-list` (`^D` on empty line).
    - Vi mode: `vi-cmd-mode`, `vi-insert`, `vi-replace`, `vi-add-next`,
      `vi-add-eol`, `vi-change`, `vi-delete`, `vi-yank`, `vi-put-after`,
      `vi-put-before`, `vi-find-next-char` / `-prev-char`, `vi-repeat-find`,
      `vi-up-line-or-history`, `vi-down-line-or-history`, `vi-goto-mark`,
      `vi-set-mark`, `vi-replace-chars`, `vi-substitute`,
      `vi-change-whole-line`, plus fish-mode equivalents.
    - Completion: `complete-word`, `expand-or-complete`,
      `expand-or-complete-prefix`, `menu-complete`, `reverse-menu-complete`.
      These trigger Warp's existing completion UI (not the shell's), but
      from whichever key the user has bound them to. Behavior of the
      completion UI itself is unchanged by this spec.

11. Widgets that have no Warp equivalent (examples: `quoted-insert` in some
    edge cases, `redisplay`, `clear-screen` if Warp already binds it
    differently, user-defined named widgets / functions whose body is shell
    code) are handled as follows:
    - If the widget has a documented behavior Warp can replicate cheaply,
      Warp replicates it.
    - Otherwise the keystroke falls through to Warp's default handling for
      that key, and Warp emits a one-time-per-session diagnostic noting
      the unsupported widget. The diagnostic uses the same redaction
      policy as telemetry: the widget name is included verbatim only if
      it is in the documented shell-vocabulary allowlist (the well-known
      ZLE/readline/fish input function names enumerated in #10);
      user-defined or otherwise unknown names are written as
      `user-defined`. The bound key sequence is not included in the
      diagnostic. Telemetry records unsupported-widget hits under the
      same rule; user-defined or otherwise unknown widget names are reported as
      the bucket `user-defined` with no further identifying information,
      since user-defined widget names can be arbitrary or private. Key
      contents, key sequences, and binding bodies are never recorded.
    - **Open question:** for user-defined shell-function widgets (e.g. zsh
      `zle -N my-widget; bindkey '^X' my-widget`), v1 treats these as
      unsupported. A future iteration could forward the keystroke to the
      shell to let it execute the widget. Confirm v1 = unsupported is
      acceptable.

12. `clear-screen` (typically `^L`) clears Warp's block list to the current
    prompt, matching the user's expectation from a real terminal — even if
    Warp's default `^L` already does this, the binding must continue to
    work when remapped to another key.

### Modes

13. When the shell is in vi mode, the active keymap follows the shell's
    current mode (insert / command / visual / replace). Warp learns about
    mode transitions through the same mechanism it uses for binding
    discovery (see #4) — when the shell signals a mode change (by repaint,
    OSC, prompt update, or whichever signal the implementation lands on),
    Warp's input editor switches keymaps so the next keystroke is matched
    against the new map. Visible mode indicators (cursor shape, vim-mode
    plugin status text in the prompt) remain whatever the shell already
    drew; Warp does not add its own.
    - **Open question:** what's the canonical signal for mode change across
      the three shells? Tech spec must answer this concretely. If no
      reliable signal exists for one shell, document the fallback (poll on
      every prompt redraw, etc.).

### Precedence and conflicts

14. Resolution order for a single keystroke in the shell command input
    editor, highest priority first:
    1. Reserved infrastructure keys (see below).
    2. User-customized Warp keybindings (anything the user has explicitly
       set in Warp settings).
    3. User shell bindkeys for the active keymap.
    4. Warp's default keybindings.
    5. Default character insertion.

    Rationale: a key the user has explicitly bound in Warp settings is the
    strongest signal of intent. Below that, the user's shell bindings
    override Warp's defaults — that is the entire point of this issue. Warp
    defaults are the floor.

    **Reserved infrastructure keys.** A small set of keys is structurally
    needed for Warp ↔ shell communication (input reporting, prompt-mode
    switching, kill-buffer signaling) and cannot be honored as
    user-controlled in v1. User bindings on these keys are imported into
    Warp's debug view tagged `reserved-by-warp` and do not fire;
    bindings on every other key follow the regular precedence above.
    The reserved set per shell:

    - **zsh:** `^P` (Warp uses for `kill-buffer`), `\ei` (input reporting).
    - **bash:** `\C-p` (`kill-whole-line` for clear-buffer), `\ei`
      (input reporting), `\ep` (switch to PS1 prompt), `\ew` (switch to
      Warp prompt).
    - **fish:** `\cP` (clear input buffer), `\ep` (switch to PS1
      prompt), `\ew` (switch to Warp prompt), `\ei` (input reporting).

    These match the keys Warp's existing bootstrap already installs in
    each shell. Lifting the exception (re-implementing each integration
    point without bind-level interception) is a tracked follow-up; the
    integrations exist today and replacing them is out of scope here.

15. When a user shell binding shadows a Warp default, no warning, banner, or
    toast appears. The user already declared this binding in their shell
    config; the override is the desired behavior. Diagnostics for shadowed
    Warp defaults may be available through verbose logging but are not
    user-facing.

16. When a user shell binding cannot be honored because the bound widget is
    unsupported (#11), the keystroke falls through to Warp's default — it
    does not steal the keystroke and produce nothing. The user sees the
    Warp default fire on that key, which may differ from what their shell
    would have done. The diagnostic from #11 is the user's signal that
    something they configured isn't supported yet.

### Multi-tab and multi-shell scenarios

17. A window with multiple tabs running different shells (one zsh, one
    bash, one fish) honors each tab's bindings independently. Switching
    focus between tabs changes the active binding table to that tab's.

18. SSH sessions: when the user SSHes from a Warp tab to a remote host and
    a shell starts on the remote, Warp does not query the remote shell for
    bindings in v1. The local Warp input editor continues to use the
    bindings of the local shell that started the tab, or Warp defaults if
    the local shell wasn't a supported one. Honoring remote bindings
    requires the remote-side Warp agent and is out of scope here.

19. Subshells started inside a session (`bash` typed at a zsh prompt,
    `tmux`, etc.) keep the parent tab's binding table. The user does not
    see a re-query, and bindings the subshell may have configured do not
    take effect in the Warp input editor. Re-querying on every subshell
    transition is feasible but a follow-up.

### Surface boundaries

20. Bindings only apply while the user's input focus is in the shell
    command input editor of a tab whose shell is one of zsh / bash / fish.
    They do not affect:
    - Warp command palette, settings, search, AI prompt input,
      block-level chrome (the keystrokes there continue to use Warp's own
      keymap).
    - Tabs whose shell is not a supported shell — those tabs use Warp
      defaults.
    - Any modal overlay rendered above the input editor (file palette,
      command palette, suggestions popover focus, etc.).

21. Switching focus from the input editor to another surface and back does
    not require re-querying. The binding table from the most recent query
    remains valid for the duration of the tab unless invalidated by #4.

### AI / agent prompt input

22. The AI prompt input editor does not honor shell bindkeys by default —
    it is not a shell, and shell vi/emacs muscle memory there would
    conflict with the AI input's own conventions.
    - **Open question:** add an opt-in setting "Use my shell bindings in AI
      prompts too"? Default off either way. Resolve before implementation.

### Settings, opt-out, and discoverability

23. The feature is on by default for supported shells once it ships.
    - **Open question:** ship behind a feature flag for staged rollout
      (default off → dogfood → preview → stable), or default on from
      release? Tech spec / release plan to decide.

24. A single setting "Honor shell keybindings in input editor" lives under
    the Keybindings section of settings. Toggling it off immediately
    restores Warp's default keymap for all tabs (no restart). Toggling
    it back on resumes matching against each tab's most recently
    received binding table; any drift since the toggle was off is
    picked up on the tab's next `precmd` payload, since re-queries are
    shell-driven (see TECH.md §1). Users who want a fresh re-import
    without waiting for the next prompt can press Enter on an empty
    line, which fires `precmd` immediately.

25. The Keybindings settings page surfaces, somewhere reachable, a
    read-only view of the bindings Warp has imported for the active tab —
    enough that a user debugging "why didn't my binding work" can see what
    Warp received from the shell and which entries Warp marked unsupported.
    Format: a list of `key → action (status)` rows where status is one of
    `honored`, `shadowed-by-warp-user`, `unsupported`. The exact UI is left
    to the tech spec; the behavioral requirement is that the information is
    discoverable without enabling debug logging.

### Performance and correctness invariants

26. The initial binding query must not block the user's first keystroke
    perceptibly. If the query has not completed by the time the user types,
    the keystroke is handled with Warp defaults; it is not buffered or
    delayed. Late-arriving bindings apply to subsequent keystrokes.

27. The query must not appear in the user's shell history, in scrollback,
    in the block list, or as visible output. Side effects on the shell's
    own state (kill ring, last-status `$?`, etc.) must be avoided or
    cleaned up.

28. Receiving a malformed or partial response from the shell never causes
    a crash, hang, or stuck input editor. The fallback is always: drop the
    bad data, log a diagnostic, keep using whatever binding table was
    valid before.

29. Existing Warp keybindings that the user has not customized continue to
    work unchanged on tabs running unsupported shells, on tabs where the
    feature is disabled, and on tabs where the query has not yet completed
    or failed.

## Open questions

Collected from inline references above plus a few cross-cutting ones the
tech spec must resolve:

- v1 handling of user-defined named widgets whose body is shell code (#11).
- Canonical signal for vi-mode transitions across zsh, bash, and fish (#13).
- AI prompt input opt-in for shell bindings (#22).
- Default-on vs feature-flagged staged rollout (#23).
- (Resolved) Telemetry redaction policy for widget names — see #11; the
  rule is allowlist-or-bucket, never raw user-defined names.
