//! Auth secret selector: an "options menu" (`ActionButton` + generic `Menu<A>`) shown
//! in the top row of the cloud mode V2 input that lets the user pick an auth secret
//! for the selected non-Oz harness.

use std::sync::Arc;

use pathfinder_geometry::vector::vec2f;
use warp_cli::agent::Harness;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::color::internal_colors;
use warp_core::ui::theme::Fill;
use warpui::elements::{
    Border, ChildAnchor, ChildView, OffsetPositioning, ParentAnchor, ParentElement as _,
    ParentOffsetBounds, Stack,
};
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::ai::harness_availability::{
    AuthSecretFetchState, HarnessAvailabilityEvent, HarnessAvailabilityModel,
};
use crate::menu::{Event as MenuEvent, Menu, MenuItem, MenuItemFields};
use crate::terminal::input::{MenuPositioning, MenuPositioningProvider};
use crate::terminal::view::ambient_agent::host_selector::NakedHeaderButtonTheme;
use crate::terminal::view::ambient_agent::AmbientAgentViewModel;
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{ActionButton, ButtonSize};

/// Font size for the header row (Figma: 12px).
const HEADER_FONT_SIZE: f32 = 12.;

/// Font size for secret item rows (Figma: 14px).
const ITEM_FONT_SIZE: f32 = 14.;

/// Horizontal padding inside menu rows (Figma: 16px).
const MENU_HORIZONTAL_PADDING: f32 = 16.;

/// Vertical padding on secret rows (Figma: 8px top & bottom).
const ITEM_VERTICAL_PADDING: f32 = 8.;

/// Width of the dropdown panel in logical pixels.
const MENU_WIDTH: f32 = 208.;

/// Tooltip string for the closed-state button.
const BUTTON_TOOLTIP: &str = "Auth secret";

/// Label rendered at the top of the dropdown.
const MENU_HEADER_LABEL: &str = "Auth secret";

/// Placeholder label shown when no secret is selected.
const NO_SECRET_LABEL: &str = "No secret";

/// Actions dispatched by the [`AuthSecretSelector`].
#[derive(Clone, Debug, PartialEq)]
pub enum AuthSecretSelectorAction {
    /// Toggle the visibility of the dropdown menu.
    ToggleMenu,
    /// The user picked an existing secret from the dropdown.
    SelectSecret(String),
}

/// Events emitted by the [`AuthSecretSelector`].
pub enum AuthSecretSelectorEvent {
    /// The dropdown visibility changed.
    MenuVisibilityChanged { open: bool },
}

/// A dropdown selector for choosing which auth secret to use for the active harness.
pub struct AuthSecretSelector {
    button: ViewHandle<ActionButton>,
    menu: ViewHandle<Menu<AuthSecretSelectorAction>>,
    is_menu_open: bool,
    menu_positioning_provider: Arc<dyn MenuPositioningProvider>,
    ambient_agent_model: ModelHandle<AmbientAgentViewModel>,
}

impl AuthSecretSelector {
    pub fn new(
        menu_positioning_provider: Arc<dyn MenuPositioningProvider>,
        ambient_agent_model: ModelHandle<AmbientAgentViewModel>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new(NO_SECRET_LABEL, NakedHeaderButtonTheme)
                .with_size(ButtonSize::AgentInputButton)
                .with_menu(true)
                .with_icon(Some(Icon::Key1))
                .with_tooltip(BUTTON_TOOLTIP)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(AuthSecretSelectorAction::ToggleMenu);
                })
        });

        let menu = ctx.add_typed_action_view(|_ctx| {
            Menu::new()
                .with_width(MENU_WIDTH)
                .with_drop_shadow()
                .prevent_interaction_with_other_elements()
        });

        ctx.subscribe_to_view(&menu, |me, _, event, ctx| match event {
            MenuEvent::Close { .. } => {
                me.set_menu_visibility(false, ctx);
            }
            MenuEvent::ItemSelected | MenuEvent::ItemHovered => {}
        });

        ctx.subscribe_to_model(&ambient_agent_model, |me, _, _, ctx| {
            me.refresh_button(ctx);
            me.refresh_menu(ctx);
        });

        ctx.subscribe_to_model(
            &HarnessAvailabilityModel::handle(ctx),
            |me, _, event, ctx| {
                if matches!(
                    event,
                    HarnessAvailabilityEvent::AuthSecretsLoaded { .. }
                        | HarnessAvailabilityEvent::AuthSecretCreated { .. }
                ) {
                    me.refresh_menu(ctx);
                    me.refresh_button(ctx);
                }
            },
        );

        ctx.subscribe_to_model(&Appearance::handle(ctx), |me, _, _, ctx| {
            me.refresh_menu(ctx);
        });

        let mut me = Self {
            button,
            menu,
            is_menu_open: false,
            menu_positioning_provider,
            ambient_agent_model,
        };
        me.refresh_button(ctx);
        me.refresh_menu(ctx);
        me
    }

    pub fn is_menu_open(&self) -> bool {
        self.is_menu_open
    }

    fn set_menu_visibility(&mut self, is_open: bool, ctx: &mut ViewContext<Self>) {
        if self.is_menu_open == is_open {
            return;
        }
        self.is_menu_open = is_open;
        if is_open {
            ctx.focus(&self.menu);
        }
        ctx.emit(AuthSecretSelectorEvent::MenuVisibilityChanged { open: is_open });
        ctx.notify();
    }

    fn refresh_button(&mut self, ctx: &mut ViewContext<Self>) {
        let label = self
            .ambient_agent_model
            .as_ref(ctx)
            .selected_harness_auth_secret_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| NO_SECRET_LABEL.to_string());
        self.button.update(ctx, |button, ctx| {
            button.set_label(label, ctx);
        });
    }

    fn refresh_menu(&mut self, ctx: &mut ViewContext<Self>) {
        let appearance = Appearance::as_ref(ctx);
        let theme = appearance.theme();
        let hover_background: Fill = internal_colors::neutral_4(theme).into();
        let header_text_color = theme.disabled_text_color(theme.surface_2()).into_solid();
        let border = Border::all(1.).with_border_fill(theme.outline());

        let harness = self.ambient_agent_model.as_ref(ctx).selected_harness();
        let availability = HarnessAvailabilityModel::as_ref(ctx);
        let items = build_menu_items(availability.auth_secrets_for(harness), hover_background, header_text_color);

        self.menu.update(ctx, |menu, ctx| {
            menu.set_border(Some(border));
            menu.set_items(items, ctx);
        });
    }

    fn menu_positioning(&self, app: &AppContext) -> OffsetPositioning {
        match self.menu_positioning_provider.menu_position(app) {
            MenuPositioning::BelowInputBox => OffsetPositioning::offset_from_parent(
                vec2f(0., 4.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::BottomLeft,
                ChildAnchor::TopLeft,
            ),
            MenuPositioning::AboveInputBox => OffsetPositioning::offset_from_parent(
                vec2f(0., -4.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::TopLeft,
                ChildAnchor::BottomLeft,
            ),
        }
    }
}

/// Builds the menu items from the fetched auth secrets.
fn build_menu_items(
    fetch_state: &AuthSecretFetchState,
    hover_background: Fill,
    header_text_color: pathfinder_color::ColorU,
) -> Vec<MenuItem<AuthSecretSelectorAction>> {
    let header = MenuItem::Header {
        fields: MenuItemFields::new(MENU_HEADER_LABEL)
            .with_font_size_override(HEADER_FONT_SIZE)
            .with_override_text_color(header_text_color)
            .with_padding_override(6., MENU_HORIZONTAL_PADDING)
            .with_no_interaction_on_hover(),
        clickable: false,
        right_side_fields: None,
    };

    let mut items = vec![header];

    match fetch_state {
        AuthSecretFetchState::Loaded(secrets) => {
            if secrets.is_empty() {
                items.push(MenuItem::Item(
                    MenuItemFields::new("No secrets available")
                        .with_font_size_override(ITEM_FONT_SIZE)
                        .with_padding_override(ITEM_VERTICAL_PADDING, MENU_HORIZONTAL_PADDING)
                        .with_disabled(true)
                        .with_override_text_color(header_text_color),
                ));
            } else {
                for secret in secrets {
                    let fields = MenuItemFields::new(secret.name.clone())
                        .with_font_size_override(ITEM_FONT_SIZE)
                        .with_padding_override(ITEM_VERTICAL_PADDING, MENU_HORIZONTAL_PADDING)
                        .with_override_hover_background_color(hover_background)
                        .with_on_select_action(AuthSecretSelectorAction::SelectSecret(
                            secret.name.clone(),
                        ));
                    items.push(MenuItem::Item(fields));
                }
            }
        }
        AuthSecretFetchState::Loading => {
            items.push(MenuItem::Item(
                MenuItemFields::new("Loading...")
                    .with_font_size_override(ITEM_FONT_SIZE)
                    .with_padding_override(ITEM_VERTICAL_PADDING, MENU_HORIZONTAL_PADDING)
                    .with_disabled(true)
                    .with_override_text_color(header_text_color),
            ));
        }
        AuthSecretFetchState::NotFetched | AuthSecretFetchState::Failed(_) => {
            items.push(MenuItem::Item(
                MenuItemFields::new("Unable to load secrets")
                    .with_font_size_override(ITEM_FONT_SIZE)
                    .with_padding_override(ITEM_VERTICAL_PADDING, MENU_HORIZONTAL_PADDING)
                    .with_disabled(true)
                    .with_override_text_color(header_text_color),
            ));
        }
    }

    items
}

impl Entity for AuthSecretSelector {
    type Event = AuthSecretSelectorEvent;
}

impl TypedActionView for AuthSecretSelector {
    type Action = AuthSecretSelectorAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            AuthSecretSelectorAction::ToggleMenu => {
                // Trigger a lazy fetch when the menu is about to open.
                let harness = self.ambient_agent_model.as_ref(ctx).selected_harness();
                HarnessAvailabilityModel::handle(ctx).update(ctx, |model, ctx| {
                    model.ensure_auth_secrets_fetched(harness, ctx);
                });
                let new_state = !self.is_menu_open;
                self.set_menu_visibility(new_state, ctx);
            }
            AuthSecretSelectorAction::SelectSecret(name) => {
                let name = name.clone();
                self.ambient_agent_model.update(ctx, |model, ctx| {
                    model.set_harness_auth_secret_name(Some(name), ctx);
                });
                self.set_menu_visibility(false, ctx);
            }
        }
    }
}

impl View for AuthSecretSelector {
    fn ui_name() -> &'static str {
        "AuthSecretSelector"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let mut stack = Stack::new();
        stack.add_child(ChildView::new(&self.button).finish());

        if self.is_menu_open {
            let positioning = self.menu_positioning(app);
            stack.add_positioned_overlay_child(ChildView::new(&self.menu).finish(), positioning);
        }

        stack.finish()
    }
}
