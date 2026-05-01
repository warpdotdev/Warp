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
   up the change without requiring a restart of the tab. The implementation
   may either re-query on a signal (prompt redraw, mode change, OSC hint) or
   re-query periodically — but the user-visible invariant is: a binding
   declared at the shell prompt is honored on the next keystroke after the
   declaration completes.

5. Each tab tracks its own bindings independently. Changing bindings in one
   tab does not affect another tab, even if both run the same shell.

6. Closing and reopening a tab re-queries from scratch. Warp does not cache
   bindings across tab restarts; the user's current shell state is always
   the source of truth.

### Honoring bindings in the input editor

7. While the user is typing in the shell command input editor, every key
   press is matched against the user's binding table for the active keymap
   first. If a match is found and the bound action has a Warp equivalent,
   Warp performs that action and consumes the keystroke. Otherwise the
   keystroke falls through to Warp's default handling (see #14 for the
   precedence list).

8. Multi-key sequences (`^X^E`, `^[f`, `gg`, fish `\\cx\\ce`) are honored as
   a single action. While Warp is mid-sequence (one or more matching prefix
   keys received but the sequence is not yet uniquely resolved), no action
   fires and no character is inserted; the sequence either completes (action
   fires) or is abandoned by a non-matching keystroke (in which case Warp
   handles all the buffered keys as it would have without the binding —
   matching readline / ZLE behavior).

9. `self-insert` and "insert literal string" bindings (e.g. zsh
   `bindkey -s '^X' 'echo hi\n'`) insert the literal text into the input
   buffer at the cursor, exactly as the equivalent shell would. Newlines in
   the inserted string are treated as literal newlines in the input buffer
   unless the binding is `accept-line` or equivalent.

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
      that key, and Warp emits a one-time-per-session diagnostic naming the
      unsupported widget and the key it was bound to. Telemetry records the
      unsupported widget name (no key contents) so Warp can prioritize
      coverage.
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
    1. User-customized Warp keybindings (anything the user has explicitly
       set in Warp settings).
    2. User shell bindkeys for the active keymap.
    3. Warp's default keybindings.
    4. Default character insertion.

    Rationale: a key the user has explicitly bound in Warp settings is the
    strongest signal of intent. Below that, the user's shell bindings
    override Warp's defaults — that is the entire point of this issue. Warp
    defaults are the floor.

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
    the Keybindings section of settings. Toggling it off restores Warp's
    default keymap for all tabs immediately (no restart). Toggling it back
    on re-queries each tab's shell.

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
- Telemetry events for unsupported widgets, query failures, and toggle
  usage — confirm what we can / want to record under existing privacy
  rules.
