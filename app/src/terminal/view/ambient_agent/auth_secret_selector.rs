//! Auth secret selector: an `ActionButton` + dropdown that lets the user pick or create
//! an auth secret for the selected non-Oz harness.
//!
//! The dropdown shows existing secrets plus a "New" parent item. Hovering or selecting
//! the "New" item opens a sidecar `Menu` listing the supported secret types for the
//! current harness. Picking a type emits `NewTypeSelected` so a parent view (the FTUX
//! view) can swap into creation mode.
//!
//! This view is shared between two render sites:
//!  - As the top-row chip when the FTUX has already been completed for the harness.
//!  - As the dropdown embedded in the FTUX content, when the FTUX is active.

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
use warpui::fonts::Properties;
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::ai::auth_secret_types::auth_secret_types_for_harness;
use crate::ai::harness_availability::{
    AuthSecretFetchState, HarnessAvailabilityEvent, HarnessAvailabilityModel,
};
use crate::menu::{Event as MenuEvent, Menu, MenuItem, MenuItemFields};
use crate::terminal::input::{MenuPositioning, MenuPositioningProvider};
use crate::terminal::view::ambient_agent::host_selector::NakedHeaderButtonTheme;
use crate::terminal::view::ambient_agent::{AmbientAgentViewModel, AmbientAgentViewModelEvent};
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

/// Width of the new-type sidecar in logical pixels.
const SIDECAR_WIDTH: f32 = 220.;

/// Horizontal gap between the main menu and the sidecar.
const SIDECAR_HORIZONTAL_GAP: f32 = 4.;

/// Tooltip string for the closed-state button.
const BUTTON_TOOLTIP: &str = "Auth secret";

/// Label rendered at the top of the main dropdown.
const MENU_HEADER_LABEL: &str = "Auth secret";

/// Label rendered at the top of the sidecar.
const SIDECAR_HEADER_LABEL: &str = "Choose a type";

/// Placeholder label shown when no secret is selected.
const NO_SECRET_LABEL: &str = "No secret";

/// Label of the "New" parent item.
const NEW_ITEM_LABEL: &str = "New";

/// Save-position id used to anchor the sidecar to the main menu's container.
const MAIN_MENU_SAVE_POSITION_ID: &str = "auth_secret_selector_main_menu";

/// Actions dispatched by the [`AuthSecretSelector`].
#[derive(Clone, Debug, PartialEq)]
pub enum AuthSecretSelectorAction {
    /// Toggle the visibility of the dropdown menu.
    ToggleMenu,
    /// The user picked an existing secret from the dropdown.
    SelectSecret(String),
    /// The user cleared the selected secret (picked "No secret").
    ClearSecret,
    /// The user clicked the "New" parent item; opens the sidecar.
    OpenNewTypeSidecar,
    /// The user picked one of the new-secret types from the sidecar.
    SelectNewType(usize),
}

/// Events emitted by the [`AuthSecretSelector`].
pub enum AuthSecretSelectorEvent {
    /// The dropdown visibility changed.
    MenuVisibilityChanged { open: bool },
    /// The user picked "New {type}" from the sidecar; the parent view (FTUX)
    /// should swap into creation mode for the given type.
    NewTypeSelected { harness: Harness, type_index: usize },
}

/// A dropdown selector for choosing or creating an auth secret for the active harness.
pub struct AuthSecretSelector {
    button: ViewHandle<ActionButton>,
    menu: ViewHandle<Menu<AuthSecretSelectorAction>>,
    new_type_sidecar: ViewHandle<Menu<AuthSecretSelectorAction>>,
    is_menu_open: bool,
    is_new_type_sidecar_open: bool,
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
                .with_icon(Icon::Key)
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

        // Subscribe to the main menu so we can:
        // - Close everything on `Close`.
        // - Track which row is hovered so the sidecar can open/close as the
        //   user moves the mouse over the "New" parent.
        ctx.subscribe_to_view(&menu, |me, _, event, ctx| match event {
            MenuEvent::Close { .. } => {
                me.set_menu_visibility(false, ctx);
            }
            MenuEvent::ItemHovered => {
                me.update_sidecar_visibility_from_hover(ctx);
            }
            MenuEvent::ItemSelected => {}
        });

        // Sidecar menu shown next to the main dropdown when "New" is hovered.
        let new_type_sidecar = ctx.add_typed_action_view(|_ctx| {
            Menu::new()
                .with_width(SIDECAR_WIDTH)
                .with_drop_shadow()
                .prevent_interaction_with_other_elements()
        });

        ctx.subscribe_to_view(&new_type_sidecar, |me, _, event, ctx| match event {
            MenuEvent::Close { .. } => {
                me.set_new_type_sidecar_open(false, ctx);
            }
            MenuEvent::ItemSelected | MenuEvent::ItemHovered => {}
        });

        ctx.subscribe_to_model(&ambient_agent_model, |me, _, event, ctx| match event {
            AmbientAgentViewModelEvent::HarnessSelected => {
                // The harness changed; rebuild both menus and refresh the chip
                // label so the dropdown reflects the new harness's secrets.
                me.refresh_button(ctx);
                me.refresh_menu(ctx);
                me.refresh_sidecar(ctx);
            }
            AmbientAgentViewModelEvent::AuthSecretSelected => {
                me.refresh_button(ctx);
                me.refresh_menu(ctx);
            }
            _ => {}
        });

        ctx.subscribe_to_model(
            &HarnessAvailabilityModel::handle(ctx),
            |me, _, event, ctx| match event {
                HarnessAvailabilityEvent::AuthSecretsLoaded { .. }
                | HarnessAvailabilityEvent::AuthSecretCreated { .. } => {
                    me.refresh_menu(ctx);
                    me.refresh_button(ctx);
                }
                HarnessAvailabilityEvent::Changed
                | HarnessAvailabilityEvent::AuthSecretCreationFailed { .. } => {}
            },
        );

        ctx.subscribe_to_model(&Appearance::handle(ctx), |me, _, _, ctx| {
            me.refresh_menu(ctx);
            me.refresh_sidecar(ctx);
        });

        let mut me = Self {
            button,
            menu,
            new_type_sidecar,
            is_menu_open: false,
            is_new_type_sidecar_open: false,
            menu_positioning_provider,
            ambient_agent_model,
        };
        me.refresh_button(ctx);
        me.refresh_menu(ctx);
        me.refresh_sidecar(ctx);
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
            // Trigger a lazy fetch when the menu opens so the dropdown
            // populates in the background.
            let harness = self.ambient_agent_model.as_ref(ctx).selected_harness();
            HarnessAvailabilityModel::handle(ctx).update(ctx, |model, ctx| {
                model.ensure_auth_secrets_fetched(harness, ctx);
            });
            ctx.focus(&self.menu);
        } else {
            // Closing the main menu also dismisses the sidecar.
            self.set_new_type_sidecar_open(false, ctx);
        }
        ctx.emit(AuthSecretSelectorEvent::MenuVisibilityChanged { open: is_open });
        ctx.notify();
    }

    fn set_new_type_sidecar_open(&mut self, is_open: bool, ctx: &mut ViewContext<Self>) {
        if self.is_new_type_sidecar_open == is_open {
            return;
        }
        self.is_new_type_sidecar_open = is_open;
        if !is_open {
            // Clear the visual highlight on the parent "New" item.
            self.menu.update(ctx, |menu, _ctx| {
                menu.set_safe_zone_target(None);
                menu.set_submenu_being_shown_for_item_index(None);
            });
        } else {
            self.refresh_sidecar(ctx);
        }
        ctx.notify();
    }

    /// Reads the main menu's hovered row index and decides whether to open or
    /// close the sidecar based on whether the hovered row is the "New" item.
    /// Mirrors the worktree-config sidecar pattern in `workspace/view.rs`.
    fn update_sidecar_visibility_from_hover(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.is_menu_open {
            return;
        }
        let hovered_index = self.menu.read(ctx, |menu, _| menu.hovered_index());
        let Some(hovered_index) = hovered_index else {
            return;
        };
        let is_new_item = self.menu.read(ctx, |menu, _| {
            menu.items()
                .get(hovered_index)
                .map(|item| {
                    matches!(item,
                    MenuItem::Item(fields) if fields.label() == NEW_ITEM_LABEL)
                })
                .unwrap_or(false)
        });
        if is_new_item {
            // Highlight the "New" parent and open the sidecar.
            self.menu.update(ctx, |menu, _ctx| {
                menu.set_submenu_being_shown_for_item_index(Some(hovered_index));
            });
            self.set_new_type_sidecar_open(true, ctx);
        } else {
            // Mouse moved off "New"; close the sidecar.
            self.set_new_type_sidecar_open(false, ctx);
        }
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
        let items = build_main_menu_items(
            availability.auth_secrets_for(harness),
            hover_background,
            header_text_color,
        );

        self.menu.update(ctx, |menu, ctx| {
            menu.set_border(Some(border));
            menu.set_items(items, ctx);
        });
    }

    fn refresh_sidecar(&mut self, ctx: &mut ViewContext<Self>) {
        let appearance = Appearance::as_ref(ctx);
        let theme = appearance.theme();
        let hover_background: Fill = internal_colors::neutral_4(theme).into();
        let header_text_color = theme.disabled_text_color(theme.surface_2()).into_solid();
        let border = Border::all(1.).with_border_fill(theme.outline());

        let harness = self.ambient_agent_model.as_ref(ctx).selected_harness();
        let items = build_sidecar_items(harness, hover_background, header_text_color);
        self.new_type_sidecar.update(ctx, |menu, ctx| {
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

    /// Positions the sidecar to the right of the main menu by default. When
    /// the sidecar would overflow the right edge of the window, flip it to the
    /// left so it stays on-screen.
    fn sidecar_positioning(&self, app: &AppContext) -> OffsetPositioning {
        let flip_left = self.should_render_sidecar_left(app);
        let offset_x = if flip_left {
            -(SIDECAR_WIDTH + SIDECAR_HORIZONTAL_GAP)
        } else {
            MENU_WIDTH + SIDECAR_HORIZONTAL_GAP
        };
        OffsetPositioning::offset_from_save_position_element(
            MAIN_MENU_SAVE_POSITION_ID.to_string(),
            vec2f(offset_x, 0.),
            warpui::elements::PositionedElementOffsetBounds::WindowByPosition,
            warpui::elements::PositionedElementAnchor::BottomLeft,
            ChildAnchor::BottomLeft,
        )
    }

    fn should_render_sidecar_left(&self, app: &AppContext) -> bool {
        let Some(window_id) = app.windows().active_window() else {
            return false;
        };
        let Some(window) = app.windows().platform_window(window_id) else {
            return false;
        };
        let Some(menu_rect) =
            app.element_position_by_id_at_last_frame(window_id, MAIN_MENU_SAVE_POSITION_ID)
        else {
            return false;
        };
        let gap = SIDECAR_HORIZONTAL_GAP;
        let would_overflow_right = menu_rect.max_x() + gap + SIDECAR_WIDTH >= window.size().x();
        let would_overflow_left = menu_rect.min_x() - gap - SIDECAR_WIDTH < 0.;
        match (would_overflow_left, would_overflow_right) {
            (true, false) => false,
            (false, true) => true,
            _ => false,
        }
    }
}

/// Builds the main menu items: header + secrets list (or empty/loading/failed
/// state) + a non-disabled "New" item that opens the sidecar.
fn build_main_menu_items(
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

    // Always show a "No secret" option so the user can clear the selection.
    items.push(MenuItem::Item(
        MenuItemFields::new("No secret")
            .with_font_size_override(ITEM_FONT_SIZE)
            .with_padding_override(ITEM_VERTICAL_PADDING, MENU_HORIZONTAL_PADDING)
            .with_override_hover_background_color(hover_background)
            .with_on_select_action(AuthSecretSelectorAction::ClearSecret),
    ));

    match fetch_state {
        AuthSecretFetchState::Loaded(secrets) => {
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
        AuthSecretFetchState::Loading => {
            items.push(MenuItem::Item(
                MenuItemFields::new("Loading…")
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

    // Always show the "New" parent item at the bottom, even when listing fails
    // or returns no results, so the user can always create a secret.
    items.push(MenuItem::Item(
        MenuItemFields::new(NEW_ITEM_LABEL)
            .with_font_size_override(ITEM_FONT_SIZE)
            .with_padding_override(ITEM_VERTICAL_PADDING, MENU_HORIZONTAL_PADDING)
            .with_override_hover_background_color(hover_background)
            .with_icon(Icon::Plus)
            // Render the chevron via right_side_label since the actual
            // `has_submenu` flag is reserved for the deprecated MenuItem::Submenu
            // codepath; we manage the sidecar ourselves.
            .with_right_side_label("›", Properties::default())
            .with_on_select_action(AuthSecretSelectorAction::OpenNewTypeSidecar),
    ));

    items
}

/// Builds the sidecar items: header + one row per secret type for the harness.
fn build_sidecar_items(
    harness: Harness,
    hover_background: Fill,
    header_text_color: pathfinder_color::ColorU,
) -> Vec<MenuItem<AuthSecretSelectorAction>> {
    let header = MenuItem::Header {
        fields: MenuItemFields::new(SIDECAR_HEADER_LABEL)
            .with_font_size_override(HEADER_FONT_SIZE)
            .with_override_text_color(header_text_color)
            .with_padding_override(6., MENU_HORIZONTAL_PADDING)
            .with_no_interaction_on_hover(),
        clickable: false,
        right_side_fields: None,
    };

    let mut items = vec![header];

    for (index, info) in auth_secret_types_for_harness(harness).iter().enumerate() {
        items.push(MenuItem::Item(
            MenuItemFields::new(info.display_name)
                .with_font_size_override(ITEM_FONT_SIZE)
                .with_padding_override(ITEM_VERTICAL_PADDING, MENU_HORIZONTAL_PADDING)
                .with_override_hover_background_color(hover_background)
                .with_icon(Icon::Key)
                .with_on_select_action(AuthSecretSelectorAction::SelectNewType(index)),
        ));
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
                let new_state = !self.is_menu_open;
                self.set_menu_visibility(new_state, ctx);
            }
            AuthSecretSelectorAction::SelectSecret(name) => {
                let name = name.clone();
                let harness = self.ambient_agent_model.as_ref(ctx).selected_harness();
                self.ambient_agent_model.update(ctx, |model, ctx| {
                    model.set_harness_auth_secret_name(Some(name), ctx);
                });
                // Mark the FTUX as completed for this harness so the user
                // goes straight to the input next time.
                crate::ai::cloud_agent_settings::CloudAgentSettings::handle(ctx).update(
                    ctx,
                    |settings, ctx| {
                        settings.mark_harness_auth_ftux_completed(harness, ctx);
                    },
                );
                self.set_menu_visibility(false, ctx);
            }
            AuthSecretSelectorAction::ClearSecret => {
                self.ambient_agent_model.update(ctx, |model, ctx| {
                    model.set_harness_auth_secret_name(None, ctx);
                });
                self.set_menu_visibility(false, ctx);
            }
            AuthSecretSelectorAction::OpenNewTypeSidecar => {
                // Clicking the "New" parent toggles the sidecar; hovering
                // already manages it via `update_sidecar_visibility_from_hover`,
                // so this is mainly relevant for keyboard / non-mouse activation.
                self.set_new_type_sidecar_open(true, ctx);
            }
            AuthSecretSelectorAction::SelectNewType(type_index) => {
                let type_index = *type_index;
                let harness = self.ambient_agent_model.as_ref(ctx).selected_harness();
                self.set_new_type_sidecar_open(false, ctx);
                self.set_menu_visibility(false, ctx);
                ctx.emit(AuthSecretSelectorEvent::NewTypeSelected {
                    harness,
                    type_index,
                });
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
            // Wrap the main menu in a SavePosition so the sidecar can anchor
            // to its bounds via `offset_from_save_position_element`.
            let main_menu = warpui::elements::SavePosition::new(
                ChildView::new(&self.menu).finish(),
                MAIN_MENU_SAVE_POSITION_ID,
            )
            .finish();
            stack.add_positioned_overlay_child(main_menu, self.menu_positioning(app));

            if self.is_new_type_sidecar_open {
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.new_type_sidecar).finish(),
                    self.sidecar_positioning(app),
                );
            }
        }

        stack.finish()
    }
}
