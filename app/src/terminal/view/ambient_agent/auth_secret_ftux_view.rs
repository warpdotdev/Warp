//! Auth secret FTUX view: a `TypedActionView` that renders the first-time
//! setup flow shown when a user picks a non-Oz harness and the per-harness
//! FTUX setting has not yet been completed.
//!
//! The FTUX view owns an [`AuthSecretFtuxDropdown`] (a full-width combobox)
//! and adds:
//!  - Description text explaining the flow.
//!  - A creation sub-form (one single-line editor per field + a name editor)
//!    that appears when the user picks "New {type}" from the dropdown.
//!  - Skip / Cancel / Continue buttons at the bottom.
//!
//! Events:
//!  - On a successful selection of an existing secret, the dropdown emits
//!    [`FtuxDropdownEvent::SecretSelected`]; this view sets it on
//!    `AmbientAgentViewModel` and marks the FTUX as completed.
//!  - On a successful creation, this view subscribes to
//!    [`HarnessAvailabilityEvent::AuthSecretCreated`] and finalizes by setting
//!    the new secret as selected and marking FTUX as completed.

use warp_cli::agent::Harness;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::color::internal_colors;
use warp_core::ui::theme::Fill;
use warpui::elements::{
    Border, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
    Expanded, Flex, Hoverable, MainAxisSize, MouseStateHandle, ParentElement as _, Radius, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::ai::auth_secret_types::{
    auth_secret_types_for_harness, build_managed_secret_value, AuthSecretTypeInfo,
};
use crate::ai::cloud_agent_settings::CloudAgentSettings;
use crate::ai::harness_availability::{HarnessAvailabilityEvent, HarnessAvailabilityModel};
use crate::ai::harness_display;
use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpEscapeKey, PropagateAndNoOpNavigationKeys,
    SingleLineEditorOptions, TextOptions,
};
use crate::terminal::view::ambient_agent::auth_secret_ftux_dropdown::{
    AuthSecretFtuxDropdown, FtuxDropdownEvent,
};
use crate::terminal::view::ambient_agent::{AmbientAgentViewModel, AmbientAgentViewModelEvent};
use warp_editor::editor::NavigationKey;

/// Description font size (Figma: 14px).
const DESCRIPTION_FONT_SIZE: f32 = 14.;

/// Field label font size (Figma: 10px gray).
const FIELD_LABEL_FONT_SIZE: f32 = 10.;

/// Skip-link font size (Figma: 12px).
const SKIP_FONT_SIZE: f32 = 12.;

/// Button label font size (Figma: 14px).
const BUTTON_FONT_SIZE: f32 = 14.;

/// Editor font size for the single-line input boxes.
const EDITOR_FONT_SIZE: f32 = 14.;

/// Vertical spacing between the major rows of the FTUX (description, form,
/// buttons).
const ROW_SPACING: f32 = 24.;

/// Vertical spacing between rows inside the content section (description +
/// dropdown).
const CONTENT_SECTION_SPACING: f32 = 12.;

/// Vertical spacing between fields in the creation sub-form.
const FORM_FIELD_SPACING: f32 = 8.;

/// Padding used inside the Cancel/Continue buttons.
const BUTTON_PADDING: f32 = 8.;

/// Padding inside the field input editor's Container.
const FIELD_EDITOR_PADDING: f32 = 8.;

/// Corner radius on buttons and editors.
const CORNER_RADIUS: f32 = 4.;

/// Minimum height of each single-line field editor.
const FIELD_EDITOR_MIN_HEIGHT: f32 = 32.;

/// Actions dispatched by the [`AuthSecretFtuxView`].
#[derive(Clone, Debug, PartialEq)]
pub enum AuthSecretFtuxAction {
    /// Mark the FTUX as completed without selecting or creating a secret.
    Skip,
    /// Switch the harness back to Oz, dismissing the FTUX.
    Cancel,
    /// Commit the current form state: if a creation type is selected, build
    /// the value and create the secret; otherwise, no-op (selecting an
    /// existing secret already commits via the selector itself).
    Continue,
}

/// Per-creation state held by the FTUX view while the user is filling in a
/// new secret's fields.
struct SecretCreationState {
    harness: Harness,
    secret_type_index: usize,
    /// True while the create-secret request is in flight; disables Continue.
    is_saving: bool,
}

/// First-time setup view shown when the user selects a non-Oz harness and the
/// per-harness FTUX has not been completed yet.
pub struct AuthSecretFtuxView {
    ambient_agent_model: ModelHandle<AmbientAgentViewModel>,
    ftux_dropdown: ViewHandle<AuthSecretFtuxDropdown>,
    /// Editor for the user-chosen name of the secret being created.
    name_editor: ViewHandle<EditorView>,
    /// One editor per `AuthSecretTypeField` of the currently-selected new type.
    /// Empty when no creation is in progress.
    field_editors: Vec<ViewHandle<EditorView>>,
    creation_state: Option<SecretCreationState>,
    /// When true, render the "Already logged in? Skip and continue" link.
    show_skip_link: bool,
    /// Mouse state handles for the bottom buttons. Owned by the view so they
    /// stay stable across renders.
    skip_mouse_state: MouseStateHandle,
    cancel_mouse_state: MouseStateHandle,
    continue_mouse_state: MouseStateHandle,
}

impl AuthSecretFtuxView {
    pub fn new(
        ambient_agent_model: ModelHandle<AmbientAgentViewModel>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let name_editor = make_single_line_editor(Some("API key name"), ctx);

        // Subscribe to Tab/ShiftTab on the name editor (index 0) for focus cycling.
        ctx.subscribe_to_view(&name_editor, |me, _, event, ctx| {
            me.handle_form_editor_nav(0, event, ctx);
        });

        // Build the FTUX-specific full-width dropdown internally.
        let ambient_agent_model_for_dropdown = ambient_agent_model.clone();
        let ftux_dropdown = ctx.add_typed_action_view(|ctx| {
            AuthSecretFtuxDropdown::new(ambient_agent_model_for_dropdown, ctx)
        });

        // When the dropdown opens, trigger a lazy fetch for auth secrets.
        ctx.subscribe_to_view(&ftux_dropdown, |_me, _, event, ctx| {
            if matches!(event, FtuxDropdownEvent::Opened) {
                let harness = _me.ambient_agent_model.as_ref(ctx).selected_harness();
                HarnessAvailabilityModel::handle(ctx).update(ctx, |model, ctx| {
                    model.ensure_auth_secrets_fetched(harness, ctx);
                });
            }
        });

        // When an existing secret is selected from the dropdown, set it on
        // the view model and mark FTUX as completed.
        ctx.subscribe_to_view(&ftux_dropdown, |me, _, event, ctx| {
            if let FtuxDropdownEvent::SecretSelected(name) = event {
                let harness = me.ambient_agent_model.as_ref(ctx).selected_harness();
                me.ambient_agent_model.update(ctx, |model, ctx| {
                    model.set_harness_auth_secret_name(Some(name.clone()), ctx);
                });
                CloudAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings.mark_harness_auth_ftux_completed(harness, ctx);
                });
                me.creation_state = None;
                me.field_editors.clear();
                ctx.notify();
            }
        });

        // When a new type is selected from the dropdown, enter creation mode.
        // When the display label is clicked, keep the current form fields
        // visible so the layout doesn't shift while the user browses options.
        // The form is only replaced when a different type is selected, or
        // cleared when an existing secret is picked.
        ctx.subscribe_to_view(&ftux_dropdown, |me, _, event, ctx| match event {
            FtuxDropdownEvent::NewTypeSelected {
                harness,
                type_index,
            } => {
                me.enter_creation_state(*harness, *type_index, ctx);
            }
            FtuxDropdownEvent::DisplayLabelCleared => {
                // Keep creation_state and field_editors intact so the form
                // stays visible while the dropdown is re-opened.
                ctx.notify();
            }
            FtuxDropdownEvent::SkipRequested => {
                me.handle_skip(ctx);
            }
            _ => {}
        });

        // When auth secret creation succeeds, finalize the FTUX (set the new
        // secret as selected on the view model and mark FTUX as completed).
        // When it fails, drop the saving flag so the user can retry.
        ctx.subscribe_to_model(
            &HarnessAvailabilityModel::handle(ctx),
            |me, _, event, ctx| match event {
                HarnessAvailabilityEvent::AuthSecretCreated { harness, name } => {
                    me.handle_secret_created(*harness, name.clone(), ctx);
                }
                HarnessAvailabilityEvent::AuthSecretCreationFailed { .. } => {
                    if let Some(state) = me.creation_state.as_mut() {
                        state.is_saving = false;
                    }
                    ctx.notify();
                }
                HarnessAvailabilityEvent::Changed
                | HarnessAvailabilityEvent::AuthSecretsLoaded { .. } => {}
            },
        );

        // When the harness changes, reset any in-progress creation state.
        ctx.subscribe_to_model(&ambient_agent_model, |me, _, event, ctx| {
            if matches!(event, AmbientAgentViewModelEvent::HarnessSelected) {
                me.clear_creation_state(ctx);
                ctx.notify();
            }
        });

        Self {
            ambient_agent_model,
            ftux_dropdown,
            name_editor,
            field_editors: Vec::new(),
            creation_state: None,
            show_skip_link: true,
            skip_mouse_state: MouseStateHandle::default(),
            cancel_mouse_state: MouseStateHandle::default(),
            continue_mouse_state: MouseStateHandle::default(),
        }
    }

    /// Returns true when the FTUX view has an active creation state (i.e. the
    /// user selected a "New {type}" and the creation form fields are showing).
    pub fn has_creation_state(&self) -> bool {
        self.creation_state.is_some()
    }

    /// Forwards an Up-arrow key press to the FTUX dropdown so the menu
    /// selection moves up. No-op when the dropdown is closed (e.g. while the
    /// creation form editors are focused).
    pub fn select_previous_in_dropdown(&self, ctx: &mut ViewContext<Self>) {
        self.ftux_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.select_previous_if_open(ctx);
        });
    }

    /// Focuses the FTUX dropdown's search editor so it receives keyboard
    /// events. Called by the parent Input view when it would normally focus
    /// the main input editor but the FTUX is active.
    pub fn focus_dropdown_editor(&self, ctx: &mut ViewContext<Self>) {
        self.ftux_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.focus_search_editor(ctx);
        });
    }

    /// Sets whether the "Skip and continue" link is rendered. Used by callers
    /// that re-enter the creation flow from the chip (after FTUX has already
    /// been completed once for this harness).
    #[allow(dead_code)]
    pub fn set_show_skip_link(&mut self, show: bool, ctx: &mut ViewContext<Self>) {
        if self.show_skip_link != show {
            self.show_skip_link = show;
            ctx.notify();
        }
    }

    /// Public entry point for entering creation mode from external callers
    /// (e.g. the top-row chip's "New {type}" sidecar in the non-FTUX path).
    pub fn enter_creation_state_public(
        &mut self,
        harness: Harness,
        type_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        self.enter_creation_state(harness, type_index, ctx);
    }

    /// Returns all form editors in order: name editor first, then field editors.
    /// Used for tab-cycling focus.
    fn all_form_editors(&self) -> Vec<&ViewHandle<EditorView>> {
        let mut editors = vec![&self.name_editor];
        editors.extend(self.field_editors.iter());
        editors
    }

    /// Focuses the form editor at the given index (0 = name editor, 1+ = field
    /// editors). Wraps around at both ends.
    fn focus_form_editor(&self, index: usize, ctx: &mut ViewContext<Self>) {
        let editors = self.all_form_editors();
        if let Some(editor) = editors.get(index % editors.len()) {
            ctx.focus(editor);
        }
    }

    /// Handles Tab / ShiftTab navigation events from a form editor at the given
    /// index in the `all_form_editors()` list. Cycles focus forward or backward.
    fn handle_form_editor_nav(
        &self,
        editor_index: usize,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        let count = self.all_form_editors().len();
        if count == 0 {
            return;
        }
        match event {
            EditorEvent::Navigate(key) => {
                match key {
                    NavigationKey::Tab => {
                        self.focus_form_editor((editor_index + 1) % count, ctx);
                    }
                    NavigationKey::ShiftTab => {
                        let prev = if editor_index == 0 {
                            count - 1
                        } else {
                            editor_index - 1
                        };
                        self.focus_form_editor(prev, ctx);
                    }
                    // Other navigation keys are not relevant for form cycling.
                    _ => {}
                }
            }
            _other => {}
        }
    }

    fn enter_creation_state(
        &mut self,
        harness: Harness,
        type_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let info = match auth_secret_types_for_harness(harness).get(type_index) {
            Some(info) => info,
            None => return,
        };
        // Build a single-line editor for each field of the chosen type and
        // subscribe to Tab/ShiftTab navigation events for focus cycling.
        let mut editors = Vec::with_capacity(info.fields.len());
        for (field_idx, field) in info.fields.iter().enumerate() {
            let editor = make_single_line_editor(Some(field.label), ctx);
            // field_idx + 1 because index 0 is the name editor.
            let editor_index = field_idx + 1;
            ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
                me.handle_form_editor_nav(editor_index, event, ctx);
            });
            editors.push(editor);
        }
        self.field_editors = editors;
        self.creation_state = Some(SecretCreationState {
            harness,
            secret_type_index: type_index,
            is_saving: false,
        });
        // Show the selected type name in the dropdown so the user sees what
        // was picked while the creation form fields are visible below.
        self.ftux_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_display_label(Some(info.display_name.to_string()), ctx);
        });
        // Auto-focus the first form field (secret name) so the user can start
        // typing immediately without clicking.
        ctx.focus(&self.name_editor);
        ctx.notify();
    }

    fn current_type_info(&self) -> Option<&'static AuthSecretTypeInfo> {
        let state = self.creation_state.as_ref()?;
        auth_secret_types_for_harness(state.harness).get(state.secret_type_index)
    }

    fn handle_skip(&mut self, ctx: &mut ViewContext<Self>) {
        let harness = self.ambient_agent_model.as_ref(ctx).selected_harness();
        CloudAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
            settings.mark_harness_auth_ftux_completed(harness, ctx);
        });
        // Clear any in-progress creation state and the dropdown's display
        // label directly — we must NOT call `set_display_label(None)` here
        // because that reopens the menu, which fights with the skip transition.
        self.creation_state = None;
        self.field_editors.clear();
        self.ftux_dropdown.update(ctx, |dropdown, _ctx| {
            dropdown.clear_display_label_quietly();
        });
        // Notify the parent Input view to re-render so it transitions from
        // the FTUX content back to the normal input container.
        ctx.notify();
    }

    fn handle_cancel(&mut self, ctx: &mut ViewContext<Self>) {
        let model = self.ambient_agent_model.clone();
        model.update(ctx, |model, ctx| {
            model.set_harness(Harness::Oz, ctx);
        });
    }

    fn handle_continue(&mut self, ctx: &mut ViewContext<Self>) {
        // No creation in progress: nothing to do. Selection of an existing
        // secret already commits via the selector itself.
        let Some(state) = self.creation_state.as_ref() else {
            return;
        };
        if state.is_saving {
            return;
        }
        let harness = state.harness;
        let type_index = state.secret_type_index;
        let Some(info) = auth_secret_types_for_harness(harness).get(type_index) else {
            return;
        };

        // Read the secret name from the dedicated name editor.
        let name = self.name_editor.as_ref(ctx).buffer_text(ctx);
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            // Surface a transient error via the harness availability event so
            // the existing toast subscription on `Input` shows a message.
            HarnessAvailabilityModel::handle(ctx).update(ctx, |_model, ctx| {
                ctx.emit(HarnessAvailabilityEvent::AuthSecretCreationFailed {
                    error: "Please enter a name for the secret.".to_string(),
                });
            });
            return;
        }
        let name = trimmed_name.to_string();

        // Read each field editor's buffer text in order.
        let field_values: Vec<String> = self
            .field_editors
            .iter()
            .map(|e| e.as_ref(ctx).buffer_text(ctx))
            .collect();

        let value = match build_managed_secret_value(info, &field_values) {
            Ok(v) => v,
            Err(err) => {
                let msg = err.to_string();
                HarnessAvailabilityModel::handle(ctx).update(ctx, |_model, ctx| {
                    ctx.emit(HarnessAvailabilityEvent::AuthSecretCreationFailed { error: msg });
                });
                return;
            }
        };

        // Mark saving and dispatch creation. Refresh on the AuthSecretCreated
        // event will finalize the flow.
        if let Some(state) = self.creation_state.as_mut() {
            state.is_saving = true;
        }
        ctx.notify();

        HarnessAvailabilityModel::handle(ctx).update(ctx, |model, ctx| {
            model.create_auth_secret(harness, name, value, ctx);
        });
    }

    /// Resets creation state and clears the dropdown display text so re-entering
    /// the FTUX starts with a fresh combobox.
    fn clear_creation_state(&mut self, ctx: &mut ViewContext<Self>) {
        if self.creation_state.is_some() {
            self.creation_state = None;
            self.field_editors.clear();
            self.ftux_dropdown.update(ctx, |dropdown, ctx| {
                dropdown.set_display_label(None, ctx);
            });
        }
    }

    fn handle_secret_created(
        &mut self,
        harness: Harness,
        name: String,
        ctx: &mut ViewContext<Self>,
    ) {
        // Set the newly-created secret as selected on the view model.
        let vm = self.ambient_agent_model.clone();
        vm.update(ctx, |model, ctx| {
            model.set_harness_auth_secret_name(Some(name), ctx);
        });
        // Mark FTUX as completed for this harness.
        CloudAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
            settings.mark_harness_auth_ftux_completed(harness, ctx);
        });
        self.clear_creation_state(ctx);
        ctx.notify();
    }

    fn render_description(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let harness = self.ambient_agent_model.as_ref(app).selected_harness();
        let display_name = harness_display::display_name(harness);
        let description = format!(
            "Please select an authentication secret or create a new one to use \
             {display_name} as a cloud agent."
        );
        Text::new_inline(
            description,
            appearance.ui_font_family(),
            DESCRIPTION_FONT_SIZE,
        )
        .with_color(theme.foreground().into())
        .soft_wrap(true)
        .finish()
    }

    fn render_field_label(&self, label: &str, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let color = internal_colors::text_sub(theme, theme.surface_1());
        Text::new_inline(
            label.to_string(),
            appearance.ui_font_family(),
            FIELD_LABEL_FONT_SIZE,
        )
        .with_color(color)
        .finish()
    }

    fn render_editor_container(
        &self,
        editor: &ViewHandle<EditorView>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let border_color = internal_colors::neutral_3(theme);
        let background = internal_colors::fg_overlay_1(theme);
        let editor_element = ChildView::new(editor).finish();
        ConstrainedBox::new(
            Container::new(editor_element)
                .with_padding_left(FIELD_EDITOR_PADDING)
                .with_padding_right(FIELD_EDITOR_PADDING)
                .with_padding_top(FIELD_EDITOR_PADDING / 2.)
                .with_padding_bottom(FIELD_EDITOR_PADDING / 2.)
                .with_background(background)
                .with_border(Border::all(1.).with_border_color(border_color))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CORNER_RADIUS)))
                .finish(),
        )
        .with_min_height(FIELD_EDITOR_MIN_HEIGHT)
        .finish()
    }

    fn render_creation_form(&self, app: &AppContext) -> Box<dyn Element> {
        let Some(info) = self.current_type_info() else {
            return Empty::new().finish();
        };
        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(FORM_FIELD_SPACING);

        // Name field at the top: required. Even though it's not in the
        // server's secret-value JSON, it identifies the saved secret.
        column.add_child(self.render_field_label("SECRET NAME", app));
        column.add_child(self.render_editor_container(&self.name_editor, app));

        for (idx, field) in info.fields.iter().enumerate() {
            let label = if field.optional {
                format!("{} (optional)", field.label)
            } else {
                field.label.to_string()
            };
            column.add_child(self.render_field_label(&label, app));
            if let Some(editor) = self.field_editors.get(idx) {
                column.add_child(self.render_editor_container(editor, app));
            }
        }
        column.finish()
    }

    fn render_skip_link(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let sub_color = internal_colors::text_sub(theme, theme.surface_1());
        let link_color = theme.accent().into_solid();
        let font_family = appearance.ui_font_family();

        let prefix = Text::new_inline(
            "Already set up authentication in your environment? ".to_string(),
            font_family,
            SKIP_FONT_SIZE,
        )
        .with_color(sub_color)
        .finish();

        let click_here = Text::new_inline(
            "Click here to skip".to_string(),
            font_family,
            SKIP_FONT_SIZE,
        )
        .with_color(link_color)
        .finish();

        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(prefix)
            .with_child(click_here)
            .finish();

        Hoverable::new(self.skip_mouse_state.clone(), move |_| row)
            .with_cursor(warpui::platform::Cursor::PointingHand)
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(AuthSecretFtuxAction::Skip);
            })
            .finish()
    }

    fn render_button(
        &self,
        label: &'static str,
        mouse_state: MouseStateHandle,
        background: Option<Fill>,
        action: AuthSecretFtuxAction,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let font_family = appearance.ui_font_family();
        let text_color = theme.foreground();
        Hoverable::new(mouse_state, move |_| {
            let inner = Container::new(
                Text::new_inline(label.to_string(), font_family, BUTTON_FONT_SIZE)
                    .with_style(Properties::default().weight(Weight::Semibold))
                    .with_color(text_color.into())
                    .finish(),
            )
            .with_padding_left(BUTTON_PADDING * 2.)
            .with_padding_right(BUTTON_PADDING * 2.)
            .with_padding_top(BUTTON_PADDING)
            .with_padding_bottom(BUTTON_PADDING);
            let inner = if let Some(background) = background {
                inner
                    .with_background(background)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CORNER_RADIUS)))
            } else {
                inner
            };
            inner.finish()
        })
        .with_cursor(warpui::platform::Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
        })
        .finish()
    }

    fn render_bottom_row(&self, app: &AppContext) -> Box<dyn Element> {
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

        if self.show_skip_link {
            row.add_child(self.render_skip_link(app));
        }
        row.add_child(Expanded::new(1., Empty::new().finish()).finish());

        // Cancel button: switches harness back to Oz.
        row.add_child(self.render_button(
            "Cancel",
            self.cancel_mouse_state.clone(),
            None,
            AuthSecretFtuxAction::Cancel,
            app,
        ));

        // Continue button: filled with accent color.
        let accent_fill = Appearance::as_ref(app).theme().accent();
        row.add_child(self.render_button(
            "Continue",
            self.continue_mouse_state.clone(),
            Some(accent_fill),
            AuthSecretFtuxAction::Continue,
            app,
        ));

        row.finish()
    }
}

/// Builds a single-line `EditorView` with the given placeholder text.
fn make_single_line_editor(
    placeholder: Option<&str>,
    ctx: &mut ViewContext<AuthSecretFtuxView>,
) -> ViewHandle<EditorView> {
    let placeholder = placeholder.map(str::to_owned);
    ctx.add_typed_action_view(move |ctx| {
        let appearance = Appearance::as_ref(ctx);
        let mut editor = EditorView::single_line(
            SingleLineEditorOptions {
                text: TextOptions::ui_text(Some(EDITOR_FONT_SIZE), appearance),
                select_all_on_focus: false,
                clear_selections_on_blur: true,
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::Always,
                propagate_and_no_op_escape_key: PropagateAndNoOpEscapeKey::PropagateFirst,
                ..Default::default()
            },
            ctx,
        );
        if let Some(placeholder) = placeholder {
            editor.set_placeholder_text(&placeholder, ctx);
        }
        editor
    })
}

impl Entity for AuthSecretFtuxView {
    type Event = ();
}

impl TypedActionView for AuthSecretFtuxView {
    type Action = AuthSecretFtuxAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            AuthSecretFtuxAction::Skip => self.handle_skip(ctx),
            AuthSecretFtuxAction::Cancel => self.handle_cancel(ctx),
            AuthSecretFtuxAction::Continue => self.handle_continue(ctx),
        }
    }
}

impl View for AuthSecretFtuxView {
    fn ui_name() -> &'static str {
        "AuthSecretFtuxView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(ROW_SPACING);

        // Description + dropdown.
        let mut content_section = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(CONTENT_SECTION_SPACING);
        content_section.add_child(self.render_description(app));
        content_section.add_child(ChildView::new(&self.ftux_dropdown).finish());
        column.add_child(content_section.finish());

        // If a creation type is selected, render the form below.
        if self.creation_state.is_some() {
            column.add_child(self.render_creation_form(app));
        }

        column.add_child(self.render_bottom_row(app));

        column.finish()
    }
}
