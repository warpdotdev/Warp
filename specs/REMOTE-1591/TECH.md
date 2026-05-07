# REMOTE-1591: Tech spec — Environment creation modal for handoff

## Context

When a user enters `&` handoff-compose mode but has zero cloud environments, there is currently no way to create one without leaving the flow. See `PRODUCT.md` for detailed user-facing behavior.

### Relevant code

**Handoff compose flow (input layer)**
- `app/src/terminal/input.rs` — `maybe_launch_cloud_handoff_request()` (line ~3810) is the Enter handler for `&` compose mode. Currently returns `true` (consumed) when the prompt is empty, or collects attachments and dispatches `WorkspaceAction::OpenLocalToCloudHandoffPane`.
- `app/src/terminal/input/handoff_compose.rs` — `HandoffComposeState` model tracking `&` mode activation and selected environment.

**Environment selector chip**
- `app/src/ai/blocklist/agent_view/agent_input_footer/environment_selector.rs` — `refresh_button()` (line 437) sets the chip label. Falls back to `"New environment"` when no env is selected.

**Environment creation form**
- `app/src/settings_view/update_environment_form.rs` — `UpdateEnvironmentForm` view. Already supports modal-style use via `show_header` (line 332), `should_handle_escape_from_editor` (line 336), and `auth_source` (line 342). Emits `UpdateEnvironmentFormEvent::Created { environment, share_with_team }` on submit.

**Prior art: embedding the form in non-settings contexts**
- `app/src/terminal/view/ambient_agent/first_time_setup.rs` — `FirstTimeCloudAgentSetupView` wraps the form with `show_header=false`, `should_handle_escape_from_editor=true`, handles `Created` by calling `UpdateManager::create_ambient_agent_environment()`. This is the pattern to follow for environment creation logic.

**Prior art: modal overlay rendering**
- `app/src/settings_view/agent_assisted_environment_modal.rs` — `AgentAssistedEnvironmentModal` renders using `Dialog::new().with_close_button().with_child().build()` wrapped in `Dismiss::new().prevent_interaction_with_other_elements()`, inside a `Container` with `ColorU::new(0, 0, 0, 179)` background. Uses `show()`/`hide()` visibility toggle and emits `Cancelled`/`Confirmed` events. This is the rendering pattern to follow.
- `app/src/ui_components/dialog.rs` — `Dialog` component used by modal overlays. Provides title, close button, child content, and bottom row.

**Workspace handoff dispatch**
- `app/src/workspace/view.rs` — `start_local_to_cloud_handoff()` (line 12972) and `start_fresh_cloud_launch()` (line 12938) handle `WorkspaceAction::OpenLocalToCloudHandoffPane`. The workspace is also where top-level overlays like `remove_tab_config_confirmation_dialog` are owned and rendered.

## Proposed changes

### 1. New view: `HandoffEnvironmentCreationModal`

New file: `app/src/settings_view/handoff_environment_creation_modal.rs`

A thin modal wrapper around `UpdateEnvironmentForm`, following the `AgentAssistedEnvironmentModal` pattern for rendering and `FirstTimeCloudAgentSetupView` for form configuration and environment creation logic.

**View state:**
- `visible: bool`
- `environment_form: ViewHandle<UpdateEnvironmentForm>`
- `close_button_mouse_state: MouseStateHandle`
- `scroll_state: ClippedScrollStateHandle` (the form is tall — needs scrolling within the modal)

**Public API:**
- `show(&mut self, ctx)` — sets `visible = true`, resets form to `Create` mode, focuses name field
- `hide(&mut self, ctx)` — sets `visible = false`
- `is_visible(&self) -> bool`

**Events:**
```rust
enum HandoffEnvironmentCreationModalEvent {
    Created { env_id: SyncId },
    Cancelled,
}
```

The `Created` event carries the `SyncId` of the newly created environment (computed from the `ClientId` used in `UpdateManager::create_ambient_agent_environment`). The modal handles environment creation internally — the caller only sees the resulting `SyncId`.

**Form configuration** (matching `FirstTimeCloudAgentSetupView`):
- `show_header = false` → submit button renders at bottom-right of form body
- `should_handle_escape_from_editor = true` → Escape in any editor emits `Cancelled`
- `auth_source = AuthSource::CloudSetup` → GitHub auth redirects back in-place

**Environment creation** (inside the modal's `handle_environment_form_event`):
- On `UpdateEnvironmentFormEvent::Created { environment, share_with_team }`:
  1. Resolve owner via `cloud_environments::owner_for_new_environment()` / `owner_for_new_personal_environment()`
  2. Generate `ClientId::default()`
  3. Call `UpdateManager::create_ambient_agent_environment()`
  4. Emit `HandoffEnvironmentCreationModalEvent::Created { env_id: SyncId::ClientId(client_id) }`
  5. The environment is immediately available in `CloudModel` after this call

**Rendering** (following `AgentAssistedEnvironmentModal`):
- When `visible == false`, render `Empty`
- When visible: `Dialog::new("Create environment", None, dialog_styles(appearance)).with_close_button(...).with_child(scrollable_form).with_width(MODAL_WIDTH).build()` → `Dismiss::new().prevent_interaction_with_other_elements().on_dismiss(cancel)` → `Container` with dark overlay background + window corner radius

### 2. Input: intercept Enter when no environments exist

In `input.rs`, `maybe_launch_cloud_handoff_request()`:

After confirming we're in handoff compose mode, before collecting attachments and dispatching the handoff, add a check:

```
if CloudAmbientAgentEnvironment::get_all(ctx).is_empty() {
    if prompt.is_empty() {
        // Behavior 6: nothing happens on empty buffer with no envs
        return true;
    }
    // Behavior 5: open environment creation modal
    ctx.emit(Event::OpenHandoffEnvironmentCreationModal);
    return true;
}
```

Add a new variant to the `Input::Event` enum:
```rust
OpenHandoffEnvironmentCreationModal,
```

The prompt and attachments stay in the input buffer — the user will see them unchanged when the modal closes (behavior 14).

### 3. Workspace: own the modal and wire up handoff auto-submit

In `workspace/view.rs`:

**New field** on `WorkspaceViewState` (or `Workspace`):
```rust
handoff_environment_creation_modal: ViewHandle<HandoffEnvironmentCreationModal>,
```

**Subscribe to modal events** during workspace construction:
- `HandoffEnvironmentCreationModalEvent::Created { env_id }` →
  1. Hide the modal
  2. Get the active terminal view's input and read the prompt + attachments from its `&` compose state
  3. Use the input's `collect_cloud_launch_attachments()` and `editor.buffer_text()` to build a `PendingCloudLaunch`
  4. Clear the input buffer and exit `&` compose mode
  5. Dispatch `WorkspaceAction::OpenLocalToCloudHandoffPane { launch, explicit_environment_id: Some(env_id) }`
- `HandoffEnvironmentCreationModalEvent::Cancelled` →
  1. Hide the modal
  2. Re-focus the active terminal input (the `&` compose state and prompt are already preserved)

**New workspace action** `ShowHandoffEnvironmentCreationModal` — shows the modal. The terminal view subscribes to `Input::Event::OpenHandoffEnvironmentCreationModal` and dispatches this action.

**Render** — add the modal overlay to the workspace's render output when `is_visible()` returns `true`, using the same overlay stacking pattern as existing workspace modals.

### 4. Ghost text fallback

In `input.rs`, `set_zero_state_hint_text()` (line ~6120), the handoff compose branch falls back to `CLOUD_HANDOFF_HINT_TEXT` when no environment is found. Update this constant from `"Start a cloud run"` to `"Handoff to cloud"` to match behavior 2 in `PRODUCT.md`.

## Testing and validation

**Unit tests** (in `handoff_compose_tests.rs` and `input_test.rs`):
- Test that `maybe_launch_cloud_handoff_request` emits `OpenHandoffEnvironmentCreationModal` when environments list is empty and prompt is non-empty (behavior 5)
- Test that empty prompt + no environments returns `true` without emitting (behavior 6)
- Test that with environments present, Enter submits normally (behavior 16)

**Unit tests** (new file `handoff_environment_creation_modal_tests.rs`):
- Test `show()` resets form to Create mode and sets `visible = true`
- Test that form `Created` event triggers `UpdateManager::create_ambient_agent_environment` and emits `Created { env_id }`
- Test that form `Cancelled` event emits `Cancelled`

**Manual validation:**
- Enter `&` with no environments → ghost text shows "Handoff to cloud"
- Type a prompt and press Enter → modal opens with environment creation form, prompt preserved in input behind modal
- Fill out form, submit → environment created, handoff auto-submits with new environment
- Escape from modal → returns to `&` compose with prompt intact
- Create env via Settings while in `&` mode → chip updates reactively (behavior 17)
