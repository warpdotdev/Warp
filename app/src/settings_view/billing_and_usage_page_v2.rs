//! V2 rendering for the Billing & Usage settings page.
//!
//! Gated behind `FeatureFlag::BillingAndUsagePageV2`. When the flag is enabled,
//! the v1 widget `render` methods delegate here; when disabled, v1 code runs
//! unchanged.

use chrono::{Local, Utc};
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use thousands::Separable;
use warp_core::ui::appearance::Appearance;
use warpui::{
    elements::{
        Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Expanded,
        Flex, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement, Radius, Text,
    },
    fonts::{Properties, Weight},
    ui_components::{
        button::{ButtonVariant, TextAndIcon, TextAndIconAlignment},
        components::UiComponent,
    },
    AppContext, Element, SingletonEntity,
};

use crate::{
    ai::{request_usage_model::BonusGrantScope, AIRequestUsageModel},
    auth::AuthStateProvider,
    settings_view::settings_page::{render_customer_type_badge, HEADER_PADDING},
    ui_components::{blended_colors, icons::Icon},
    workspaces::{
        user_workspaces::UserWorkspaces,
        workspace::CustomerType,
    },
};

use super::billing_and_usage_page::{
    BillingAndUsagePageAction, BillingAndUsagePageView, BillingUsageTab, PlanWidget, UsageWidget,
};

// ─── Plan Widget V2 ─────────────────────────────────────────────────────────

/// Renders the v2 plan row: white-text plan name, badge, manage billing + admin
/// panel buttons (admin only). Non-admin users see only the plan name + badge.
pub fn render_plan_widget_v2(
    _widget: &PlanWidget,
    view: &BillingAndUsagePageView,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    if view.auth_state.is_anonymous_or_logged_out() {
        return Empty::new().finish();
    }

    let theme = appearance.theme();
    let fg = theme.foreground();

    let workspaces = UserWorkspaces::as_ref(app);
    let team = workspaces.current_team();

    let plan_name = team
        .map(|t| t.billing_metadata.tier.name.clone())
        .unwrap_or_else(|| "Free".to_string());

    // White text plan name
    let plan_label = Text::new_inline(plan_name, appearance.ui_font_family(), 16.)
        .with_color(fg.into())
        .with_style(Properties::default().weight(Weight::Bold))
        .finish();

    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);

    // Left side: plan name + badge
    let mut left = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    left.add_child(Container::new(plan_label).with_margin_right(8.).finish());

    if let Some(team) = team {
        if team.billing_metadata.customer_type != CustomerType::Unknown {
            left.add_child(render_customer_type_badge(
                appearance,
                team.billing_metadata.customer_type.to_display_string(),
            ));
        }
    }

    row.add_child(left.finish());

    // Right side: admin buttons (admin only)
    if let Some(team) = team {
        let current_user_email = AuthStateProvider::as_ref(app)
            .get()
            .user_email()
            .unwrap_or_default();
        let has_admin_permissions = team.has_admin_permissions(&current_user_email);

        if has_admin_permissions {
            let mut right = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

            // Manage billing button (skip for enterprise or no billing history)
            if team.billing_metadata.customer_type != CustomerType::Enterprise
                && team.has_billing_history
            {
                let team_uid = team.uid;
                let manage_billing = appearance
                    .ui_builder()
                    .button(
                        ButtonVariant::Link,
                        MouseStateHandle::default(),
                    )
                    .with_text_and_icon_label(
                        TextAndIcon::new(
                            TextAndIconAlignment::IconFirst,
                            "Manage billing",
                            Icon::CoinsStacked.to_warpui_icon(theme.accent()),
                            MainAxisSize::Min,
                            MainAxisAlignment::Center,
                            vec2f(14., 14.),
                        )
                        .with_inner_padding(4.),
                    )
                    .build()
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(
                            BillingAndUsagePageAction::GenerateStripeBillingPortalLink { team_uid },
                        );
                    })
                    .finish();
                right.add_child(Container::new(manage_billing).with_margin_right(8.).finish());
            }

            // Open admin panel button
            let team_uid = team.uid;
            let admin_panel = appearance
                .ui_builder()
                .button(
                    ButtonVariant::Link,
                    MouseStateHandle::default(),
                )
                .with_text_and_icon_label(
                    TextAndIcon::new(
                        TextAndIconAlignment::IconFirst,
                        "Open admin panel",
                        Icon::Users.to_warpui_icon(theme.accent()),
                        MainAxisSize::Min,
                        MainAxisAlignment::Center,
                        vec2f(14., 14.),
                    )
                    .with_inner_padding(4.),
                )
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(BillingAndUsagePageAction::OpenAdminPanel {
                        team_uid,
                    });
                })
                .finish();
            right.add_child(admin_panel);

            row.add_child(right.finish());
        }
    }

    Container::new(row.finish())
        .with_margin_bottom(HEADER_PADDING)
        .finish()
}

// ─── Usage Widget V2 ────────────────────────────────────────────────────────

/// Top-level v2 render for the UsageWidget. Renders the tab selector then
/// delegates to the v2 Overview or the existing Usage History tab.
pub fn render_usage_widget_v2(
    widget: &UsageWidget,
    view: &BillingAndUsagePageView,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let mut col = Flex::column();

    // Tab selector (reuse same tabs as v1)
    let tabs = vec![
        crate::ui_components::tab_selector::SettingsTab::new(
            BillingUsageTab::Overview.label(),
            widget.overview_tab_mouse_state.clone(),
        ),
        crate::ui_components::tab_selector::SettingsTab::new(
            BillingUsageTab::UsageHistory.label(),
            widget.usage_history_tab_mouse_state.clone(),
        ),
    ];
    let tab_selector = crate::ui_components::tab_selector::render_tab_selector(
        tabs,
        view.selected_tab.label(),
        |label, ctx| {
            ctx.dispatch_typed_action(BillingAndUsagePageAction::SelectTab(
                BillingUsageTab::get_tab_from_label(label),
            ));
        },
        appearance,
    );
    col.add_child(tab_selector);

    if view.selected_tab == BillingUsageTab::Overview {
        col.add_child(render_overview_v2(widget, view, appearance, app));
    } else {
        // Delegate to existing v1 usage history rendering.
        col.add_child(widget.render_usage_history_content(view, appearance, app));
    }

    col.finish()
}

// ─── V2 Overview ────────────────────────────────────────────────────────────

fn render_overview_v2(
    widget: &UsageWidget,
    view: &BillingAndUsagePageView,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let mut col = Flex::column();

    // Balance section
    if let Some(balance) = render_balance_section(appearance, app) {
        col.add_child(
            Container::new(balance).with_margin_bottom(24.).finish(),
        );
    }

    // Reuse the v1 addon credits panel for purchasing.
    let workspace = UserWorkspaces::as_ref(app).current_workspace();
    let team = UserWorkspaces::as_ref(app).current_team();
    let current_user_email = AuthStateProvider::as_ref(app)
        .get()
        .user_email()
        .unwrap_or_default();
    let has_admin_permissions =
        team.is_some_and(|t| t.has_admin_permissions(&current_user_email));
    let workspace_is_delinquent = team
        .map(|t| t.billing_metadata.is_delinquent_due_to_payment_issue())
        .unwrap_or(false);

    if let (Some(workspace), Some(team)) = (workspace, team) {
        let ai_model = AIRequestUsageModel::as_ref(app);
        let bonus_credit_balance =
            ai_model.total_workspace_bonus_credits_remaining(workspace.uid);

        let is_enterprise_payg_with_zero_credits = workspace
            .billing_metadata
            .is_enterprise_pay_as_you_go_enabled()
            && bonus_credit_balance == 0;

        if !is_enterprise_payg_with_zero_credits {
            col.add_child(widget.render_addon_credits_panel(
                view.selected_addon_denomination,
                workspace,
                team.uid,
                has_admin_permissions,
                bonus_credit_balance,
                &view.addon_credits_options,
                &view.addon_credit_denomination_buttons,
                view.purchase_addon_credits_loading,
                workspace_is_delinquent,
                app,
            ));
        }
    }

    col.finish()
}

// ─── Balance Section ────────────────────────────────────────────────────────

fn render_balance_section(
    appearance: &Appearance,
    app: &AppContext,
) -> Option<Box<dyn Element>> {
    let ai_model = AIRequestUsageModel::as_ref(app);
    let workspace = UserWorkspaces::as_ref(app).current_workspace();
    let workspace_uid = workspace.map(|w| w.uid);

    let mut cards: Vec<Box<dyn Element>> = Vec::new();

    // Base credits card
    let base_limit = ai_model.request_limit();
    if base_limit > 0 {
        let base_remaining = base_limit.saturating_sub(ai_model.requests_used());
        let next_refresh = ai_model.next_refresh_time();
        let formatted_date = next_refresh
            .with_timezone(&Local)
            .format("%b %d")
            .to_string();

        cards.push(render_balance_card(
            appearance,
            ColorU::new(0, 200, 150, 255), // teal/green
            "Base credits",
            &format!("Resets {formatted_date}"),
            base_remaining,
            false,
        ));
    }

    // Personal credits card (user-scoped bonus grants)
    let now = Utc::now();
    let user_grants: Vec<_> = ai_model
        .bonus_grants()
        .iter()
        .filter(|g| g.scope == BonusGrantScope::User)
        .collect();
    if !user_grants.is_empty() {
        let personal_remaining: i32 = user_grants
            .iter()
            .filter(|g| g.expiration.is_none_or(|exp| now < exp))
            .map(|g| g.request_credits_remaining)
            .sum();
        let soonest_expiration = user_grants
            .iter()
            .filter_map(|g| g.expiration)
            .min();
        let expiry_text = soonest_expiration
            .map(|exp| {
                format!(
                    "Expires {}",
                    exp.with_timezone(&Local).format("%b %d")
                )
            })
            .unwrap_or_else(|| "No expiration".to_string());

        let auto_reload = workspace
            .is_some_and(|w| w.settings.addon_credits_settings.auto_reload_enabled);

        cards.push(render_balance_card(
            appearance,
            ColorU::new(255, 160, 50, 255), // orange
            "Personal credits",
            &expiry_text,
            personal_remaining.max(0) as usize,
            auto_reload,
        ));
    }

    // Team credits card (workspace-scoped bonus grants)
    if let Some(uid) = workspace_uid {
        let workspace_grants: Vec<_> = ai_model
            .bonus_grants()
            .iter()
            .filter(|g| g.scope == BonusGrantScope::Workspace(uid))
            .collect();
        if !workspace_grants.is_empty() {
            let team_remaining: i32 = workspace_grants
                .iter()
                .filter(|g| g.expiration.is_none_or(|exp| now < exp))
                .map(|g| g.request_credits_remaining)
                .sum();
            let soonest_expiration = workspace_grants
                .iter()
                .filter_map(|g| g.expiration)
                .min();
            let expiry_text = soonest_expiration
                .map(|exp| {
                    format!(
                        "Expires {}",
                        exp.with_timezone(&Local).format("%b %d")
                    )
                })
                .unwrap_or_else(|| "No expiration".to_string());

            cards.push(render_balance_card(
                appearance,
                ColorU::new(230, 80, 120, 255), // pink/red
                "Team credits",
                &expiry_text,
                team_remaining.max(0) as usize,
                false,
            ));
        }
    }

    if cards.is_empty() {
        return None;
    }

    // Header: "Balance" (16px bold)
    let header = Text::new_inline("Balance", appearance.ui_font_family(), 16.)
        .with_color(appearance.theme().foreground().into())
        .with_style(Properties::default().weight(Weight::Bold))
        .finish();

    let cards_row = Flex::row()
        .with_children(cards)
        .with_spacing(8.)
        .finish();

    Some(
        Flex::column()
            .with_child(Container::new(header).with_margin_bottom(12.).finish())
            .with_child(cards_row)
            .finish(),
    )
}

fn render_balance_card(
    appearance: &Appearance,
    dot_color: ColorU,
    title: &str,
    date_text: &str,
    remaining: usize,
    show_auto_reload: bool,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let bg = theme.background().into_solid();
    let sub_color = blended_colors::text_sub(theme, bg);

    // Colored dot
    let dot = ConstrainedBox::new(
        Container::new(Empty::new().finish())
            .with_background_color(dot_color)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish(),
    )
    .with_width(8.)
    .with_height(8.)
    .finish();

    // Title (semibold 12px)
    let title_text = Text::new_inline(title.to_string(), appearance.ui_font_family(), 12.)
        .with_color(sub_color)
        .with_style(Properties::default().weight(Weight::Semibold))
        .finish();

    // Date text (10px, right-aligned via Expanded spacer)
    let date_label = Text::new_inline(date_text.to_string(), appearance.ui_font_family(), 10.)
        .with_color(sub_color)
        .finish();

    // Header row: dot + title + (spacer) + date
    let header_row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(Container::new(dot).with_margin_right(8.).finish())
        .with_child(
            Expanded::new(1., title_text).finish(),
        )
        .with_child(date_label);

    // Large remaining number (24px semibold)
    let remaining_text = Text::new_inline(
        remaining.separate_with_commas(),
        appearance.ui_font_family(),
        24.,
    )
    .with_color(blended_colors::text_main(theme, bg).into())
    .with_style(Properties::default().weight(Weight::Semibold))
    .finish();

    // "remaining" label (14px regular)
    let remaining_label = Text::new_inline("remaining", appearance.ui_font_family(), 14.)
        .with_color(sub_color)
        .finish();

    // Value row: number + "remaining" aligned at bottom
    let mut value_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::End)
        .with_child(Container::new(remaining_text).with_margin_right(4.).finish())
        .with_child(
            Container::new(remaining_label).with_padding_bottom(1.).finish(),
        );

    if show_auto_reload {
        let chip = Container::new(
            Text::new_inline("Auto-reload", appearance.ui_font_family(), 14.)
                .with_color(blended_colors::text_main(theme, bg).into())
                .finish(),
        )
        .with_horizontal_padding(4.)
        .with_vertical_padding(2.)
        .with_background(blended_colors::accent_bg(theme))
        .with_border(
            Border::all(1.).with_border_fill(blended_colors::accent_bg_strong(theme)),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish();
        value_row.add_child(Container::new(chip).with_margin_left(8.).finish());
    }

    let content = Flex::column()
        .with_child(header_row.finish())
        .with_child(
            Container::new(value_row.finish()).with_margin_top(8.).finish(),
        )
        .finish();

    Expanded::new(
        1.,
        ConstrainedBox::new(
            Container::new(content)
                .with_background(theme.background())
                .with_border(
                    Border::all(1.)
                        .with_border_color(blended_colors::neutral_3(theme)),
                )
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                .with_horizontal_padding(16.)
                .with_vertical_padding(12.)
                .finish(),
        )
        .with_height(88.)
        .finish(),
    )
    .finish()
}
