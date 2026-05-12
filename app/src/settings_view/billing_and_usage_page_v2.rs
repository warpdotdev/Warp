use chrono::Local;
use itertools::Itertools;
use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use thousands::Separable;
use warp_core::{features::FeatureFlag, ui::appearance::Appearance};
use warp_graphql::billing::AddonCreditsOption;
use warpui::prelude::ChildView;
use warpui::{
    elements::{
        Align, Border, Clipped, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
        Expanded, Flex, FormattedTextElement, HighlightedHyperlink, MainAxisAlignment,
        MainAxisSize, MouseStateHandle, ParentElement, Radius, Shrinkable, Text, Wrap,
    },
    fonts::{Properties, Weight},
    ui_components::{
        button::{ButtonVariant, TextAndIcon, TextAndIconAlignment},
        components::{Coords, UiComponent, UiComponentStyles},
        switch::SwitchStateHandle,
    },
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, UpdateView, View,
    ViewContext, ViewHandle,
};

use settings::Setting;

use crate::{
    ai::{
        request_usage_model::{BonusGrant, BonusGrantScope, BonusGrantType},
        AIRequestUsageModel,
    },
    auth::{
        auth_state::AuthState, auth_view_modal::AuthViewVariant, AuthManager, AuthStateProvider,
    },
    modal::{Modal, ModalEvent, ModalViewState},
    pricing::PricingInfoModel,
    send_telemetry_from_ctx,
    server::{ids::ServerId, telemetry::TelemetryEvent},
    settings::ai::AISettings,
    ui_components::{
        blended_colors,
        buttons::icon_button,
        icons::Icon,
        tab_selector::{self, SettingsTab},
    },
    view_components::{
        action_button::{ActionButton, PrimaryTheme, SecondaryTheme},
        ToastFlavor,
    },
    workspaces::{
        update_manager::TeamUpdateManager,
        user_workspaces::{UserWorkspaces, UserWorkspacesEvent},
        workspace::{CustomerType, Workspace},
    },
    WorkspaceAction,
};

use super::{
    billing_and_usage::{
        overage_limit_modal::{SpendingLimitModal, SpendingLimitModalEvent},
        usage_history_entry::UsageHistoryEntry,
        usage_history_model::UsageHistoryModel,
    },
    billing_and_usage_page::{BillingAndUsagePageAction, BillingUsageTab},
    settings_page::{
        render_customer_type_badge, render_info_icon, AdditionalInfo, MatchData, SettingsPageMeta,
        SettingsPageViewHandle, PAGE_PADDING,
    },
    SettingsSection,
};

pub use super::billing_and_usage_page::BillingAndUsagePageEvent;

const ADDON_CREDITS_DESCRIPTION: &str = "Add-on credits are purchased in prepaid packages that roll over each billing cycle and expire after one year. The more you purchase, the better the per-credit rate. Once your base plan credits are used, add-on credits will be consumed.";
const ADDITIONAL_ADDON_CREDITS_DESCRIPTION_FOR_TEAM: &str =
    "Purchased add-on credits are shared across your team.";

const AUTO_RELOAD_DELINQUENT_WARNING_STRING: &str =
    "Restricted due to billing issue. Update your payment method to purchase add-on credits.";
const RESTRICTED_BILLING_USAGE_WARNING_STRING: &str =
    "Auto reload is disabled due to recent failed reload. Please update your payment method and try again.";

const HEADER_FONT_SIZE: f32 = 16.;

const CARD_BORDER_COLOR: ColorU = ColorU::new(43, 43, 43, 255);
const BASE_CREDITS_DOT_COLOR: ColorU = ColorU::new(0, 194, 255, 255);
const BONUS_CREDITS_DOT_COLOR: ColorU = ColorU::new(255, 165, 100, 255);
const DEFAULT_MAX_MONTHLY_SPEND_CENTS: i32 = 20_000;

#[derive(Default)]
struct PlanSectionMouseStates {
    anonymous_user_sign_up_button: MouseStateHandle,
    manage_billing_link: MouseStateHandle,
    open_admin_panel_link: MouseStateHandle,
    admin_panel_link: MouseStateHandle,
}

#[derive(Default)]
struct BuyCreditsMouseStates {
    addon_info_icon: MouseStateHandle,
    edit_monthly_limit: MouseStateHandle,
    auto_reload_switch: SwitchStateHandle,
    auto_reload_info: MouseStateHandle,
    buy_button: MouseStateHandle,
}

#[derive(Default)]
struct TabMouseStates {
    overview: MouseStateHandle,
    usage_history: MouseStateHandle,
}

struct AddonCreditsState {
    selected_denomination: usize,
    options: Vec<AddonCreditsOption>,
    denomination_buttons: Vec<ViewHandle<ActionButton>>,
    purchase_loading: bool,
}

struct UsageHistoryState {
    model: ModelHandle<UsageHistoryModel>,
    expanded_entries: HashMap<String, bool>,
    entry_mouse_states: RefCell<HashMap<String, MouseStateHandle>>,
    tooltip_mouse_states: RefCell<HashMap<String, MouseStateHandle>>,
    load_more_button: ViewHandle<ActionButton>,
}

pub struct BillingAndUsagePageV2View {
    auth_state: Arc<AuthState>,
    addon_credit_modal_state: ModalViewState<Modal<SpendingLimitModal>>,
    selected_tab: BillingUsageTab,
    usage_history: UsageHistoryState,
    addon_credits: AddonCreditsState,
    tab_mouse_states: TabMouseStates,
    plan_mouse_states: PlanSectionMouseStates,
    buy_credits_mouse_states: BuyCreditsMouseStates,
}

impl BillingAndUsagePageV2View {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        ctx.subscribe_to_model(&UserWorkspaces::handle(ctx), |me, _handle, event, ctx| {
            me.handle_workspaces_event(event, ctx);
            ctx.notify();
        });

        ctx.subscribe_to_model(&AuthManager::handle(ctx), |me, _, _, ctx| {
            me.refresh_addon_credits_settings(ctx);
            ctx.notify();
        });

        let team_update_manager = TeamUpdateManager::handle(ctx);
        ctx.subscribe_to_model(&team_update_manager, |_, _handle, _, ctx| {
            ctx.notify();
        });

        ctx.subscribe_to_model(&AIRequestUsageModel::handle(ctx), |_, _, _, ctx| {
            ctx.notify()
        });

        ctx.subscribe_to_model(
            &PricingInfoModel::handle(ctx),
            |me, _handle, _event, ctx| {
                me.update_addon_credits_options(ctx);
                me.refresh_addon_credits_settings(ctx);
                ctx.notify();
            },
        );

        let usage_history_model = ctx.add_model(UsageHistoryModel::new);
        ctx.subscribe_to_model(&usage_history_model, |_, _, _, ctx| {
            ctx.notify();
        });
        usage_history_model.update(ctx, |m, ctx| m.refresh_usage_history_async(ctx));

        let auth_state = AuthStateProvider::as_ref(ctx).get().clone();

        let addon_credit_modal = ctx.add_typed_action_view(SpendingLimitModal::new);
        ctx.subscribe_to_view(&addon_credit_modal, |me, _, event, ctx| {
            me.handle_addon_credit_modal_event(event, ctx);
        });

        let addon_credit_modal_view = ctx.add_typed_action_view(|ctx| {
            Modal::new(
                Some("Monthly spending limit".to_string()),
                addon_credit_modal,
                ctx,
            )
            .with_header_style(UiComponentStyles {
                padding: Some(Coords::uniform(24.).bottom(16.)),
                ..Default::default()
            })
            .with_body_style(UiComponentStyles {
                padding: Some(Coords::uniform(24.).top(0.).bottom(12.)),
                ..Default::default()
            })
        });
        ctx.subscribe_to_view(&addon_credit_modal_view, |me, _, event, ctx| {
            me.handle_addon_credit_modal_close_event(event, ctx);
        });

        let load_more_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Load more", SecondaryTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(BillingAndUsagePageAction::RenderMoreUsageEntries);
            })
        });

        let mut me = Self {
            auth_state,
            addon_credit_modal_state: ModalViewState::new(addon_credit_modal_view),
            selected_tab: BillingUsageTab::Overview,
            usage_history: UsageHistoryState {
                model: usage_history_model,
                expanded_entries: HashMap::new(),
                entry_mouse_states: RefCell::new(HashMap::new()),
                tooltip_mouse_states: RefCell::new(HashMap::new()),
                load_more_button,
            },
            addon_credits: AddonCreditsState {
                selected_denomination: 0,
                options: Default::default(),
                denomination_buttons: Default::default(),
                purchase_loading: false,
            },
            tab_mouse_states: Default::default(),
            plan_mouse_states: Default::default(),
            buy_credits_mouse_states: Default::default(),
        };
        me.update_addon_credits_options(ctx);
        me.refresh_addon_credits_settings(ctx);
        me
    }

    fn refresh_addon_credits_settings(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(workspace) = UserWorkspaces::as_ref(ctx).current_workspace() else {
            return;
        };
        let addon_credits_settings = &workspace.settings.addon_credits_settings;
        if addon_credits_settings.auto_reload_enabled {
            self.addon_credits.selected_denomination = addon_credits_settings
                .selected_auto_reload_credit_denomination
                .and_then(|amount| {
                    self.addon_credits
                        .options
                        .iter()
                        .find_position(|option| option.credits == amount)
                })
                .map_or(0, |pair| pair.0);
        }
        self.update_denomination_buttons_focus(ctx);
    }

    fn update_denomination_buttons_focus(&mut self, ctx: &mut ViewContext<Self>) {
        for (i, button_handle) in self.addon_credits.denomination_buttons.iter().enumerate() {
            ctx.update_view(button_handle, |button, ctx| {
                if i == self.addon_credits.selected_denomination {
                    button.set_theme(PrimaryTheme, ctx);
                } else {
                    button.set_theme(SecondaryTheme, ctx);
                }
            });
        }
    }

    fn handle_workspaces_event(
        &mut self,
        event: &UserWorkspacesEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            UserWorkspacesEvent::TeamsChanged => {
                self.update_addon_credit_modal(ctx);
            }
            UserWorkspacesEvent::UpdateWorkspaceSettingsSuccess => {
                self.update_addon_credit_modal(ctx);
                self.refresh_addon_credits_settings(ctx);
                ctx.notify();
            }
            UserWorkspacesEvent::UpdateWorkspaceSettingsRejected(_err) => {
                self.show_toast(
                    "Failed to update workspace settings",
                    ToastFlavor::Error,
                    ctx,
                );
            }
            UserWorkspacesEvent::AiOveragesUpdated => {
                ctx.notify();
            }
            UserWorkspacesEvent::PurchaseAddonCreditsSuccess => {
                self.addon_credits.purchase_loading = false;
                self.show_toast(
                    "Successfully purchased add-on credits",
                    ToastFlavor::Success,
                    ctx,
                );
                AIRequestUsageModel::handle(ctx)
                    .update(ctx, |m, ctx| m.refresh_request_usage_async(ctx));
            }
            UserWorkspacesEvent::PurchaseAddonCreditsRejected(err) => {
                self.addon_credits.purchase_loading = false;
                self.show_toast(&err.to_string(), ToastFlavor::Error, ctx);
            }
            _ => {}
        }
    }

    fn show_toast(&self, message: &str, flavor: ToastFlavor, ctx: &mut ViewContext<Self>) {
        ctx.emit(BillingAndUsagePageEvent::ShowToast {
            message: message.to_string(),
            flavor,
        });
    }

    pub fn get_modal_content(&self) -> Option<Box<dyn Element>> {
        if self.addon_credit_modal_state.is_open() {
            Some(self.addon_credit_modal_state.render())
        } else {
            None
        }
    }

    fn handle_addon_credit_modal_close_event(
        &mut self,
        event: &ModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ModalEvent::Close => {
                self.addon_credit_modal_state.close();
                ctx.emit(BillingAndUsagePageEvent::HideModal);
            }
        }
    }

    fn handle_addon_credit_modal_event(
        &mut self,
        event: &SpendingLimitModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SpendingLimitModalEvent::Close => {
                self.hide_addon_credit_modal(ctx);
            }
            SpendingLimitModalEvent::Update { amount_cents } => {
                let workspaces = UserWorkspaces::as_ref(ctx);
                let team_uid = workspaces.current_team_uid();

                if let Some(team_uid) = team_uid {
                    UserWorkspaces::handle(ctx).update(ctx, |user_workspaces, ctx| {
                        user_workspaces.update_addon_credits_settings(
                            team_uid,
                            None,
                            Some(*amount_cents as i32),
                            None,
                            ctx,
                        );
                    });
                    self.hide_addon_credit_modal(ctx);
                    ctx.notify();
                }
            }
        }
    }

    fn show_addon_credit_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.addon_credit_modal_state.open();
        self.addon_credit_modal_state
            .view
            .update(ctx, |modal, ctx| {
                modal.body().update(ctx, |body, ctx| {
                    body.focus_input(ctx);
                });
            });
        ctx.emit(BillingAndUsagePageEvent::ShowModal);
    }

    fn hide_addon_credit_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.addon_credit_modal_state.close();
        ctx.emit(BillingAndUsagePageEvent::HideModal);
    }

    fn update_addon_credit_modal(&mut self, ctx: &mut ViewContext<Self>) {
        let addon_limit = UserWorkspaces::as_ref(ctx)
            .current_workspace()
            .and_then(|ws| ws.settings.addon_credits_settings.max_monthly_spend_cents)
            .unwrap_or(DEFAULT_MAX_MONTHLY_SPEND_CENTS);

        self.addon_credit_modal_state
            .view
            .update(ctx, |modal, ctx| {
                modal.body().update(ctx, |body, ctx| {
                    body.update_amount_editor(addon_limit.max(0) as u32, ctx);
                });
            });
        ctx.notify();
    }

    fn update_addon_credits_options(&mut self, ctx: &mut ViewContext<Self>) {
        self.addon_credits.options = PricingInfoModel::as_ref(ctx)
            .addon_credits_options()
            .map(|opts| opts.to_vec())
            .unwrap_or_default();
        self.addon_credits.denomination_buttons = self
            .addon_credits
            .options
            .iter()
            .enumerate()
            .map(|(i, option)| {
                ctx.add_typed_action_view(move |_ctx| {
                    ActionButton::new(option.credits.separate_with_commas(), SecondaryTheme)
                        .with_icon(Icon::Credits)
                        .on_click(move |ctx| {
                            ctx.dispatch_typed_action(
                                BillingAndUsagePageAction::SelectTopupDenomination(i),
                            );
                        })
                })
            })
            .collect();
    }

    // ── Rendering ────────────────────────────────────────────────────────

    fn render_plan_section(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let mut plan_header = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);

        plan_header.add_child(
            Text::new_inline("Plan", appearance.ui_font_family(), HEADER_FONT_SIZE)
                .with_style(Properties::default().weight(Weight::Bold))
                .with_color(appearance.theme().active_ui_text_color().into())
                .finish(),
        );

        let mut right_side = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End);

        let workspaces = UserWorkspaces::as_ref(app);

        if let Some(team) = workspaces.current_team() {
            if team.billing_metadata.customer_type != CustomerType::Unknown {
                right_side.add_child(
                    Container::new(render_customer_type_badge(
                        appearance,
                        team.billing_metadata.customer_type.to_display_string(),
                    ))
                    .with_margin_right(12.)
                    .finish(),
                );
            }

            let current_user_email = AuthStateProvider::as_ref(app)
                .get()
                .user_email()
                .unwrap_or_default();
            let has_admin_permissions = team.has_admin_permissions(&current_user_email);

            if has_admin_permissions {
                if team.billing_metadata.customer_type != CustomerType::Enterprise
                    && team.has_billing_history
                {
                    let team_uid = team.uid;
                    let fg_color = appearance.theme().foreground();
                    right_side.add_child(
                        Container::new(
                            appearance
                                .ui_builder()
                                .button(
                                    ButtonVariant::Link,
                                    self.plan_mouse_states.manage_billing_link.clone(),
                                )
                                .with_text_and_icon_label(
                                    TextAndIcon::new(
                                        TextAndIconAlignment::IconFirst,
                                        "Manage billing",
                                        Icon::CoinsStacked.to_warpui_icon(fg_color),
                                        MainAxisSize::Min,
                                        MainAxisAlignment::Center,
                                        vec2f(14., 14.),
                                    )
                                    .with_inner_padding(4.),
                                )
                                .with_style(UiComponentStyles {
                                    font_color: Some(fg_color.into()),
                                    ..Default::default()
                                })
                                .build()
                                .on_click(move |ctx, _, _| {
                                    ctx.dispatch_typed_action(
                                        BillingAndUsagePageAction::GenerateStripeBillingPortalLink {
                                            team_uid,
                                        },
                                    );
                                })
                                .finish(),
                        )
                        .with_margin_left(12.)
                        .finish(),
                    );
                }

                let team_uid = team.uid;
                let fg_color = appearance.theme().foreground();
                right_side.add_child(
                    Container::new(
                        appearance
                            .ui_builder()
                            .button(
                                ButtonVariant::Link,
                                self.plan_mouse_states.open_admin_panel_link.clone(),
                            )
                            .with_text_and_icon_label(
                                TextAndIcon::new(
                                    TextAndIconAlignment::IconFirst,
                                    "Open admin panel",
                                    Icon::Users.to_warpui_icon(fg_color),
                                    MainAxisSize::Min,
                                    MainAxisAlignment::Center,
                                    vec2f(14., 14.),
                                )
                                .with_inner_padding(4.),
                            )
                            .with_style(UiComponentStyles {
                                font_color: Some(fg_color.into()),
                                ..Default::default()
                            })
                            .build()
                            .on_click(move |ctx, _, _| {
                                ctx.dispatch_typed_action(
                                    BillingAndUsagePageAction::OpenAdminPanel { team_uid },
                                );
                            })
                            .finish(),
                    )
                    .with_margin_left(12.)
                    .finish(),
                );
            }
        } else if self.auth_state.is_anonymous_or_logged_out() {
            right_side.add_child(
                appearance
                    .ui_builder()
                    .button(
                        ButtonVariant::Accent,
                        self.plan_mouse_states.anonymous_user_sign_up_button.clone(),
                    )
                    .with_style(UiComponentStyles {
                        font_size: Some(14.),
                        font_weight: Some(Weight::Semibold),
                        border_radius: Some(CornerRadius::with_all(Radius::Pixels(4.))),
                        padding: Some(Coords {
                            top: 12.,
                            bottom: 12.,
                            left: 40.,
                            right: 40.,
                        }),
                        ..Default::default()
                    })
                    .with_text_label("Sign up".to_owned())
                    .build()
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(BillingAndUsagePageAction::SignupAnonymousUser);
                    })
                    .finish(),
            );
        } else {
            let current_user_id = self.auth_state.user_id().unwrap_or_default();
            right_side.add_child(
                Container::new(render_customer_type_badge(appearance, "Free".into()))
                    .with_margin_right(16.)
                    .finish(),
            );
            right_side.add_child(
                Container::new(
                    appearance
                        .ui_builder()
                        .button(
                            ButtonVariant::Link,
                            self.plan_mouse_states.admin_panel_link.clone(),
                        )
                        .with_text_and_icon_label(
                            TextAndIcon::new(
                                TextAndIconAlignment::IconFirst,
                                "Compare plans",
                                Icon::CoinsStacked.to_warpui_icon(appearance.theme().accent()),
                                MainAxisSize::Min,
                                MainAxisAlignment::Center,
                                vec2f(14., 14.),
                            )
                            .with_inner_padding(4.),
                        )
                        .build()
                        .on_click(move |ctx, _, _| {
                            ctx.dispatch_typed_action(BillingAndUsagePageAction::Upgrade {
                                team_uid: None,
                                user_id: current_user_id,
                            });
                        })
                        .finish(),
                )
                .with_margin_left(12.)
                .finish(),
            );
        }

        plan_header.add_child(right_side.finish());

        Container::new(plan_header.finish())
            .with_margin_bottom(24.)
            .finish()
    }

    fn render_balance_section(
        &self,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Option<Box<dyn Element>> {
        let theme = appearance.theme();
        let ai_model = AIRequestUsageModel::as_ref(app);

        let has_base_credits = ai_model.request_limit() > 0;

        let grants = ai_model.bonus_grants();
        let ws_uid = UserWorkspaces::as_ref(app)
            .current_workspace()
            .map(|ws| ws.uid);

        let has_personal_grants = grants.iter().any(|g| {
            g.scope == BonusGrantScope::User
                && g.grant_type != BonusGrantType::AmbientOnly
                && g.request_credits_remaining > 0
        });
        let has_team_grants = ws_uid.is_some_and(|uid| {
            grants.iter().any(|g| {
                g.scope == BonusGrantScope::Workspace(uid)
                    && g.grant_type != BonusGrantType::AmbientOnly
                    && g.request_credits_remaining > 0
            })
        });

        if !has_base_credits && !has_personal_grants && !has_team_grants {
            return None;
        }

        let mut cards_row = Flex::row()
            .with_spacing(8.)
            .with_main_axis_size(MainAxisSize::Max);

        if has_base_credits {
            let reset_str = ai_model
                .next_refresh_time_local()
                .format("Resets %b %d at %-I:%M %p")
                .to_string();
            let base_remaining = ai_model
                .request_limit()
                .saturating_sub(ai_model.requests_used()) as i64;
            cards_row.add_child(
                Expanded::new(
                    1.,
                    render_balance_card(
                        appearance,
                        BASE_CREDITS_DOT_COLOR,
                        "Base credits",
                        &reset_str,
                        base_remaining,
                        CARD_BORDER_COLOR,
                    ),
                )
                .finish(),
            );
        }

        if has_personal_grants {
            let personal_balance = ai_model.total_user_interactive_bonus_credits_remaining() as i64;
            let personal_expiry = uniform_grant_expiry(grants, |s| *s == BonusGrantScope::User);
            cards_row.add_child(
                Expanded::new(
                    1.,
                    render_balance_card(
                        appearance,
                        BONUS_CREDITS_DOT_COLOR,
                        "Personal credits",
                        &personal_expiry,
                        personal_balance,
                        CARD_BORDER_COLOR,
                    ),
                )
                .finish(),
            );
        }

        if has_team_grants {
            let team_balance = ws_uid
                .map(|uid| ai_model.total_workspace_bonus_credits_remaining(uid) as i64)
                .unwrap_or(0);
            let team_expiry = uniform_grant_expiry(grants, move |s| {
                ws_uid.is_some_and(|uid| *s == BonusGrantScope::Workspace(uid))
            });
            cards_row.add_child(
                Expanded::new(
                    1.,
                    render_balance_card(
                        appearance,
                        BONUS_CREDITS_DOT_COLOR,
                        "Team credits",
                        &team_expiry,
                        team_balance,
                        CARD_BORDER_COLOR,
                    ),
                )
                .finish(),
            );
        }

        Some(
            Flex::column()
                .with_child(
                    Container::new(
                        Text::new_inline("Balance", appearance.ui_font_family(), HEADER_FONT_SIZE)
                            .with_style(Properties::default().weight(Weight::Bold))
                            .with_color(theme.active_ui_text_color().into())
                            .finish(),
                    )
                    .with_margin_bottom(12.)
                    .finish(),
                )
                .with_child(cards_row.finish())
                .with_child(
                    ConstrainedBox::new(Empty::new().finish())
                        .with_height(24.)
                        .finish(),
                )
                .finish(),
        )
    }

    fn render_addon_credits_panel(
        &self,
        workspace: &Workspace,
        team_uid: ServerId,
        has_admin_permissions: bool,
        delinquent: bool,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let fg = appearance.theme().foreground();
        let bg = appearance.theme().background();
        let ui_builder = appearance.ui_builder();
        let theme = appearance.theme();

        let header = Text::new_inline("Buy credits", appearance.ui_font_family(), HEADER_FONT_SIZE)
            .with_color(fg.into())
            .with_style(Properties::default().weight(Weight::Medium))
            .finish();

        let team_can_purchase = UserWorkspaces::as_ref(app)
            .current_team()
            .and_then(|t| t.billing_metadata.tier.purchase_add_on_credits_policy)
            .is_some_and(|p| p.enabled);
        let can_upgrade = UserWorkspaces::as_ref(app)
            .current_workspace()
            .map(|ws| ws.billing_metadata.can_upgrade_to_build_plan())
            .unwrap_or(false);

        let purchase_restriction_message =
            match (team_can_purchase, can_upgrade, has_admin_permissions) {
                (true, _, true) => None,
                (false, true, true) => {
                    let url = UserWorkspaces::upgrade_link_for_team(team_uid);
                    let is_legacy = UserWorkspaces::handle(app)
                        .as_ref(app)
                        .current_team()
                        .is_some_and(|t| t.billing_metadata.is_on_legacy_paid_plan());
                    let (link, suffix) = if is_legacy {
                        ("Switch to the Build plan", " to purchase add-on credits.")
                    } else {
                        ("Upgrade to the Build plan", " to purchase add-on credits.")
                    };
                    Some(
                        FormattedTextElement::new(
                            FormattedText::new([FormattedTextLine::Line(vec![
                                FormattedTextFragment::hyperlink(link, url),
                                FormattedTextFragment::plain_text(suffix),
                            ])]),
                            appearance.ui_font_size(),
                            appearance.ui_font_family(),
                            appearance.ui_font_family(),
                            theme.sub_text_color(bg).into(),
                            HighlightedHyperlink::default(),
                        )
                        .with_hyperlink_font_color(theme.accent().into_solid())
                        .register_default_click_handlers_with_action_support(|lens, event, ctx| {
                            match lens {
                                warpui::elements::HyperlinkLens::Url(u) => ctx.open_url(u),
                                warpui::elements::HyperlinkLens::Action(a) => {
                                    if let Some(act) =
                                        a.as_any().downcast_ref::<BillingAndUsagePageAction>()
                                    {
                                        event.dispatch_typed_action(act.clone());
                                    }
                                }
                            }
                        })
                        .finish(),
                    )
                }
                (false, false, true) => Some(
                    ui_builder
                        .paragraph("Contact your Account Executive for more add-on credits.")
                        .with_style(UiComponentStyles {
                            font_color: Some(theme.sub_text_color(bg).into()),
                            ..Default::default()
                        })
                        .build()
                        .finish(),
                ),
                (_, _, false) => Some(
                    ui_builder
                        .paragraph("Contact a team admin to purchase add-on credits.")
                        .with_style(UiComponentStyles {
                            font_color: Some(theme.sub_text_color(bg).into()),
                            ..Default::default()
                        })
                        .build()
                        .finish(),
                ),
            };

        if let Some(explanation) = purchase_restriction_message {
            let card = Flex::column()
                .with_children([
                    Container::new(header).with_margin_bottom(8.).finish(),
                    explanation,
                ])
                .finish();
            return Container::new(card)
                .with_background_color(theme.surface_1().into_solid())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                .with_margin_bottom(16.)
                .with_uniform_padding(16.)
                .finish();
        }

        let team_count = UserWorkspaces::as_ref(app)
            .current_team()
            .map(|t| t.members.len())
            .unwrap_or(1);
        let para_text = if team_count > 1 {
            format!("{ADDON_CREDITS_DESCRIPTION} {ADDITIONAL_ADDON_CREDITS_DESCRIPTION_FOR_TEAM}")
        } else {
            ADDON_CREDITS_DESCRIPTION.to_string()
        };
        let paragraph = ui_builder
            .paragraph(para_text)
            .with_style(UiComponentStyles {
                font_color: Some(theme.sub_text_color(bg).into()),
                ..Default::default()
            })
            .build()
            .finish();

        let info_icon = render_info_icon(
            appearance,
            AdditionalInfo::<BillingAndUsagePageAction> {
                mouse_state: self.buy_credits_mouse_states.addon_info_icon.clone(),
                on_click_action: None,
                secondary_text: None,
                tooltip_override_text: Some(
                    "Sets the monthly limit spent on add-on credits".to_string(),
                ),
            },
        );

        let spend_limit = workspace
            .settings
            .addon_credits_settings
            .max_monthly_spend_cents
            .map(|c| format!("${:.2}", c as f64 / 100.0))
            .unwrap_or_else(|| "$200.00".to_string());

        let spend_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_children([
                ui_builder.span("Monthly spend limit").build().finish(),
                Shrinkable::new(1., Align::new(info_icon).left().finish()).finish(),
                icon_button(
                    appearance,
                    Icon::Pencil,
                    false,
                    self.buy_credits_mouse_states.edit_monthly_limit.clone(),
                )
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(BillingAndUsagePageAction::ShowAddOnCreditModal);
                })
                .finish(),
                ui_builder.span(spend_limit).build().finish(),
            ])
            .finish();

        let sel = self
            .addon_credits
            .options
            .get(self.addon_credits.selected_denomination);
        let ar_enabled = workspace
            .settings
            .addon_credits_settings
            .auto_reload_enabled;

        let denom_buttons = self
            .addon_credits
            .denomination_buttons
            .iter()
            .map(|h| ChildView::new(h).finish())
            .collect::<Vec<_>>();
        let denoms = Wrap::row()
            .with_children(denom_buttons)
            .with_spacing(8.)
            .finish();

        let mut upper = Flex::column()
            .with_children([header, paragraph, spend_row])
            .with_spacing(8.);

        let would_exceed = sel.is_some_and(|opt| {
            let limit = workspace
                .settings
                .addon_credits_settings
                .max_monthly_spend_cents
                .unwrap_or(DEFAULT_MAX_MONTHLY_SPEND_CENTS);
            (workspace.bonus_grants_purchased_this_month.cents_spent + opt.price_usd_cents) > limit
        });

        let disabled =
            self.addon_credits.purchase_loading || would_exceed || delinquent || ar_enabled;
        let btn_text = if self.addon_credits.purchase_loading {
            "Buying\u{2026}"
        } else {
            "One-time purchase"
        };
        let btn_font = disabled.then_some(theme.disabled_text_color(theme.surface_3()).into());
        let btn_bg = disabled.then_some(theme.surface_3().into());
        let btn_border = disabled.then_some(ColorU::transparent_black().into());
        let mut buy_btn = ui_builder
            .button(
                ButtonVariant::Accent,
                self.buy_credits_mouse_states.buy_button.clone(),
            )
            .with_text_label(btn_text.to_string())
            .with_style(UiComponentStyles {
                font_size: Some(14.),
                font_weight: Some(Weight::Semibold),
                font_color: btn_font,
                background: btn_bg,
                border_color: btn_border,
                ..Default::default()
            })
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(BillingAndUsagePageAction::PurchaseAddonCredits {
                    team_uid,
                });
            });
        if disabled {
            buy_btn = buy_btn.disable();
        }
        let buy_btn = buy_btn.finish();

        let ar_amount = if ar_enabled {
            sel.map(|o| o.credits.to_string())
                .unwrap_or_else(|| "your selected".to_string())
        } else {
            "your selected".to_string()
        };
        let ar_tooltip_text = format!(
            "When enabled, auto reload will automatically purchase {ar_amount} \
            credits when your add-on credit balance reaches 100 credits remaining."
        );

        let ar_switch_el = {
            let sw = ui_builder
                .switch(self.buy_credits_mouse_states.auto_reload_switch.clone())
                .check(ar_enabled);
            if delinquent {
                sw.disable().build().finish()
            } else {
                sw.build()
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(
                            BillingAndUsagePageAction::UpdateAutoReloadEnabled {
                                team_uid,
                                enabled: !ar_enabled,
                            },
                        );
                    })
                    .finish()
            }
        };

        let ar_info_icon = render_info_icon(
            appearance,
            AdditionalInfo::<BillingAndUsagePageAction> {
                mouse_state: self.buy_credits_mouse_states.auto_reload_info.clone(),
                on_click_action: None,
                secondary_text: None,
                tooltip_override_text: Some(ar_tooltip_text),
            },
        );

        let denom_row = Container::new(denoms).with_margin_bottom(8.).finish();

        upper.add_child(denom_row);

        let card_upper = Container::new(upper.finish())
            .with_horizontal_padding(16.)
            .with_padding_top(16.)
            .finish();

        let price_label = sel
            .map(|opt| {
                let credits = opt.credits.separate_with_commas();
                let dollars = format!("${:.2}", opt.price_usd_cents as f64 / 100.0);
                format!("{credits} credits / {dollars}")
            })
            .unwrap_or_default();

        let mut price_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Text::new_inline(price_label, appearance.ui_font_family(), 14.)
                    .with_color(fg.into())
                    .with_style(Properties::default().weight(Weight::Medium))
                    .finish(),
            );
        if ar_enabled {
            price_row.add_child(render_auto_reload_chip(appearance));
        }

        let right_group = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_children([
                Text::new_inline("Auto-reload", appearance.ui_font_family(), 14.)
                    .with_color(fg.into())
                    .with_style(Properties::default().weight(Weight::Semibold))
                    .finish(),
                Container::new(ar_info_icon).with_margin_left(4.).finish(),
                Container::new(ar_switch_el).with_margin_left(8.).finish(),
                Container::new(buy_btn).with_margin_left(16.).finish(),
            ])
            .finish();

        let lower_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(price_row.finish())
            .with_child(right_group);

        let mut lower_children: Vec<Box<dyn Element>> = vec![lower_row.finish()];

        if delinquent {
            lower_children.push(self.render_warning_row(
                appearance,
                AUTO_RELOAD_DELINQUENT_WARNING_STRING.to_string(),
            ));
        } else if workspace
            .billing_metadata
            .has_failed_addon_credit_auto_reload_status()
        {
            lower_children.push(self.render_warning_row(
                appearance,
                RESTRICTED_BILLING_USAGE_WARNING_STRING.to_string(),
            ));
        } else if would_exceed {
            lower_children.push(
                self.render_warning_row(
                    appearance,
                    "Reloading would exceed your monthly limit. Increase your limit to continue."
                        .to_string(),
                ),
            );
        }

        let card_lower = Container::new(
            Flex::column()
                .with_children(lower_children)
                .with_spacing(8.)
                .finish(),
        )
        .with_uniform_padding(16.)
        .with_border(Border::top(1.).with_border_color(theme.outline().into()))
        .finish();

        Container::new(
            Flex::column()
                .with_children([card_upper, card_lower])
                .finish(),
        )
        .with_background_color(theme.surface_1().into_solid())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .with_margin_bottom(16.)
        .finish()
    }

    fn render_warning_row(&self, appearance: &Appearance, msg: String) -> Box<dyn Element> {
        let theme = appearance.theme();
        let icon = ConstrainedBox::new(
            Icon::AlertTriangle
                .to_warpui_icon(theme.ui_error_color().into())
                .finish(),
        )
        .with_height(16.)
        .with_width(16.)
        .finish();
        let text = Text::new(msg, appearance.ui_font_family(), 12.)
            .with_color(theme.ui_error_color())
            .finish();
        Container::new(
            Flex::row()
                .with_child(Container::new(icon).with_margin_right(8.).finish())
                .with_child(Shrinkable::new(1.0, text).finish())
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .finish(),
        )
        .with_margin_top(8.)
        .finish()
    }

    fn render_overview_tab(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let mut content = Flex::column();

        content.add_child(self.render_plan_section(appearance, app));
        if let Some(balance) = self.render_balance_section(appearance, app) {
            content.add_child(balance);
        }

        let delinquent = UserWorkspaces::as_ref(app)
            .current_team()
            .map(|t| t.billing_metadata.is_delinquent_due_to_payment_issue())
            .unwrap_or_default();

        let ai_model = AIRequestUsageModel::as_ref(app);

        if let (Some(ws), Some(team)) = (
            UserWorkspaces::as_ref(app).current_workspace(),
            UserWorkspaces::as_ref(app).current_team(),
        ) {
            let bonus = ai_model.total_workspace_bonus_credits_remaining(ws.uid);
            let is_payg_zero =
                ws.billing_metadata.is_enterprise_pay_as_you_go_enabled() && bonus == 0;

            if !is_payg_zero {
                let admin = {
                    let email = AuthStateProvider::as_ref(app)
                        .get()
                        .user_email()
                        .unwrap_or_default();
                    team.has_admin_permissions(&email)
                };
                content.add_child(
                    self.render_addon_credits_panel(ws, team.uid, admin, delinquent, app),
                );
            }
        }

        content.finish()
    }

    fn render_usage_history_tab(
        &self,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let history = self.usage_history.model.as_ref(app);
        if history.entries().is_empty() {
            return self.render_empty_usage_history(history.is_loading(), appearance, app);
        }

        let mut content = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_child(
                Container::new(
                    Text::new_inline("Last 30 days", appearance.ui_font_family(), 14.)
                        .with_color(blended_colors::text_sub(
                            appearance.theme(),
                            appearance.theme().surface_1(),
                        ))
                        .finish(),
                )
                .with_vertical_margin(12.)
                .finish(),
            );

        let mut list = Flex::column().with_spacing(8.);
        for entry in history.entries().iter() {
            let expanded = self
                .usage_history
                .expanded_entries
                .get(&entry.conversation_id)
                .copied()
                .unwrap_or(false);
            let ms = self
                .usage_history
                .entry_mouse_states
                .borrow_mut()
                .entry(entry.conversation_id.clone())
                .or_default()
                .clone();
            let tms = self
                .usage_history
                .tooltip_mouse_states
                .borrow_mut()
                .entry(entry.conversation_id.clone())
                .or_default()
                .clone();
            list.add_child(
                Container::new(
                    UsageHistoryEntry::new(Some(entry.clone()), expanded, Some(ms), tms)
                        .render(appearance, app),
                )
                .finish(),
            );
        }
        content.add_child(list.finish());

        if history.has_more_entries() {
            content.add_child(
                Container::new(
                    Flex::row()
                        .with_child(self.usage_history.load_more_button.as_ref(app).render(app))
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_main_axis_size(MainAxisSize::Max)
                        .finish(),
                )
                .with_margin_top(24.)
                .finish(),
            );
        }

        content.finish()
    }

    fn render_empty_usage_history(
        &self,
        loading: bool,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let mut content = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_alignment(MainAxisAlignment::Center);

        if loading {
            let mut list = Flex::column().with_spacing(8.);
            for _ in 0..3 {
                list.add_child(
                    UsageHistoryEntry::new(None, false, None, MouseStateHandle::default())
                        .render(appearance, app),
                );
            }
            content.add_child(list.finish());
        } else {
            let zero = Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Container::new(
                        ConstrainedBox::new(
                            Icon::Conversation
                                .to_warpui_icon(
                                    blended_colors::text_sub(
                                        appearance.theme(),
                                        appearance.theme().surface_1(),
                                    )
                                    .into(),
                                )
                                .finish(),
                        )
                        .with_width(24.)
                        .with_height(24.)
                        .finish(),
                    )
                    .with_margin_bottom(12.)
                    .finish(),
                )
                .with_child(
                    Container::new(
                        Text::new("No usage history", appearance.ui_font_family(), 14.)
                            .with_color(blended_colors::text_sub(
                                appearance.theme(),
                                appearance.theme().surface_1(),
                            ))
                            .finish(),
                    )
                    .with_margin_bottom(4.)
                    .finish(),
                )
                .with_child(
                    Text::new(
                        "Kick off an agent task to view usage history here.",
                        appearance.ui_font_family(),
                        14.,
                    )
                    .with_color(blended_colors::text_disabled(
                        appearance.theme(),
                        appearance.theme().surface_1(),
                    ))
                    .finish(),
                );
            content.add_child(
                Container::new(zero.finish())
                    .with_vertical_margin(160.)
                    .finish(),
            );
        }

        content.finish()
    }
}

impl SettingsPageMeta for BillingAndUsagePageV2View {
    fn section() -> SettingsSection {
        SettingsSection::BillingAndUsage
    }

    fn should_render(&self, ctx: &AppContext) -> bool {
        !AuthStateProvider::as_ref(ctx)
            .get()
            .is_anonymous_or_logged_out()
    }

    fn on_page_selected(&mut self, _allow_steal_focus: bool, ctx: &mut ViewContext<Self>) {
        self.addon_credits.purchase_loading = false;
        std::mem::drop(
            TeamUpdateManager::handle(ctx)
                .update(ctx, |mgr, ctx| mgr.refresh_workspace_metadata(ctx)),
        );
        AIRequestUsageModel::handle(ctx).update(ctx, |m, ctx| m.refresh_request_usage_async(ctx));
        self.usage_history
            .model
            .update(ctx, |m, ctx| m.refresh_usage_history_async(ctx));
        self.refresh_addon_credits_settings(ctx);
    }

    fn update_filter(&mut self, _query: &str, _ctx: &mut ViewContext<Self>) -> MatchData {
        MatchData::Uncounted(false)
    }

    fn scroll_to_widget(&mut self, _widget_id: &'static str) {}

    fn clear_highlighted_widget(&mut self) {}
}

impl Entity for BillingAndUsagePageV2View {
    type Event = BillingAndUsagePageEvent;
}

impl View for BillingAndUsagePageV2View {
    fn ui_name() -> &'static str {
        "Billing and usage v2"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let mut page = Flex::column();

        let tabs = vec![
            SettingsTab::new(
                BillingUsageTab::Overview.label(),
                self.tab_mouse_states.overview.clone(),
            ),
            SettingsTab::new(
                BillingUsageTab::UsageHistory.label(),
                self.tab_mouse_states.usage_history.clone(),
            ),
        ];

        page.add_child(tab_selector::render_tab_selector(
            tabs,
            self.selected_tab.label(),
            |label, ctx| {
                ctx.dispatch_typed_action(BillingAndUsagePageAction::SelectTab(
                    BillingUsageTab::get_tab_from_label(label),
                ));
            },
            appearance,
        ));

        if self.selected_tab == BillingUsageTab::Overview {
            page.add_child(self.render_overview_tab(appearance, app));
        } else {
            page.add_child(self.render_usage_history_tab(appearance, app));
        }

        Container::new(
            Align::new(
                ConstrainedBox::new(page.finish())
                    .with_max_width(800.)
                    .finish(),
            )
            .top_center()
            .finish(),
        )
        .with_uniform_padding(PAGE_PADDING)
        .finish()
    }
}

impl TypedActionView for BillingAndUsagePageV2View {
    type Action = BillingAndUsagePageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        let is_login_gated = matches!(
            action,
            BillingAndUsagePageAction::Upgrade { .. }
                | BillingAndUsagePageAction::GenerateStripeBillingPortalLink { .. },
        );
        if AuthStateProvider::as_ref(ctx)
            .get()
            .is_anonymous_or_logged_out()
            && is_login_gated
        {
            AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                auth_manager.attempt_login_gated_feature(
                    action.into(),
                    AuthViewVariant::RequireLoginCloseable,
                    ctx,
                )
            });
            return;
        }

        match action {
            BillingAndUsagePageAction::Upgrade { team_uid, user_id } => match team_uid {
                Some(team_uid) => ctx.open_url(&UserWorkspaces::upgrade_link_for_team(*team_uid)),
                None => ctx.open_url(&UserWorkspaces::upgrade_link(*user_id)),
            },
            BillingAndUsagePageAction::GenerateStripeBillingPortalLink { team_uid } => {
                UserWorkspaces::handle(ctx).update(ctx, |ws, ctx| {
                    ws.generate_stripe_billing_portal_link(*team_uid, ctx);
                });
            }
            BillingAndUsagePageAction::OpenAdminPanel { team_uid } => {
                super::admin_actions::AdminActions::open_admin_panel(*team_uid, ctx);
            }
            BillingAndUsagePageAction::ContactSupport => {
                super::admin_actions::AdminActions::contact_support(ctx);
            }
            BillingAndUsagePageAction::SignupAnonymousUser => {
                ctx.emit(BillingAndUsagePageEvent::SignupAnonymousUser);
            }
            BillingAndUsagePageAction::AttemptLoginGatedUpgrade => {
                AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                    auth_manager.attempt_login_gated_feature(
                        action.into(),
                        AuthViewVariant::RequireLoginCloseable,
                        ctx,
                    )
                });
            }
            BillingAndUsagePageAction::OpenUrl(url) => ctx.open_url(&url.url),
            // Not applicable in v2
            BillingAndUsagePageAction::UpdateUsageBasedPricingSettings { .. }
            | BillingAndUsagePageAction::ShowOverageLimitModal => {}
            // Not applicable in v2
            BillingAndUsagePageAction::ToggleSortingMenu
            | BillingAndUsagePageAction::ChangeUsageSort { .. } => {}
            BillingAndUsagePageAction::RefreshWorkspaceData => {
                std::mem::drop(
                    TeamUpdateManager::handle(ctx)
                        .update(ctx, |mgr, ctx| mgr.refresh_workspace_metadata(ctx)),
                );
                AIRequestUsageModel::handle(ctx)
                    .update(ctx, |m, ctx| m.refresh_request_usage_async(ctx));
            }
            BillingAndUsagePageAction::SelectTab(tab) => {
                if self.selected_tab != *tab {
                    self.selected_tab = tab.clone();
                    ctx.notify();
                }
            }
            BillingAndUsagePageAction::ToggleUsageEntryExpanded { conversation_id } => {
                let expanded = self
                    .usage_history
                    .expanded_entries
                    .get(conversation_id)
                    .copied()
                    .unwrap_or(false);
                self.usage_history
                    .expanded_entries
                    .insert(conversation_id.clone(), !expanded);
                ctx.notify();
            }
            BillingAndUsagePageAction::RenderMoreUsageEntries => {
                self.usage_history
                    .model
                    .update(ctx, |m, ctx| m.load_more_usage_history_async(ctx));
            }
            BillingAndUsagePageAction::SelectTopupDenomination(i) => {
                self.addon_credits.selected_denomination = *i;
                self.update_denomination_buttons_focus(ctx);
                UserWorkspaces::handle(ctx).update(ctx, |ws, ctx| {
                    let team_uid = ws.current_team_uid();
                    if let Some((workspace, team_uid)) = ws.current_workspace().zip(team_uid) {
                        if workspace
                            .settings
                            .addon_credits_settings
                            .auto_reload_enabled
                        {
                            if let Some(opt) = self
                                .addon_credits
                                .options
                                .get(self.addon_credits.selected_denomination)
                            {
                                ws.update_addon_credits_settings(
                                    team_uid,
                                    None,
                                    None,
                                    Some(opt.credits),
                                    ctx,
                                );
                            }
                        }
                    }
                });
                ctx.notify();
            }
            BillingAndUsagePageAction::PurchaseAddonCredits { team_uid } => {
                if let Some(opt) = self
                    .addon_credits
                    .options
                    .get(self.addon_credits.selected_denomination)
                {
                    let credits = opt.credits;
                    let uid = *team_uid;
                    self.addon_credits.purchase_loading = true;
                    UserWorkspaces::handle(ctx).update(ctx, |ws, ctx| {
                        ws.purchase_addon_credits(uid, credits, ctx);
                    });
                    ctx.notify();
                }
            }
            BillingAndUsagePageAction::ShowAddOnCreditModal => {
                self.show_addon_credit_modal(ctx);
            }
            BillingAndUsagePageAction::UpdateAutoReloadEnabled { team_uid, enabled } => {
                send_telemetry_from_ctx!(
                    TelemetryEvent::AutoReloadToggledFromBillingSettings {
                        enabled: *enabled,
                        banner_toggle_flag_enabled: FeatureFlag::BuildPlanAutoReloadBannerToggle
                            .is_enabled(),
                        post_purchase_modal_flag_enabled:
                            FeatureFlag::BuildPlanAutoReloadPostPurchaseModal.is_enabled(),
                    },
                    ctx
                );
                let reload_val = if *enabled {
                    self.addon_credits
                        .options
                        .get(self.addon_credits.selected_denomination)
                        .map(|o| o.credits)
                } else {
                    None
                };
                UserWorkspaces::handle(ctx).update(ctx, |ws, ctx| {
                    ws.update_addon_credits_settings(
                        *team_uid,
                        Some(*enabled),
                        None,
                        reload_val,
                        ctx,
                    );
                });
            }
            BillingAndUsagePageAction::DismissAmbientAgentTrialWidget => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    let _ = settings
                        .ambient_agent_trial_widget_dismissed
                        .set_value(true, ctx);
                });
                ctx.notify();
            }
            BillingAndUsagePageAction::NavigateToByokSettings => {
                ctx.dispatch_typed_action_deferred(WorkspaceAction::ShowSettingsPageWithSearch {
                    search_query: "api".to_string(),
                    section: Some(SettingsSection::WarpAgent),
                });
            }
        }
    }
}

fn render_auto_reload_chip(appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    Container::new(
        Container::new(
            Text::new_inline("Auto-reload", appearance.ui_font_family(), 14.)
                .with_color(theme.accent().into_solid())
                .finish(),
        )
        .with_horizontal_padding(4.)
        .with_vertical_padding(2.)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_border(Border::all(1.).with_border_color(theme.accent_overlay().into()))
        .with_background(warp_core::ui::theme::color::internal_colors::accent_overlay_1(theme))
        .finish(),
    )
    .with_margin_left(8.)
    .finish()
}

fn render_balance_card(
    appearance: &Appearance,
    dot_color: ColorU,
    label: &str,
    date: &str,
    remaining: i64,
    border_color: ColorU,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let sub_color = blended_colors::text_sub(theme, theme.background());

    let status_dot = ConstrainedBox::new(
        Container::new(Empty::new().finish())
            .with_background_color(dot_color)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish(),
    )
    .with_width(8.)
    .with_height(8.)
    .finish();

    let label_text = Text::new_inline(label.to_string(), appearance.ui_font_family(), 12.)
        .with_color(sub_color)
        .with_style(Properties::default().weight(Weight::Semibold))
        .finish();

    let date_text = Clipped::new(
        Text::new_inline(date.to_string(), appearance.ui_font_family(), 10.)
            .with_color(sub_color)
            .finish(),
    )
    .finish();

    let header = Flex::row()
        .with_child(status_dot)
        .with_child(
            Container::new(Shrinkable::new(1., label_text).finish())
                .with_margin_left(8.)
                .with_margin_right(8.)
                .finish(),
        )
        .with_child(Shrinkable::new(1., Align::new(date_text).right().finish()).finish())
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .finish();

    let credit_count = Text::new_inline(
        remaining.separate_with_commas(),
        appearance.ui_font_family(),
        24.,
    )
    .with_color(theme.active_ui_text_color().into())
    .with_style(Properties::default().weight(Weight::Semibold))
    .finish();

    let remaining_label = Text::new_inline("remaining", appearance.ui_font_family(), 14.)
        .with_color(sub_color)
        .finish();

    let value_row = Flex::row()
        .with_child(credit_count)
        .with_child(
            Container::new(remaining_label)
                .with_margin_left(4.)
                .with_padding_bottom(1.)
                .finish(),
        )
        .with_cross_axis_alignment(CrossAxisAlignment::End);

    Container::new(
        Flex::column()
            .with_child(header)
            .with_child(value_row.finish())
            .with_spacing(8.)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .finish(),
    )
    .with_border(Border::all(1.).with_border_color(border_color))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
    .with_horizontal_padding(16.)
    .with_vertical_padding(12.)
    .finish()
}

fn uniform_grant_expiry(
    grants: &[BonusGrant],
    scope_filter: impl Fn(&BonusGrantScope) -> bool,
) -> String {
    let now = chrono::Utc::now();
    let expiries: Vec<_> = grants
        .iter()
        .filter(|g| scope_filter(&g.scope))
        .filter(|g| g.grant_type != BonusGrantType::AmbientOnly)
        .filter(|g| g.expiration.is_none_or(|exp| now < exp))
        .filter(|g| g.request_credits_remaining > 0)
        .filter_map(|g| g.expiration)
        .collect();
    if expiries.is_empty() {
        return String::new();
    }
    let first = expiries[0];
    if expiries
        .iter()
        .all(|e| e.date_naive() == first.date_naive())
    {
        let local = first.with_timezone(&Local);
        format!("Expires {}", local.format("%b %d, %Y"))
    } else {
        String::new()
    }
}

impl From<warpui::ViewHandle<BillingAndUsagePageV2View>> for SettingsPageViewHandle {
    fn from(view_handle: warpui::ViewHandle<BillingAndUsagePageV2View>) -> Self {
        SettingsPageViewHandle::BillingAndUsageV2(view_handle)
    }
}
