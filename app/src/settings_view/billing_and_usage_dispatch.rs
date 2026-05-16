//! Dispatch wrapper that routes between the legacy and v2 billing & usage
//! pages.
//!
//! The dispatcher is what the Settings framework treats as the
//! "Billing and Usage" page. It owns a single [`PageType::Monolith`] whose
//! sole [`SettingsWidget`] picks between the v1 and v2 inner pages at
//! render time via [`ChildView`]. The v1/v2 inner views are kept as plain
//! `View`s and only render their content (no chrome of their own).
//!
//! Concretely this means both v1 and v2 share:
//! - The same outer vertical (and narrow-pane dual-axis) scrollable.
//! - The same `PAGE_PADDING` and `MAX_PAGE_WIDTH` page chrome.
//! - One union set of search terms — typing any of
//!   `plan|billing|ai|usage|limit|credits|balance|overview` surfaces the
//!   page in the sidebar regardless of which inner page is active.
//! - One stable `widget_id` for any future scroll-to-widget callers.
//!
//! Eligibility for v2: the feature flag is on AND the current workspace is
//! on a Build / Build Max / Build Business / Enterprise plan. Everyone else
//! (Free, legacy paid plans, anonymous) falls back to v1.

use warp_core::{features::FeatureFlag, ui::appearance::Appearance};
use warpui::elements::ChildView;
use warpui::{AppContext, Element, Entity, SingletonEntity, View, ViewContext, ViewHandle};

use crate::auth::AuthStateProvider;
use crate::workspaces::user_workspaces::UserWorkspaces;

use super::billing_and_usage_page::{BillingAndUsagePageEvent, BillingAndUsagePageView};
use super::billing_and_usage_page_v2::BillingAndUsagePageV2View;
use super::settings_page::{
    MatchData, PageType, SettingsPageMeta, SettingsPageViewHandle, SettingsWidget,
};
use super::SettingsSection;

pub struct BillingAndUsageDispatchView {
    page: PageType<Self>,
    v1: ViewHandle<BillingAndUsagePageView>,
    v2: ViewHandle<BillingAndUsagePageV2View>,
}

impl BillingAndUsageDispatchView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let v1 = ctx.add_typed_action_view(BillingAndUsagePageView::new);
        let v2 = ctx.add_typed_action_view(BillingAndUsagePageV2View::new);

        // Re-emit events from either inner view as our own so SettingsView
        // only has to subscribe to the dispatcher.
        ctx.subscribe_to_view(&v1, |_, _, event, ctx| {
            ctx.emit(event.clone());
        });
        ctx.subscribe_to_view(&v2, |_, _, event, ctx| {
            ctx.emit(event.clone());
        });

        // Monolith with a single routing widget. `is_dual_scrollable: true`
        // gives us a vertical scrollable always and a horizontal one when
        // the pane is narrower than `MIN_PAGE_WIDTH`, matching every other
        // settings page.
        let page = PageType::new_monolith(BillingAndUsageWidget, None, true);

        Self { page, v1, v2 }
    }

    /// Whether the v2 page should be used for the current viewer.
    ///
    /// `true` when the v2 feature flag is on AND the current workspace is
    /// on a Build family or Enterprise plan. Re-evaluated on every render
    /// so plan/flag changes flip the active page automatically.
    fn use_v2(&self, ctx: &AppContext) -> bool {
        if !FeatureFlag::BillingAndUsagePageV2.is_enabled() {
            return false;
        }
        UserWorkspaces::as_ref(ctx)
            .current_workspace()
            .is_some_and(|workspace| {
                let bm = &workspace.billing_metadata;
                bm.is_on_build_plan()
                    || bm.is_on_build_max_plan()
                    || bm.is_on_build_business_plan()
                    || bm.is_enterprise_plan()
            })
    }

    /// Returns the modal element to render on top of whichever inner page
    /// is currently active, if any.
    pub fn get_modal_content(&self, app: &AppContext) -> Option<Box<dyn Element>> {
        if self.use_v2(app) {
            self.v2.read(app, |view, _| view.get_modal_content())
        } else {
            self.v1.read(app, |view, _| view.get_modal_content())
        }
    }
}

impl Entity for BillingAndUsageDispatchView {
    type Event = BillingAndUsagePageEvent;
}

impl View for BillingAndUsageDispatchView {
    fn ui_name() -> &'static str {
        "Billing and usage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for BillingAndUsageDispatchView {
    fn section() -> SettingsSection {
        SettingsSection::BillingAndUsage
    }

    fn should_render(&self, ctx: &AppContext) -> bool {
        // Visible for any non-anonymous user; the active inner page is
        // chosen at render time by `use_v2`.
        !AuthStateProvider::as_ref(ctx)
            .get()
            .is_anonymous_or_logged_out()
    }

    fn on_page_selected(&mut self, allow_steal_focus: bool, ctx: &mut ViewContext<Self>) {
        if self.use_v2(ctx) {
            self.v2
                .update(ctx, |view, ctx| view.on_page_selected(allow_steal_focus, ctx));
        } else {
            self.v1
                .update(ctx, |view, ctx| view.on_page_selected(allow_steal_focus, ctx));
        }
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id);
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<BillingAndUsageDispatchView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<BillingAndUsageDispatchView>) -> Self {
        SettingsPageViewHandle::BillingAndUsage(view_handle)
    }
}

/// Single widget on the dispatcher's [`PageType::Monolith`]. Its `render`
/// chooses between the v1 and v2 inner pages, and its `search_terms` is
/// the union of v1's two original widgets' terms plus v2-specific
/// terminology — so search-discoverability of the page is independent of
/// which inner view is active.
#[derive(Default)]
struct BillingAndUsageWidget;

impl SettingsWidget for BillingAndUsageWidget {
    type View = BillingAndUsageDispatchView;

    fn search_terms(&self) -> &str {
        "plan billing a.i. ai usage limit credits balance overview"
    }

    fn render(
        &self,
        view: &Self::View,
        _appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        if view.use_v2(app) {
            ChildView::new(&view.v2).finish()
        } else {
            ChildView::new(&view.v1).finish()
        }
    }
}
