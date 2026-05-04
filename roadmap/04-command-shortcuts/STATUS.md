# 04 — Custom command shortcuts

**Phase:** not-started
**Spec PR:** —
**Impl PR:** —

## Scope

Declarative keybindings → action sequence (`new_tab`, `type:`, `press:`, `wait:`). See README §3 for the YAML sketch and the two driving examples (⌘⇧D for `claude`, ⌘⇧A for `claude` + delayed slash command).

## Sub-phases

Likely one impl PR if the config schema holds during spec review. If the schema is contentious or the action set grows past the README sketch, split into:

1. config + parser (no runtime effect)
2. executor + bindings registration

This decision belongs in TECH.md.

## Notes

- Spec phase should explicitly enumerate the action vocabulary (`new_tab`, `type`, `press`, `wait`, …) and decide what's in v1 vs deferred.
- The config format is user-facing — error messages for malformed config are part of the product spec, not just an implementation detail.
