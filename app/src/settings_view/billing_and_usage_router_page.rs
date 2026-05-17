use crate::{
    auth::AuthStateProvider,
    workspaces::user_workspaces::{UserWorkspaces, UserWorkspacesEvent},
};
use warp_core::features::FeatureFlag;
use warpui::{
    elements::{ChildView, Empty},
    AppContext, Element, Entity, SingletonEntity, TypedActionView, UpdateView, View, ViewContext,
    ViewHandle,
};

use super::{
    billing_and_usage_page::{
        BillingAndUsagePageAction, BillingAndUsagePageEvent, BillingAndUsagePageView,
    },
    billing_and_usage_page_v2::BillingAndUsagePageV2View,
    settings_page::{MatchData, SettingsPageMeta, SettingsPageViewHandle},
    SettingsSection,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BillingAndUsageRoute {
    V1,
    V2,
}

impl BillingAndUsageRoute {
    fn from_state(v2_flag_enabled: bool, is_build_plan: bool) -> Self {
        if v2_flag_enabled && is_build_plan {
            Self::V2
        } else {
            Self::V1
        }
    }

    fn current(ctx: &AppContext) -> Self {
        Self::from_state(
            FeatureFlag::BillingAndUsagePageV2.is_enabled(),
            UserWorkspaces::as_ref(ctx)
                .current_workspace()
                .is_some_and(|workspace| workspace.billing_metadata.is_on_build_plan()),
        )
    }
}

enum ActiveBillingAndUsagePage {
    V1(ViewHandle<BillingAndUsagePageView>),
    V2(ViewHandle<BillingAndUsagePageV2View>),
}

impl ActiveBillingAndUsagePage {
    fn deactivate(self, ctx: &mut ViewContext<BillingAndUsageRouterPageView>) {
        match self {
            Self::V1(handle) => {
                ctx.unsubscribe_to_view(&handle);
                ctx.update_view(&handle, |view, ctx| {
                    view.deactivate_subscriptions(ctx);
                });
            }
            Self::V2(handle) => {
                ctx.unsubscribe_to_view(&handle);
                ctx.update_view(&handle, |view, ctx| {
                    view.deactivate_subscriptions(ctx);
                });
            }
        }
    }

    fn on_page_selected(
        &self,
        allow_steal_focus: bool,
        ctx: &mut ViewContext<BillingAndUsageRouterPageView>,
    ) {
        match self {
            Self::V1(handle) => {
                ctx.update_view(handle, |view, ctx| {
                    view.on_page_selected(allow_steal_focus, ctx);
                });
            }
            Self::V2(handle) => {
                ctx.update_view(handle, |view, ctx| {
                    view.on_page_selected(allow_steal_focus, ctx);
                });
            }
        }
    }

    fn update_filter(
        &self,
        query: &str,
        ctx: &mut ViewContext<BillingAndUsageRouterPageView>,
    ) -> MatchData {
        match self {
            Self::V1(handle) => ctx.update_view(handle, |view, ctx| view.update_filter(query, ctx)),
            Self::V2(handle) => ctx.update_view(handle, |view, ctx| view.update_filter(query, ctx)),
        }
    }

    fn scroll_to_widget(
        &self,
        widget_id: &'static str,
        ctx: &mut ViewContext<BillingAndUsageRouterPageView>,
    ) {
        match self {
            Self::V1(handle) => {
                ctx.update_view(handle, |view, ctx| view.scroll_to_widget(widget_id, ctx));
            }
            Self::V2(handle) => {
                ctx.update_view(handle, |view, ctx| view.scroll_to_widget(widget_id, ctx));
            }
        }
    }

    fn clear_highlighted_widget(&self, ctx: &mut ViewContext<BillingAndUsageRouterPageView>) {
        match self {
            Self::V1(handle) => {
                ctx.update_view(handle, |view, ctx| view.clear_highlighted_widget(ctx));
            }
            Self::V2(handle) => {
                ctx.update_view(handle, |view, ctx| view.clear_highlighted_widget(ctx));
            }
        }
    }

    fn handle_action(
        &self,
        action: &BillingAndUsagePageAction,
        ctx: &mut ViewContext<BillingAndUsageRouterPageView>,
    ) {
        match self {
            Self::V1(handle) => {
                ctx.update_view(handle, |view, ctx| view.handle_action(action, ctx));
            }
            Self::V2(handle) => {
                ctx.update_view(handle, |view, ctx| view.handle_action(action, ctx));
            }
        }
    }

    fn child_view(&self) -> Box<dyn Element> {
        match self {
            Self::V1(handle) => ChildView::new(handle).finish(),
            Self::V2(handle) => ChildView::new(handle).finish(),
        }
    }

    fn get_modal_content(&self, app: &AppContext) -> Option<Box<dyn Element>> {
        match self {
            Self::V1(handle) => handle.read(app, |view, _| view.get_modal_content()),
            Self::V2(handle) => handle.read(app, |view, _| view.get_modal_content()),
        }
    }
}

pub struct BillingAndUsageRouterPageView {
    active_route: Option<BillingAndUsageRoute>,
    active_page: Option<ActiveBillingAndUsagePage>,
}

impl BillingAndUsageRouterPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let mut me = Self {
            active_route: None,
            active_page: None,
        };
        me.refresh_active_page(ctx);
        ctx.subscribe_to_model(&UserWorkspaces::handle(ctx), |me, _, event, ctx| {
            if matches!(event, UserWorkspacesEvent::TeamsChanged) {
                me.refresh_active_page(ctx);
            }
            ctx.notify();
        });
        me
    }

    pub fn get_modal_content(&self, app: &AppContext) -> Option<Box<dyn Element>> {
        self.active_page
            .as_ref()
            .and_then(|page| page.get_modal_content(app))
    }

    fn refresh_active_page(&mut self, ctx: &mut ViewContext<Self>) {
        let route = BillingAndUsageRoute::current(ctx);
        if self.active_route == Some(route) {
            return;
        }

        if let Some(active_page) = self.active_page.take() {
            active_page.deactivate(ctx);
        }

        self.active_page = Some(match route {
            BillingAndUsageRoute::V1 => {
                let handle = ctx.add_typed_action_view(BillingAndUsagePageView::new);
                ctx.subscribe_to_view(&handle, |_, _, event, ctx| {
                    ctx.emit(event.clone());
                });
                ActiveBillingAndUsagePage::V1(handle)
            }
            BillingAndUsageRoute::V2 => {
                let handle = ctx.add_typed_action_view(BillingAndUsagePageV2View::new);
                ctx.subscribe_to_view(&handle, |_, _, event, ctx| {
                    ctx.emit(event.clone());
                });
                ActiveBillingAndUsagePage::V2(handle)
            }
        });
        self.active_route = Some(route);
        ctx.notify();
    }
}

impl SettingsPageMeta for BillingAndUsageRouterPageView {
    fn section() -> SettingsSection {
        SettingsSection::BillingAndUsage
    }

    fn should_render(&self, ctx: &AppContext) -> bool {
        !AuthStateProvider::as_ref(ctx)
            .get()
            .is_anonymous_or_logged_out()
    }

    fn on_page_selected(&mut self, allow_steal_focus: bool, ctx: &mut ViewContext<Self>) {
        self.refresh_active_page(ctx);
        if let Some(active_page) = &self.active_page {
            active_page.on_page_selected(allow_steal_focus, ctx);
        }
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.refresh_active_page(ctx);
        self.active_page
            .as_ref()
            .map(|page| page.update_filter(query, ctx))
            .unwrap_or(MatchData::Uncounted(false))
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str, ctx: &mut ViewContext<Self>) {
        self.refresh_active_page(ctx);
        if let Some(active_page) = &self.active_page {
            active_page.scroll_to_widget(widget_id, ctx);
        }
    }

    fn clear_highlighted_widget(&mut self, ctx: &mut ViewContext<Self>) {
        self.refresh_active_page(ctx);
        if let Some(active_page) = &self.active_page {
            active_page.clear_highlighted_widget(ctx);
        }
    }
}

impl Entity for BillingAndUsageRouterPageView {
    type Event = BillingAndUsagePageEvent;
}

impl View for BillingAndUsageRouterPageView {
    fn ui_name() -> &'static str {
        "Billing and usage router"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        self.active_page
            .as_ref()
            .map(ActiveBillingAndUsagePage::child_view)
            .unwrap_or_else(|| Empty::new().finish())
    }
}

impl TypedActionView for BillingAndUsageRouterPageView {
    type Action = BillingAndUsagePageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        self.refresh_active_page(ctx);
        if let Some(active_page) = &self.active_page {
            active_page.handle_action(action, ctx);
        }
    }
}

impl From<ViewHandle<BillingAndUsageRouterPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<BillingAndUsageRouterPageView>) -> Self {
        SettingsPageViewHandle::BillingAndUsageRouter(view_handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_to_v1_when_v2_flag_is_disabled() {
        assert_eq!(
            BillingAndUsageRoute::from_state(false, true),
            BillingAndUsageRoute::V1
        );
    }

    #[test]
    fn routes_to_v1_when_v2_flag_is_enabled_for_non_build_plan() {
        assert_eq!(
            BillingAndUsageRoute::from_state(true, false),
            BillingAndUsageRoute::V1
        );
    }

    #[test]
    fn routes_to_v2_when_v2_flag_is_enabled_for_build_plan() {
        assert_eq!(
            BillingAndUsageRoute::from_state(true, true),
            BillingAndUsageRoute::V2
        );
    }
}
