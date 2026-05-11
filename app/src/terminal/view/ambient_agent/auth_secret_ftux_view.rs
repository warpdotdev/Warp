use crate::view_components::DismissibleToast;
use crate::workspace::ToastStack;
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

use settings::Setting as _;

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

const DESCRIPTION_FONT_SIZE: f32 = 14.;

const FIELD_LABEL_FONT_SIZE: f32 = 10.;

const BUTTON_FONT_SIZE: f32 = 14.;

const EDITOR_FONT_SIZE: f32 = 14.;

const ROW_SPACING: f32 = 24.;

const CONTENT_SECTION_SPACING: f32 = 12.;

const FORM_FIELD_SPACING: f32 = 8.;

const BUTTON_PADDING: f32 = 8.;

const FIELD_EDITOR_PADDING: f32 = 8.;

const CORNER_RADIUS: f32 = 4.;

const FIELD_EDITOR_MIN_HEIGHT: f32 = 32.;

#[derive(Clone, Debug, PartialEq)]
pub enum AuthSecretFtuxAction {
    Skip,
    Cancel,
    Continue,
}

struct SecretCreationState {
    harness: Harness,
    secret_type_index: usize,
    is_saving: bool,
}

pub struct AuthSecretFtuxView {
    ambient_agent_model: ModelHandle<AmbientAgentViewModel>,
    ftux_dropdown: ViewHandle<AuthSecretFtuxDropdown>,
    name_editor: ViewHandle<EditorView>,
    field_editors: Vec<ViewHandle<EditorView>>,
    creation_state: Option<SecretCreationState>,
    cancel_mouse_state: MouseStateHandle,
    continue_mouse_state: MouseStateHandle,
}

impl AuthSecretFtuxView {
    pub fn new(
        ambient_agent_model: ModelHandle<AmbientAgentViewModel>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let name_editor = make_single_line_editor(Some("NICKNAME"), false, ctx);

        ctx.subscribe_to_view(&name_editor, |me, _, event, ctx| {
            me.handle_form_editor_nav(0, event, ctx);
        });

        let ambient_agent_model_for_dropdown = ambient_agent_model.clone();
        let ftux_dropdown = ctx.add_typed_action_view(|ctx| {
            AuthSecretFtuxDropdown::new(ambient_agent_model_for_dropdown, ctx)
        });

        ctx.subscribe_to_view(&ftux_dropdown, |_me, _, event, ctx| {
            if matches!(event, FtuxDropdownEvent::Opened) {
                let harness = _me.ambient_agent_model.as_ref(ctx).selected_harness();
                HarnessAvailabilityModel::handle(ctx).update(ctx, |model, ctx| {
                    model.ensure_auth_secrets_fetched(harness, ctx);
                });
            }
        });

        ctx.subscribe_to_view(&ftux_dropdown, |me, _, event, ctx| {
            if let FtuxDropdownEvent::SecretSelected(name) = event {
                let harness = me.ambient_agent_model.as_ref(ctx).selected_harness();
                me.ambient_agent_model.update(ctx, |model, ctx| {
                    model.set_harness_auth_secret_name(Some(name.clone()), ctx);
                });
                CloudAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings.mark_harness_auth_ftux_completed(harness, ctx);
                    // Persist the selection so the secret is sticky across sessions.
                    let mut map = settings.last_selected_auth_secret.value().clone();
                    map.insert(harness.config_name().to_string(), name.clone());
                    let _ = settings.last_selected_auth_secret.set_value(map, ctx);
                });
                me.clear_all_editor_buffers(ctx);
                me.creation_state = None;
                me.field_editors.clear();
                ctx.notify();
            }
        });

        ctx.subscribe_to_view(&ftux_dropdown, |me, _, event, ctx| match event {
            FtuxDropdownEvent::NewTypeSelected {
                harness,
                type_index,
            } => {
                me.enter_creation_state(*harness, *type_index, ctx);
            }
            FtuxDropdownEvent::DisplayLabelCleared => {
                ctx.notify();
            }
            FtuxDropdownEvent::SkipRequested => {
                me.handle_skip(ctx);
            }
            _ => {}
        });

        ctx.subscribe_to_model(
            &HarnessAvailabilityModel::handle(ctx),
            |me, _, event, ctx| match event {
                HarnessAvailabilityEvent::AuthSecretCreated { harness, name } => {
                    if me.creation_state.is_some() {
                        me.handle_secret_created(*harness, name.clone(), ctx);
                    }
                }
                HarnessAvailabilityEvent::AuthSecretCreationFailed { error } => {
                    if let Some(state) = me.creation_state.as_mut() {
                        state.is_saving = false;
                        let window_id = ctx.window_id();
                        let message = format!("Failed to save API key: {error}");
                        ToastStack::handle(ctx).update(ctx, |ts, ctx| {
                            ts.add_ephemeral_toast(
                                DismissibleToast::error(message),
                                window_id,
                                ctx,
                            );
                        });
                        ctx.notify();
                    }
                }
                HarnessAvailabilityEvent::Changed | HarnessAvailabilityEvent::AuthSecretsLoaded => {
                }
            },
        );

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
            cancel_mouse_state: MouseStateHandle::default(),
            continue_mouse_state: MouseStateHandle::default(),
        }
    }

    pub fn has_creation_state(&self) -> bool {
        self.creation_state.is_some()
    }

    pub fn select_previous_in_dropdown(&self, ctx: &mut ViewContext<Self>) {
        self.ftux_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.select_previous_if_open(ctx);
        });
    }

    pub fn focus_dropdown_editor(&self, ctx: &mut ViewContext<Self>) {
        self.ftux_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.focus_search_editor(ctx);
        });
    }

    pub fn enter_creation_state_public(
        &mut self,
        harness: Harness,
        type_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        self.enter_creation_state(harness, type_index, ctx);
    }

    fn all_form_editors(&self) -> Vec<&ViewHandle<EditorView>> {
        let mut editors = vec![&self.name_editor];
        editors.extend(self.field_editors.iter());
        editors
    }

    fn focus_form_editor(&self, index: usize, ctx: &mut ViewContext<Self>) {
        let editors = self.all_form_editors();
        if let Some(editor) = editors.get(index % editors.len()) {
            ctx.focus(editor);
        }
    }

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
            EditorEvent::Navigate(key) => match key {
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
                _ => {}
            },
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
        let mut editors = Vec::with_capacity(info.fields.len());
        for (field_idx, field) in info.fields.iter().enumerate() {
            let editor = make_single_line_editor(Some(field.label), true, ctx);
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
        self.ftux_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_display_label(Some(info.display_name.to_string()), ctx);
        });
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
        self.clear_all_editor_buffers(ctx);
        self.creation_state = None;
        self.field_editors.clear();
        self.ftux_dropdown.update(ctx, |dropdown, _ctx| {
            dropdown.clear_display_label_quietly();
        });
        ctx.notify();
    }

    fn handle_cancel(&mut self, ctx: &mut ViewContext<Self>) {
        let model = self.ambient_agent_model.clone();
        model.update(ctx, |model, ctx| {
            model.set_harness(Harness::Oz, ctx);
        });
    }

    fn handle_continue(&mut self, ctx: &mut ViewContext<Self>) {
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

        let name = self.name_editor.as_ref(ctx).buffer_text(ctx);
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            HarnessAvailabilityModel::handle(ctx).update(ctx, |_model, ctx| {
                ctx.emit(HarnessAvailabilityEvent::AuthSecretCreationFailed {
                    error: "Please enter a name for the secret.".to_string(),
                });
            });
            return;
        }
        let name = trimmed_name.to_string();

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

        if let Some(state) = self.creation_state.as_mut() {
            state.is_saving = true;
        }
        ctx.notify();

        HarnessAvailabilityModel::handle(ctx).update(ctx, |model, ctx| {
            model.create_auth_secret(harness, name, value, ctx);
        });
    }

    fn clear_creation_state(&mut self, ctx: &mut ViewContext<Self>) {
        if self.creation_state.is_some() {
            self.creation_state = None;
            self.clear_all_editor_buffers(ctx);
            self.field_editors.clear();
            self.ftux_dropdown.update(ctx, |dropdown, ctx| {
                dropdown.set_display_label(None, ctx);
            });
        }
    }

    fn clear_all_editor_buffers(&self, ctx: &mut ViewContext<Self>) {
        self.name_editor.update(ctx, |editor, ctx| {
            editor.system_clear_buffer(true, ctx);
        });
        for editor in &self.field_editors {
            editor.update(ctx, |editor, ctx| {
                editor.system_clear_buffer(true, ctx);
            });
        }
    }

    fn handle_secret_created(
        &mut self,
        harness: Harness,
        name: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let window_id = ctx.window_id();
        let message = format!("API key '{name}' saved.");
        ToastStack::handle(ctx).update(ctx, |ts, ctx| {
            ts.add_ephemeral_toast(DismissibleToast::default(message), window_id, ctx);
        });
        let vm = self.ambient_agent_model.clone();
        vm.update(ctx, |model, ctx| {
            model.set_harness_auth_secret_name(Some(name.clone()), ctx);
        });
        CloudAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
            settings.mark_harness_auth_ftux_completed(harness, ctx);
            // Persist the newly created secret so it is sticky across sessions.
            let mut map = settings.last_selected_auth_secret.value().clone();
            map.insert(harness.config_name().to_string(), name);
            let _ = settings.last_selected_auth_secret.set_value(map, ctx);
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
            "Please select an API key or create a new one to use \
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

        column.add_child(self.render_field_label("NICKNAME", app));
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
        row.add_child(Expanded::new(1., Empty::new().finish()).finish());

        row.add_child(self.render_button(
            "Cancel",
            self.cancel_mouse_state.clone(),
            None,
            AuthSecretFtuxAction::Cancel,
            app,
        ));

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

fn make_single_line_editor(
    placeholder: Option<&str>,
    is_password: bool,
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
                is_password,
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

        let mut content_section = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(CONTENT_SECTION_SPACING);
        content_section.add_child(self.render_description(app));
        content_section.add_child(ChildView::new(&self.ftux_dropdown).finish());
        column.add_child(content_section.finish());

        if self.creation_state.is_some() {
            column.add_child(self.render_creation_form(app));
        }

        column.add_child(self.render_bottom_row(app));

        column.finish()
    }
}
