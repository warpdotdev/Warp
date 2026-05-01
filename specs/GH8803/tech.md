# Tech Spec: User-configurable language servers

**Issue:** [warpdotdev/warp#8803](https://github.com/warpdotdev/warp/issues/8803)

## Context

Warp's LSP layer currently treats every language server as a closed-set enum variant on `LSPServerType` (`crates/lsp/src/supported_servers.rs`). Each variant has:

- A `LanguageServerCandidate` impl in `crates/lsp/src/servers/<name>.rs` (rust_analyzer, gopls, pyright, intelephense, etc.).
- An entry in `LanguageId` (`crates/lsp/src/config.rs:25-36`) that maps file extensions to language identifiers.
- A `LanguageId::lsp_language_identifier` arm and a `LanguageId::from_path` arm.

Adding a new language requires touching all four sites. PRs #9562 (PHP Intelephense) and #9568 (JSON via vscode-json-languageserver) demonstrated this pattern but were closed by maintainer @kevinyang372 in favor of this user-configurable path. The infrastructure those PRs built (probe-spawn install detection, executable-bit checks, cross-platform PATH handling, `INSTALL_PROBE_TIMEOUT` via `warpui::r#async::FutureExt::with_timeout`) is reusable here.

### Relevant code

| Path | Role |
|---|---|
| `crates/lsp/src/language_server_candidate.rs` | The trait every server impl satisfies. The natural extension point — a new impl, `UserConfiguredLanguageServer`, will go here. |
| `crates/lsp/src/supported_servers.rs` | `LSPServerType` enum + the closed registry of impls. Will grow a new arm carrying user config. |
| `crates/lsp/src/config.rs` | `LanguageId` enum + `from_path` extension mapping + `lsp_language_identifier`. Needs a path that bypasses the enum for user-configured types. |
| `crates/lsp/src/manager.rs` | Spawns/owns running LSP processes. The new code lives here for per-workspace lifecycle. |
| `app/src/settings/` | Settings group definitions (see `app/src/settings/input.rs` for the macro pattern). Where `[lsp.servers]` parsing lands. |
| `app/src/code/editor/` | Editor footer rendering — where the "Enable `<name>` for this workspace?" chip surfaces. |

### Related closed PRs (input to this spec)

- #9562 — PHP Intelephense as built-in. Closed; lessons: probe-spawn `--stdio` with bounded timeout, executable-bit check on Unix, executable's full PATH search via `binary_in_path` helper, cross-platform tests via `std::env::join_paths`.
- #9568 — JSON via vscode-json-languageserver. Closed; lessons: schema-fetching is a security surface (the JSON server defaults to fetching `http`/`https`/`file` schema URIs); `initializationOptions` is the right place for per-server restrictions like `handledSchemaProtocols`.

## Proposed changes

### 1. New settings group: `LspSettings`

**File:** new `app/src/settings/lsp.rs`. Pattern matches `app/src/settings/input.rs` (`define_settings_group!` macro).

The group holds a single setting, `custom_servers`, of type `Vec<UserLspServerConfig>`. The struct:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserLspServerConfig {
    pub name: String,
    pub command: Vec<String>,
    pub file_types: Vec<String>,
    #[serde(default)]
    pub root_files: Vec<String>,
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
    #[serde(default = "default_start_timeout_ms")]
    pub start_timeout_ms: u64,
}

fn default_start_timeout_ms() -> u64 { 5000 }
```

Validation runs at parse time (in a `validate()` method called from the settings init path):

- `name` non-empty and unique across the vec.
- `command` non-empty.
- `file_types` non-empty; each entry stripped of leading `.`.
- `start_timeout_ms` ≥ 100 and ≤ 60_000.

Validation failures emit a settings-error notification (existing pattern in `app/src/settings/initializer.rs`) with the offending entry index. Other entries continue to load.

### 2. Per-workspace enablement state

**File:** new fields on the existing per-workspace settings struct (`app/src/workspace/state.rs` or similar — exact location depends on where workspace-scoped state lives; verify against current code at implementation time).

```rust
pub struct WorkspaceLspState {
    /// Set of `UserLspServerConfig.name` values the user explicitly enabled
    /// for this workspace. Survives Warp restart via the existing workspace
    /// state persistence path.
    enabled_custom_servers: HashSet<String>,
    /// Set of `UserLspServerConfig.name` values dismissed via the chip.
    /// Suppresses the chip until cleared from settings UI.
    dismissed_custom_servers: HashSet<String>,
}
```

### 3. New `LanguageServerCandidate` impl

**File:** new `crates/lsp/src/servers/user_configured.rs`.

```rust
pub struct UserConfiguredLanguageServer {
    config: UserLspServerConfig,
    workspace_root: PathBuf,
}

#[async_trait]
impl LanguageServerCandidate for UserConfiguredLanguageServer {
    async fn should_suggest_for_repo(&self, path: &Path, _executor: &CommandBuilder) -> bool {
        // Heuristic: any file in `path` matches one of `file_types`,
        // OR (if `root_files` is non-empty) one of those globs is present
        // in `path` or any ancestor up to the workspace root.
        // Implementation-wise: glob over the immediate dir for `file_types`,
        // walk parents for `root_files`.
        ...
    }

    async fn is_installed_in_data_dir(&self, _executor: &CommandBuilder) -> bool {
        // We never install user-configured servers. Always false.
        false
    }

    async fn is_installed_on_path(&self, executor: &CommandBuilder) -> bool {
        // Reuse the patterns from #9562's probe:
        //   1. `binary_in_path` filesystem check for `command[0]` (with executable-bit check on Unix)
        //   2. NO probe-spawn — that contract was specific to npm-installed servers
        //      that may have stale shims. For user-configured servers we trust
        //      the user; an unhealthy spawn is surfaced via the start-time
        //      error toast instead.
        binary_in_path(&self.config.command[0], executor.path_env_var())
    }

    async fn install(&self, _: LanguageServerMetadata, _: &CommandBuilder) -> anyhow::Result<()> {
        // User-configured servers are never installed by Warp. The user owns
        // the lifecycle. Surface a clear error if anything tries to call this.
        anyhow::bail!("user-configured LSP `{}` is not installable by Warp", self.config.name)
    }

    async fn fetch_latest_server_metadata(&self) -> anyhow::Result<LanguageServerMetadata> {
        anyhow::bail!("user-configured LSP `{}` has no version metadata", self.config.name)
    }
}
```

### 4. `LSPServerType` extension

**File:** `crates/lsp/src/supported_servers.rs`.

Add a variant:

```rust
pub enum LSPServerType {
    // ...existing variants...
    UserConfigured(UserLspServerConfig),
}
```

The construction site that walks built-in servers becomes:

```rust
fn all_candidates_for(workspace: &Workspace, settings: &LspSettings) -> Vec<Box<dyn LanguageServerCandidate>> {
    let mut out = built_in_candidates();
    for cfg in &settings.custom_servers.0 {
        if workspace.lsp_state.enabled_custom_servers.contains(&cfg.name) {
            out.push(Box::new(UserConfiguredLanguageServer::new(cfg.clone(), workspace.root.clone())));
        }
    }
    out
}
```

### 5. Bypassing `LanguageId` for custom file types

The `LanguageId` enum is used to populate the `languageId` field in LSP `textDocument/didOpen`. For user-configured servers, the user provides `file_types` directly. Two implementation options:

**Option A — extend the enum with a `Custom(String)` variant.** The `Custom(String)` carries the file extension and `lsp_language_identifier()` returns it as-is.

**Option B — keep the enum closed and add a parallel `LspLanguageId` type that's either `Builtin(LanguageId)` or `Custom { extension: String, identifier: String }`.**

**Recommendation: Option A.** It's mechanical (one variant + two match arms) and avoids splitting the type system. The downside (more `Custom(...)` arms in match exhaustiveness) is acceptable.

### 6. Footer chip

**File:** `app/src/code/editor/` — extending whatever surface renders the LSP-related chip when a built-in server is detected-but-not-installed (find via `grep -rn "Install" app/src/code/editor/`).

The chip rendering branches on three states:

1. Built-in server detected, not installed → existing "Install `<server>`" chip.
2. Custom server defined, not enabled in workspace, not dismissed → new "Enable `<name>`" chip.
3. Custom server defined, but `command[0]` not on PATH → "Configure `<name>`" chip with `Open settings` action (variant of state 2).

Click handlers:

- `Enable` → mutate `WorkspaceLspState.enabled_custom_servers`, persist, trigger `manager::start_for_workspace` for the new candidate.
- `Dismiss` → mutate `dismissed_custom_servers`, persist, no spawn.
- `Open settings` → existing settings-open-with-search action, scoped to `lsp.servers`.

### 7. Settings reload handling

**File:** `app/src/settings/initializer.rs` — extend the existing reload path.

On settings change:

1. Compute diff of `[lsp.servers]` between old and new config (by `name`).
2. For added entries: nothing immediate; chip will appear on next matching file open.
3. For removed entries: shut down any running instance for that name, clear from `enabled_custom_servers` in all workspaces.
4. For changed entries (config differs but `name` matches): if currently running, restart with new config.

The diff mechanism reuses existing settings-event hooks; no new infra needed.

### 8. Lifecycle: spawn, initialize, timeout, shutdown

**File:** `crates/lsp/src/manager.rs`.

The existing `start_for_workspace` (or whatever the entry-point is named — verify) spawns built-in servers. Extending it for user-configured servers means:

- Build the `Command` from `config.command[0]` + `config.command[1..]`.
- `stdin/stdout` piped, `stderr` captured to a per-server ring buffer (last 200 bytes for error toasts).
- Wrap the `initialize` request in `warpui::r#async::FutureExt::with_timeout(Duration::from_millis(config.start_timeout_ms))`. Three outcomes (matches the pattern from `feat/9168-php-lsp-intelephense` commit `31285c4`):
  - `Ok(Ok(_))` — server initialized; route LSP traffic.
  - `Ok(Err(err))` — JSON-RPC error; surface notification with err message.
  - `Err(timeout)` — kill child via `Drop`, surface timeout notification.
- Forward `config.initialization_options` (TOML → `serde_json::Value`) into `InitializeParams.initialization_options` on the request.
- On Warp shutdown: existing `shutdown` + `exit` flow already handles all running servers; user-configured servers ride the same path.

## Testing and validation

Each invariant from `product.md` maps to a test at this layer:

| Invariant | Test layer | File |
|---|---|---|
| 1, 2 (parse / validate) | unit | `app/src/settings/lsp_tests.rs` (new) — TOML strings → `LspSettings` parse outcomes (success cases + each validation error). |
| 4, 5, 6, 7 (spawn outcomes) | unit | `crates/lsp/src/servers/user_configured_tests.rs` (new) — mock `CommandBuilder` returning success / error / hang. |
| 4 (`initialization_options` forwarded) | unit | same file — assert the `InitializeParams` JSON contains the configured options byte-equivalent. |
| 7 (timeout via `with_timeout`) | unit | same file — wire a future that never resolves, assert the timeout branch fires within `start_timeout_ms + 100`. |
| 9 (settings reload restarts running servers) | unit | `crates/lsp/src/manager_tests.rs` extension — pre-populate a running server, swap config, assert restart. |
| 10 (built-in + user-configured coexist) | integration | `crates/integration/tests/` — open a `.py` file in a workspace where both pyright (built-in) and a user-configured "ruff" server match; assert both are in the active candidate list. |
| 12 (per-workspace enablement persists across restart) | unit | workspace-state serialization round-trip. |
| 3, 8, 11, 14 (chip behavior) | integration | UI integration test under `crates/integration/`. Stub the file-open event, assert chip presence/absence based on enablement+dismissal state. |
| 13 (TOML→JSON shape preservation) | unit | parse a TOML with nested table + array, assert resulting `serde_json::Value` is structurally equivalent. |
| 15 (graceful shutdown on quit) | unit | shutdown handler test — running user-configured server receives `shutdown` then `exit` then has 1s window before SIGKILL. |

### Cross-platform constraints (lessons from #9562/#9568)

- Tests building `PATH` strings must use `std::env::join_paths`, not `:`. Reuse the `make_path_var` helper introduced in #9562's tests.
- On Windows, `command[0]` may need `.exe` / `.cmd` resolution. Reuse `binary_filename` helper (also from #9562's tests).
- `std::process::Stdio::null()` for stdin during `is_installed_on_path` check is a no-op for user-configured servers in this design (we don't probe-spawn), but if we ever do, the same pattern from `intelephense.rs:113` applies.

## End-to-end flow

```
User edits settings.toml
  └─> [LspSettings::reload]                                        (settings/lsp.rs)
        ├─> validate()                                              (rejects malformed)
        └─> emit SettingsChanged event
              └─> [manager::on_settings_change]                     (lsp/src/manager.rs)
                    ├─> diff old vs new custom_servers
                    ├─> stop removed entries
                    └─> restart changed entries that are running

User opens .lua file in workspace W
  └─> [editor::on_file_open]                                       (app/src/code/editor/)
        └─> [chip_renderer]
              ├─> get LspSettings.custom_servers
              ├─> filter where file_types contains "lua"
              ├─> filter where W.enabled_custom_servers does NOT contain name
              ├─> filter where W.dismissed_custom_servers does NOT contain name
              └─> render "Enable <name>" chip per remaining entry

User clicks Enable
  └─> [chip_handler::on_enable_clicked]
        ├─> W.enabled_custom_servers.insert(name)
        ├─> persist W
        └─> [manager::start_candidate]                              (lsp/src/manager.rs)
              ├─> construct UserConfiguredLanguageServer            (servers/user_configured.rs)
              ├─> is_installed_on_path → if false, surface "Configure <name>" toast
              ├─> spawn Command with stderr-buffered
              ├─> initialize.with_timeout(start_timeout_ms)
              │     ├─> Ok(Ok(_)) → register, route LSP traffic
              │     ├─> Ok(Err(err)) → surface error toast, drop child
              │     └─> Err(_timeout) → kill child via Drop, surface timeout toast
              └─> on subsequent file opens, no chip — server already running
```

## Risks

- **`initialization_options` is a security surface.** Some servers (e.g. JSON via vscode-json-languageserver, see #9568) default to fetching remote schemas. The user controls this knob, but they may not realize it. **Mitigation:** ship `docs/custom-lsp-examples.md` with annotated examples that explicitly set network-related options to safe defaults where applicable.
- **A configured server that crashes on every spawn could loop forever.** **Mitigation:** the chip's `Enable` action is one-shot per click. We do not auto-restart on crash; the user re-enables manually. If exit happens after `initialize` succeeded, we surface a "server crashed" toast and the chip returns to the disabled-but-defined state.
- **`Custom(String)` propagation in `LanguageId`.** Adding a `Custom` variant to a closed exhaustive enum touches every match site. **Mitigation:** WARP.md prohibits wildcard matches, so the compiler will surface every site at code-write time. Audit during implementation.
- **Per-workspace state migration.** Existing workspaces have no `enabled_custom_servers` field. **Mitigation:** `#[serde(default)]` on the new fields; absence parses as empty set.

## Follow-ups (out of this spec)

- `nix flake check`-validated dev shell with all referenced LSP binaries pre-installed (would help testing).
- Settings UI: a "Custom language servers" page under Settings → Code intelligence that lists configured servers + workspace-enablement state with `Enable`/`Disable` buttons (currently described only in product.md user-experience section).
- `~` and `$VAR` expansion in `command[0]`. Recommend yes for `~` (consistency with `/open-file`); defer `$VAR`.
- Document `network_access` semantics if we later add a generic per-server flag. The JSON-server-specific `handledSchemaProtocols` discovery from #9568's review is the model.
