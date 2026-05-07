use crate::ai::cloud_environments;
use crate::appearance::Appearance;
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::ids::{ClientId, SyncId};
use crate::settings_view::update_environment_form::{
    AuthSource, EnvironmentFormInitArgs, GithubAuthRedirectTarget, UpdateEnvironmentForm,
    UpdateEnvironmentFormEvent,
};
use crate::ui_components::dialog::{dialog_styles, Dialog};
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{ActionButton, SecondaryTheme};
use pathfinder_color::ColorU;
use warpui::elements::{
    Align, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
    CrossAxisAlignment, Dismiss, Element, Empty, Flex, MouseStateHandle, ParentElement,
    ScrollbarWidth,
};
use warpui::ui_components::components::UiComponent;
use warpui::{AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle};

use crate::ui_components::buttons::icon_button;

const DIALOG_WIDTH: f32 = 600.;
const FORM_MAX_HEIGHT: f32 = 520.;
const FORM_PADDING: f32 = 8.;

#[derive(Debug, Clone)]
pub(crate) enum HandoffEnvironmentCreationModalEvent {
    Created { env_id: SyncId },
    Cancelled,
}

#[derive(Debug, Clone)]
pub(crate) enum HandoffEnvironmentCreationModalAction {
    Cancel,
}

pub(crate) struct HandoffEnvironmentCreationModal {
    visible: bool,
    environment_form: ViewHandle<UpdateEnvironmentForm>,
    cancel_button: ViewHandle<ActionButton>,
    close_button_mouse_state: MouseStateHandle,
    scroll_state: ClippedScrollStateHandle,
}

impl HandoffEnvironmentCreationModal {
    pub(crate) fn new(ctx: &mut ViewContext<Self>) -> Self {
        let environment_form = ctx.add_typed_action_view(|ctx| {
            let mut form = UpdateEnvironmentForm::new(EnvironmentFormInitArgs::Create, ctx);
            form.set_github_auth_redirect_target(GithubAuthRedirectTarget::FocusCloudMode);
            form.set_show_header(false, ctx);
            form.set_should_handle_escape_from_editor(true);
            form.set_auth_source(AuthSource::CloudSetup);
            form
        });

        ctx.subscribe_to_view(&environment_form, |me, _, event, ctx| {
            me.handle_environment_form_event(event, ctx);
        });

        let cancel_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Cancel", SecondaryTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(HandoffEnvironmentCreationModalAction::Cancel);
            })
        });

        Self {
            visible: false,
            environment_form,
            cancel_button,
            close_button_mouse_state: MouseStateHandle::default(),
            scroll_state: ClippedScrollStateHandle::default(),
        }
    }

    pub(crate) fn show(&mut self, ctx: &mut ViewContext<Self>) {
        self.visible = true;
        self.scroll_state = ClippedScrollStateHandle::default();
        self.environment_form.update(ctx, |form, ctx| {
            form.set_mode(EnvironmentFormInitArgs::Create, ctx);
            form.focus(ctx);
        });
        ctx.notify();
    }

    fn hide(&mut self, ctx: &mut ViewContext<Self>) {
        self.visible = false;
        ctx.notify();
    }

    fn handle_environment_form_event(
        &mut self,
        event: &UpdateEnvironmentFormEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            UpdateEnvironmentFormEvent::Created {
                environment,
                share_with_team,
            } => {
                let owner = if *share_with_team {
                    cloud_environments::owner_for_new_environment(ctx)
                } else {
                    cloud_environments::owner_for_new_personal_environment(ctx)
                };

                let Some(owner) = owner else {
                    log::error!("Unable to create environment: not logged in");
                    self.hide(ctx);
                    ctx.emit(HandoffEnvironmentCreationModalEvent::Cancelled);
                    return;
                };

                let client_id = ClientId::default();
                UpdateManager::handle(ctx).update(ctx, |update_manager, ctx| {
                    update_manager.create_ambient_agent_environment(
                        environment.clone(),
                        client_id,
                        owner,
                        ctx,
                    );
                });

                let env_id = SyncId::ClientId(client_id);
                self.hide(ctx);
                ctx.emit(HandoffEnvironmentCreationModalEvent::Created { env_id });
            }
            UpdateEnvironmentFormEvent::Cancelled => {
                self.hide(ctx);
                ctx.emit(HandoffEnvironmentCreationModalEvent::Cancelled);
            }
            UpdateEnvironmentFormEvent::Updated { .. }
            | UpdateEnvironmentFormEvent::DeleteRequested { .. } => {}
        }
    }

    fn render_dialog(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let theme = appearance.theme();

        let close_button = icon_button(
            appearance,
            Icon::X,
            false,
            self.close_button_mouse_state.clone(),
        )
        .build()
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(HandoffEnvironmentCreationModalAction::Cancel);
        })
        .finish();

        let form_content = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(ChildView::new(&self.environment_form).finish())
            .finish();

        let scrollable_form = ClippedScrollable::vertical(
            self.scroll_state.clone(),
            form_content,
            ScrollbarWidth::Auto,
            theme.nonactive_ui_text_color().into(),
            theme.active_ui_text_color().into(),
            warpui::elements::Fill::None,
        )
        .finish();

        let constrained_form = ConstrainedBox::new(scrollable_form)
            .with_max_height(FORM_MAX_HEIGHT)
            .finish();

        let padded_form = warpui::elements::Container::new(constrained_form)
            .with_uniform_padding(FORM_PADDING)
            .finish();

        let dialog = Dialog::new(
            "Create environment".to_string(),
            None,
            dialog_styles(appearance),
        )
        .with_close_button(close_button)
        .with_child(padded_form)
        .with_separator()
        .with_bottom_row_child(ChildView::new(&self.cancel_button).finish())
        .with_width(DIALOG_WIDTH)
        .build();

        let dialog = Dismiss::new(dialog.finish())
            .prevent_interaction_with_other_elements()
            .on_dismiss(|ctx, _app| {
                ctx.dispatch_typed_action(HandoffEnvironmentCreationModalAction::Cancel);
            })
            .finish();

        warpui::elements::Container::new(Align::new(dialog).finish())
            .with_background_color(ColorU::new(0, 0, 0, 179))
            .with_corner_radius(app.windows().window_corner_radius())
            .finish()
    }
}

impl Entity for HandoffEnvironmentCreationModal {
    type Event = HandoffEnvironmentCreationModalEvent;
}

impl TypedActionView for HandoffEnvironmentCreationModal {
    type Action = HandoffEnvironmentCreationModalAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            HandoffEnvironmentCreationModalAction::Cancel => {
                self.hide(ctx);
                ctx.emit(HandoffEnvironmentCreationModalEvent::Cancelled);
            }
        }
    }
}

impl View for HandoffEnvironmentCreationModal {
    fn ui_name() -> &'static str {
        "HandoffEnvironmentCreationModal"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        if !self.visible {
            return Empty::new().finish();
        }

        let appearance = Appearance::as_ref(app);
        self.render_dialog(appearance, app)
    }
}
