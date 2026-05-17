use chrono::{DateTime, Local, Utc};
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::appearance::Appearance;
use warpui::{
    elements::{
        ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
        Flex, Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, OffsetPositioning,
        ParentAnchor, ParentElement, ParentOffsetBounds, Radius, Stack, Text,
    },
    fonts::{Properties, Weight},
    platform::Cursor,
    ui_components::{
        button::ButtonVariant,
        components::{UiComponent, UiComponentStyles},
    },
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::{
    ai::AIRequestUsageModel,
    auth::{AuthManager, AuthStateProvider},
    menu::{self, Menu, MenuItem, MenuItemFields},
    settings_view::{
        admin_actions::AdminActions,
        billing_and_usage::billing_cycle_usage_rows::{render_rows, RowMouseStates, SourceFilter},
        billing_and_usage_page_v2::{
            AGGREGATE_CREDITS_DOT_COLOR, AMBIENT_CREDITS_DOT_COLOR, BASE_CREDITS_DOT_COLOR,
            BONUS_CREDITS_DOT_COLOR, PAYG_CREDITS_DOT_COLOR,
        },
    },
    ui_components::icons::Icon,
    workspaces::{
        update_manager::TeamUpdateManager,
        user_workspaces::UserWorkspaces,
        workspace::{
            AiCreditsUsageAndCostType, BillingCycleUsageSummary, MaxPriorCycles, UsageVisibility,
            Workspace,
        },
    },
};

const HEADER_FONT_SIZE: f32 = 16.;
const LEGEND_DOT_SIZE: f32 = 8.;

pub struct BillingCycleUsageSectionView {
    selected_period_end: Option<DateTime<Utc>>,
    refresh_button_mouse_state: MouseStateHandle,
    period_selector_mouse_state: MouseStateHandle,
    admin_panel_button_mouse_state: MouseStateHandle,
    period_menu: ViewHandle<Menu<BillingCycleUsageAction>>,
    period_menu_open: bool,
    source_filter: SourceFilter,
    row_mouse_states: RowMouseStates,
}

pub enum BillingCycleUsageEvent {}

#[derive(Clone, Debug)]
pub enum BillingCycleUsageAction {
    SelectPeriod(Option<DateTime<Utc>>),
    TogglePeriodMenu,
    Refresh,
    OpenAdminPanel,
    ChangeSourceFilter(SourceFilter),
}

impl Entity for BillingCycleUsageSectionView {
    type Event = BillingCycleUsageEvent;
}

impl BillingCycleUsageSectionView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        ctx.subscribe_to_model(&UserWorkspaces::handle(ctx), |me, _, _, ctx| {
            me.reconcile_selected_period(ctx);
            ctx.notify();
        });
        ctx.subscribe_to_model(&AIRequestUsageModel::handle(ctx), |_, _, _, ctx| {
            ctx.notify()
        });
        ctx.subscribe_to_model(&AuthManager::handle(ctx), |_, _, _, ctx| ctx.notify());
        ctx.subscribe_to_model(&TeamUpdateManager::handle(ctx), |_, _, _, ctx| ctx.notify());

        // `prevent_interaction_with_other_elements` so a click on the
        // trigger button while the menu is open is consumed by the menu's
        // outside-click dismiss handler — without it, the trigger also
        // received the click and immediately re-toggled the menu open.
        let period_menu = ctx.add_typed_action_view(|_| {
            Menu::new()
                .with_drop_shadow()
                .prevent_interaction_with_other_elements()
        });
        ctx.subscribe_to_view(&period_menu, |me, _, event, ctx| {
            if let menu::Event::Close { .. } = event {
                me.period_menu_open = false;
                ctx.notify();
            }
        });

        Self {
            selected_period_end: None,
            refresh_button_mouse_state: MouseStateHandle::default(),
            period_selector_mouse_state: MouseStateHandle::default(),
            admin_panel_button_mouse_state: MouseStateHandle::default(),
            period_menu,
            period_menu_open: false,
            source_filter: SourceFilter::default(),
            row_mouse_states: RowMouseStates::default(),
        }
    }

    fn resolved_viewer_email(app: &AppContext) -> Option<String> {
        AuthStateProvider::as_ref(app).get().user_email()
    }

    fn resolved_viewer_uid(app: &AppContext) -> Option<String> {
        AuthStateProvider::as_ref(app)
            .get()
            .user_id()
            .map(|uid| uid.as_string())
    }

    fn resolved_viewer_display_name(app: &AppContext) -> Option<String> {
        let state = AuthStateProvider::as_ref(app).get();
        state
            .display_name()
            .or_else(|| state.username_for_display())
            .or_else(|| state.user_email())
    }

    fn current_summary<'a>(
        &self,
        workspace: &'a Workspace,
    ) -> Option<&'a BillingCycleUsageSummary> {
        let data = workspace.billing_cycle_usage.as_ref()?;
        match self.selected_period_end {
            Some(end) => data.summaries.iter().find(|s| s.period_end == end),
            None => data.summaries.first(),
        }
    }

    fn reconcile_selected_period(&mut self, ctx: &AppContext) {
        let Some(selected) = self.selected_period_end else {
            return;
        };
        let still_present = UserWorkspaces::as_ref(ctx)
            .current_workspace()
            .and_then(|ws| ws.billing_cycle_usage.as_ref())
            .map(|data| data.summaries.iter().any(|s| s.period_end == selected))
            .unwrap_or(false);
        if !still_present {
            self.selected_period_end = None;
        }
    }
}

impl TypedActionView for BillingCycleUsageSectionView {
    type Action = BillingCycleUsageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            BillingCycleUsageAction::SelectPeriod(period_end) => {
                self.selected_period_end = *period_end;
                self.period_menu_open = false;
                ctx.notify();
            }
            BillingCycleUsageAction::TogglePeriodMenu => {
                self.period_menu_open = !self.period_menu_open;
                if self.period_menu_open {
                    self.refresh_period_menu_items(ctx);
                }
                ctx.notify();
            }
            BillingCycleUsageAction::Refresh => {
                std::mem::drop(
                    TeamUpdateManager::handle(ctx)
                        .update(ctx, |mgr, ctx| mgr.refresh_workspace_metadata(ctx)),
                );
                AIRequestUsageModel::handle(ctx)
                    .update(ctx, |m, ctx| m.refresh_request_usage_async(ctx));
            }
            BillingCycleUsageAction::OpenAdminPanel => {
                if let Some(team_uid) = UserWorkspaces::as_ref(ctx).current_team_uid() {
                    AdminActions::open_admin_panel(team_uid, ctx);
                }
            }
            BillingCycleUsageAction::ChangeSourceFilter(filter) => {
                self.source_filter = *filter;
                ctx.notify();
            }
        }
    }
}

impl BillingCycleUsageSectionView {
    fn refresh_period_menu_items(&self, ctx: &mut ViewContext<Self>) {
        let Some(workspace) = UserWorkspaces::as_ref(ctx).current_workspace().cloned() else {
            return;
        };
        let Some(data) = workspace.billing_cycle_usage.as_ref() else {
            return;
        };
        let items: Vec<MenuItem<BillingCycleUsageAction>> = data
            .summaries
            .iter()
            .map(|summary| {
                let label = format_period_label(summary);
                MenuItem::Item(MenuItemFields::new(label).with_on_select_action(
                    BillingCycleUsageAction::SelectPeriod(Some(summary.period_end)),
                ))
            })
            .collect();

        self.period_menu
            .update(ctx, |menu: &mut Menu<BillingCycleUsageAction>, ctx| {
                menu.set_items(items, ctx);
            });
    }
}

impl View for BillingCycleUsageSectionView {
    fn ui_name() -> &'static str {
        "BillingCycleUsageSection"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let Some(workspace) = UserWorkspaces::as_ref(app).current_workspace().cloned() else {
            return Empty::new().finish();
        };
        let viewer_email = Self::resolved_viewer_email(app);
        let is_admin = viewer_email
            .as_deref()
            .is_some_and(|email| workspace.is_workspace_admin(email));
        let visibility = workspace.resolve_usage_visibility(is_admin);

        // Enterprise admins can set detailed usage limits in the admin panel that we're not going to support in the client for rn
        if workspace.billing_metadata.is_enterprise_plan() && is_admin {
            return self.render_admin_panel_cta(appearance);
        }

        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        column.add_child(self.render_header(&workspace, &visibility, appearance, app));

        if let Some(legend) = self.render_legend(&workspace, appearance) {
            column.add_child(Container::new(legend).with_margin_top(8.).finish());
        }

        column.add_child(
            Container::new(self.render_body(&workspace, &visibility, appearance, app))
                .with_margin_top(16.)
                .finish(),
        );

        column.finish()
    }
}

impl BillingCycleUsageSectionView {
    fn render_header(
        &self,
        workspace: &Workspace,
        visibility: &UsageVisibility,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);

        row.add_child(
            Text::new_inline("Usage", appearance.ui_font_family(), HEADER_FONT_SIZE)
                .with_style(Properties::default().weight(Weight::Bold))
                .with_color(appearance.theme().active_ui_text_color().into())
                .finish(),
        );

        let mut right_side = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End);

        let summary_count = workspace
            .billing_cycle_usage
            .as_ref()
            .map(|d| d.summaries.len())
            .unwrap_or(0);
        let has_multiple_periods =
            summary_count >= 2 && visibility.max_prior_cycles != MaxPriorCycles::None;
        if has_multiple_periods {
            right_side.add_child(
                Container::new(self.render_period_selector(workspace, appearance))
                    .with_margin_right(12.)
                    .finish(),
            );
        } else {
            right_side.add_child(
                Container::new(self.render_resets_label(workspace, appearance, app))
                    .with_margin_right(12.)
                    .finish(),
            );
        }

        right_side.add_child(self.render_refresh_button(appearance));

        row.add_child(right_side.finish());

        Container::new(row.finish())
            .with_margin_bottom(12.)
            .finish()
    }

    fn render_resets_label(
        &self,
        workspace: &Workspace,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let label = match self.selected_period_end {
            Some(_) => self
                .current_summary(workspace)
                .map(format_period_label)
                .unwrap_or_default(),
            None => AIRequestUsageModel::as_ref(app)
                .next_refresh_time_local()
                .format("Resets %b %d at %-I:%M %p")
                .to_string(),
        };
        Text::new_inline(
            label,
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(theme.sub_text_color(theme.background()).into())
        .finish()
    }

    fn render_refresh_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let icon_color = theme.sub_text_color(theme.background());
        let mouse_state = self.refresh_button_mouse_state.clone();
        Hoverable::new(mouse_state, move |_| {
            Container::new(
                ConstrainedBox::new(Icon::Refresh.to_warpui_icon(icon_color).finish())
                    .with_width(16.)
                    .with_height(16.)
                    .finish(),
            )
            .with_uniform_padding(2.)
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(BillingCycleUsageAction::Refresh);
        })
        .finish()
    }

    fn render_period_selector(
        &self,
        workspace: &Workspace,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let bg = theme.background();
        let label = match self.selected_period_end {
            Some(_) => self
                .current_summary(workspace)
                .map(format_period_label)
                .unwrap_or_default(),
            None => workspace
                .billing_cycle_usage
                .as_ref()
                .and_then(|d| d.summaries.first())
                .map(format_period_label)
                .unwrap_or_default(),
        };

        let mouse_state = self.period_selector_mouse_state.clone();
        let font_family = appearance.ui_font_family();
        let font_size = appearance.ui_font_size();
        let main_text = theme.sub_text_color(bg);

        let button = Hoverable::new(mouse_state, move |_| {
            let mut inner = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);
            inner.add_child(
                Text::new_inline(label.clone(), font_family, font_size)
                    .with_color(main_text.into())
                    .finish(),
            );
            inner.add_child(
                Container::new(
                    ConstrainedBox::new(Icon::ChevronDown.to_warpui_icon(main_text).finish())
                        .with_width(12.)
                        .with_height(12.)
                        .finish(),
                )
                .with_margin_left(4.)
                .finish(),
            );
            inner.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(BillingCycleUsageAction::TogglePeriodMenu);
        })
        .finish();

        let mut stack = Stack::new();
        stack.add_child(button);
        if self.period_menu_open {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.period_menu).finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(0., 4.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }
        stack.finish()
    }

    fn render_legend(
        &self,
        workspace: &Workspace,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        let summary = self.current_summary(workspace)?;
        if summary.entries.is_empty() {
            return None;
        }

        let mut present_buckets = Vec::new();
        for cost_type in [
            AiCreditsUsageAndCostType::BaseLimit,
            AiCreditsUsageAndCostType::BonusGrant,
            AiCreditsUsageAndCostType::Payg,
            AiCreditsUsageAndCostType::AmbientBonusGrant,
            AiCreditsUsageAndCostType::Aggregate,
        ] {
            if summary.entries.iter().any(|e| e.cost_type == cost_type) {
                present_buckets.push(cost_type);
            }
        }
        if present_buckets.is_empty() {
            return None;
        }

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        for (idx, bucket) in present_buckets.iter().enumerate() {
            if idx > 0 {
                row.add_child(
                    Container::new(Empty::new().finish())
                        .with_margin_right(12.)
                        .finish(),
                );
            }
            row.add_child(self.render_legend_entry(bucket.clone(), appearance));
        }
        Some(row.finish())
    }

    fn render_legend_entry(
        &self,
        cost_type: AiCreditsUsageAndCostType,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let (color, label) = legend_style_for(cost_type);
        let theme = appearance.theme();
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        row.add_child(
            ConstrainedBox::new(
                Container::new(Empty::new().finish())
                    .with_background_color(color)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                        LEGEND_DOT_SIZE / 2.,
                    )))
                    .finish(),
            )
            .with_height(LEGEND_DOT_SIZE)
            .with_width(LEGEND_DOT_SIZE)
            .finish(),
        );
        row.add_child(
            Container::new(
                Text::new_inline(
                    label,
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(theme.sub_text_color(theme.background()).into())
                .finish(),
            )
            .with_margin_left(6.)
            .finish(),
        );
        row.finish()
    }

    fn render_body(
        &self,
        workspace: &Workspace,
        visibility: &UsageVisibility,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let Some(summary) = self.current_summary(workspace) else {
            return self.render_empty_state(
                "No usage this period",
                "Usage will appear here once you start making agent requests.",
                appearance,
            );
        };

        let viewer_uid = Self::resolved_viewer_uid(app);
        let viewer_display_name = Self::resolved_viewer_display_name(app);

        render_rows(
            workspace,
            &summary.entries,
            viewer_uid.as_deref(),
            viewer_display_name.as_deref(),
            visibility,
            self.source_filter,
            &self.row_mouse_states,
            appearance,
            std::sync::Arc::new(|filter, ctx| {
                ctx.dispatch_typed_action(BillingCycleUsageAction::ChangeSourceFilter(filter));
            }),
        )
    }

    fn render_admin_panel_cta(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let bg = theme.background();

        let header = Text::new_inline(
            "View detailed usage in the admin panel",
            appearance.ui_font_family(),
            14.,
        )
        .with_color(theme.active_ui_text_color().into())
        .with_style(Properties::default().weight(Weight::Medium))
        .finish();

        let body = appearance
            .ui_builder()
            .paragraph("Detailed team usage and spend limit controls live in the admin panel.")
            .with_style(UiComponentStyles {
                font_color: Some(theme.sub_text_color(bg).into()),
                ..Default::default()
            })
            .build()
            .finish();

        let fg = theme.active_ui_text_color();
        let button = appearance
            .ui_builder()
            .button(
                ButtonVariant::Secondary,
                self.admin_panel_button_mouse_state.clone(),
            )
            .with_text_label("Open admin panel".to_string())
            .with_style(UiComponentStyles {
                font_color: Some(fg.into()),
                ..Default::default()
            })
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(BillingCycleUsageAction::OpenAdminPanel);
            })
            .finish();

        let column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_children([
                Container::new(header).with_margin_bottom(8.).finish(),
                Container::new(body).with_margin_bottom(12.).finish(),
                button,
            ])
            .finish();

        Container::new(column)
            .with_background_color(theme.surface_1().into_solid())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .with_vertical_padding(40.)
            .with_uniform_padding(16.)
            .finish()
    }

    fn render_empty_state(
        &self,
        title: &str,
        subtitle: &str,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let bg = theme.background();
        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center);
        col.add_child(
            Container::new(
                Text::new_inline(title.to_string(), appearance.ui_font_family(), 14.)
                    .with_color(theme.active_ui_text_color().into())
                    .with_style(Properties::default().weight(Weight::Medium))
                    .finish(),
            )
            .with_margin_bottom(4.)
            .finish(),
        );
        col.add_child(
            Text::new_inline(
                subtitle.to_string(),
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .with_color(theme.sub_text_color(bg).into())
            .finish(),
        );

        Container::new(col.finish())
            .with_background_color(theme.surface_1().into_solid())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .with_vertical_padding(40.)
            .with_uniform_padding(16.)
            .finish()
    }
}

fn legend_style_for(cost_type: AiCreditsUsageAndCostType) -> (ColorU, &'static str) {
    match cost_type {
        AiCreditsUsageAndCostType::BaseLimit => (BASE_CREDITS_DOT_COLOR, "Base"),
        AiCreditsUsageAndCostType::BonusGrant => (BONUS_CREDITS_DOT_COLOR, "Add-ons"),
        AiCreditsUsageAndCostType::Payg => (PAYG_CREDITS_DOT_COLOR, "Pay-as-you-go"),
        AiCreditsUsageAndCostType::AmbientBonusGrant => (AMBIENT_CREDITS_DOT_COLOR, "Ambient-only"),
        AiCreditsUsageAndCostType::Aggregate => (AGGREGATE_CREDITS_DOT_COLOR, "Aggregated"),
        AiCreditsUsageAndCostType::Other(_) => (BASE_CREDITS_DOT_COLOR, ""),
    }
}

fn format_period_label(summary: &BillingCycleUsageSummary) -> String {
    let start = summary.period_start.with_timezone(&Local);
    let end = summary.period_end.with_timezone(&Local);
    format!(
        "{} - {}",
        start.format("%b %d, %Y"),
        end.format("%b %d, %Y")
    )
}
