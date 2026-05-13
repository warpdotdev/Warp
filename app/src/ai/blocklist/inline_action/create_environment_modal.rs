use crate::{
    ai::cloud_environments::{owner_for_new_environment, AmbientAgentEnvironment},
    appearance::Appearance,
    server::{
        cloud_objects::update_manager::{
            ObjectOperation, OperationSuccessType, UpdateManager, UpdateManagerEvent,
        },
        ids::ClientId,
    },
    settings_view::update_environment_form::{
        AuthSource, EnvironmentFormCopy, EnvironmentFormInitArgs, GithubAuthRedirectTarget,
        UpdateEnvironmentForm, UpdateEnvironmentFormEvent,
    },
    view_components::DismissibleToast,
    workspace::ToastStack,
};
use pathfinder_color::ColorU;
use warpui::{
    elements::{
        Align, Border, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
        Dismiss, DropShadow, Element, Empty, Flex, ParentElement, Radius, Text,
    },
    fonts::{Properties, Weight},
    AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

const MODAL_WIDTH: f32 = 592.;
const MODAL_MAX_HEIGHT: f32 = 588.;
const MODAL_HORIZONTAL_PADDING: f32 = 24.;
const MODAL_VERTICAL_PADDING: f32 = 24.;
const MODAL_CONTENT_SPACING: f32 = 12.;
const MODAL_FORM_FIELD_SPACING: f32 = 10.;
const MODAL_DESCRIPTION_HEIGHT: f32 = 52.;

#[derive(Debug, Clone)]
pub enum CreateEnvironmentModalEvent {
    Cancelled,
    Created { environment_id: String },
}

#[derive(Debug, Clone)]
pub enum CreateEnvironmentModalAction {
    Cancel,
}

pub struct CreateEnvironmentModal {
    visible: bool,
    form: ViewHandle<UpdateEnvironmentForm>,
    pending_create_client_id: Option<ClientId>,
}

impl CreateEnvironmentModal {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let form = ctx.add_typed_action_view(|ctx| {
            let mut form = UpdateEnvironmentForm::new_with_deferred_github_repos_fetch(
                EnvironmentFormInitArgs::Create,
                ctx,
            );
            form.set_github_auth_redirect_target(GithubAuthRedirectTarget::FocusCloudMode);
            form.set_auth_source(AuthSource::CloudSetup);
            form.set_should_handle_escape_from_editor(true);
            form.set_show_header(false, ctx);
            form.set_copy(EnvironmentFormCopy::orchestration_modal(), ctx);
            form.set_show_footer_cancel_button(true, ctx);
            form.set_show_share_with_team_controls(false, ctx);
            form.set_field_max_width(MODAL_WIDTH - MODAL_HORIZONTAL_PADDING * 2., ctx);
            form.set_field_spacing(MODAL_FORM_FIELD_SPACING, ctx);
            form.set_description_height(MODAL_DESCRIPTION_HEIGHT, ctx);
            form.set_show_repo_helper_text(false, ctx);
            form
        });

        ctx.subscribe_to_view(&form, |me, _, event, ctx| match event {
            UpdateEnvironmentFormEvent::Created { environment, .. } => {
                me.create_environment(environment, ctx);
            }
            UpdateEnvironmentFormEvent::Cancelled => {
                me.cancel(ctx);
            }
            UpdateEnvironmentFormEvent::Updated { .. }
            | UpdateEnvironmentFormEvent::DeleteRequested { .. } => {}
        });

        ctx.subscribe_to_model(&UpdateManager::handle(ctx), |me, _, event, ctx| {
            me.handle_update_manager_event(event, ctx);
        });

        Self {
            visible: false,
            form,
            pending_create_client_id: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, ctx: &mut ViewContext<Self>) {
        self.visible = true;
        self.pending_create_client_id = None;
        self.form.update(ctx, |form, ctx| {
            form.set_mode(EnvironmentFormInitArgs::Create, ctx);
            form.fetch_github_repos(ctx);
            form.focus(ctx);
        });
        ctx.focus(&self.form);
        ctx.notify();
    }

    pub fn hide(&mut self, ctx: &mut ViewContext<Self>) {
        self.visible = false;
        self.pending_create_client_id = None;
        ctx.notify();
    }

    fn create_environment(
        &mut self,
        environment: &AmbientAgentEnvironment,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.pending_create_client_id.is_some() {
            return;
        }

        let Some(owner) = owner_for_new_environment(ctx) else {
            self.show_error_toast("Sign in to create an environment.".to_string(), ctx);
            return;
        };

        let client_id = ClientId::new();
        self.pending_create_client_id = Some(client_id);
        let environment = environment.clone();
        UpdateManager::handle(ctx).update(ctx, |update_manager, ctx| {
            update_manager.create_ambient_agent_environment(environment, client_id, owner, ctx);
        });
    }

    fn handle_update_manager_event(
        &mut self,
        event: &UpdateManagerEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        let UpdateManagerEvent::ObjectOperationComplete { result } = event else {
            return;
        };
        let ObjectOperation::Create { .. } = &result.operation else {
            return;
        };
        let Some(pending_client_id) = self.pending_create_client_id.as_ref() else {
            return;
        };
        if result.client_id.as_ref() != Some(pending_client_id) {
            return;
        }

        self.pending_create_client_id = None;
        if matches!(result.success_type, OperationSuccessType::Success) {
            let Some(server_id) = &result.server_id else {
                self.show_error_toast(
                    "Created environment, but couldn't select it.".to_string(),
                    ctx,
                );
                return;
            };

            let environment_id = server_id.uid();
            self.visible = false;
            ctx.emit(CreateEnvironmentModalEvent::Created { environment_id });
            ctx.notify();
        } else {
            self.show_error_toast("Failed to create environment.".to_string(), ctx);
            ctx.notify();
        }
    }

    fn cancel(&mut self, ctx: &mut ViewContext<Self>) {
        self.hide(ctx);
        ctx.emit(CreateEnvironmentModalEvent::Cancelled);
    }

    fn show_error_toast(&self, message: String, ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(DismissibleToast::error(message), window_id, ctx);
        });
    }

    fn render_dialog(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();

        let title = Text::new(
            "Create environment".to_string(),
            appearance.header_font_family(),
            20.,
        )
        .with_style(Properties::default().weight(Weight::Bold))
        .with_color(theme.active_ui_text_color().into())
        .finish();

        let description = Text::new(
            "Cloud agents require an environment that they’ll run in to get their task done. Create your first environment below for your orchestrated cloud agents.".to_string(),
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .soft_wrap(true)
        .with_color(theme.nonactive_ui_text_color().into())
        .finish();

        let content = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(MODAL_CONTENT_SPACING)
            .with_child(title)
            .with_child(description)
            .with_child(ChildView::new(&self.form).finish())
            .finish();

        let dialog = ConstrainedBox::new(
            Container::new(content)
                .with_padding_top(MODAL_VERTICAL_PADDING)
                .with_padding_bottom(MODAL_VERTICAL_PADDING)
                .with_padding_left(MODAL_HORIZONTAL_PADDING)
                .with_padding_right(MODAL_HORIZONTAL_PADDING)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                .with_border(Border::all(1.).with_border_fill(theme.outline()))
                .with_background(theme.surface_1())
                .with_drop_shadow(DropShadow {
                    color: ColorU::new(0, 0, 0, 77),
                    offset: pathfinder_geometry::vector::vec2f(0., 7.),
                    blur_radius: 7.,
                    spread_radius: 0.,
                })
                .finish(),
        )
        .with_width(MODAL_WIDTH)
        .with_max_height(MODAL_MAX_HEIGHT)
        .finish();

        Dismiss::new(dialog)
            .prevent_interaction_with_other_elements()
            .on_dismiss(|ctx, _app| {
                ctx.dispatch_typed_action(CreateEnvironmentModalAction::Cancel);
            })
            .finish()
    }
}

impl Entity for CreateEnvironmentModal {
    type Event = CreateEnvironmentModalEvent;
}

impl TypedActionView for CreateEnvironmentModal {
    type Action = CreateEnvironmentModalAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            CreateEnvironmentModalAction::Cancel => {
                self.cancel(ctx);
            }
        }
    }
}

impl View for CreateEnvironmentModal {
    fn ui_name() -> &'static str {
        "CreateEnvironmentModal"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        if !self.visible {
            return Empty::new().finish();
        }

        let appearance = Appearance::as_ref(app);
        Container::new(Align::new(self.render_dialog(appearance)).finish())
            .with_background(appearance.theme().dark_overlay())
            .finish()
    }
}
