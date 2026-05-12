use warpui::{elements::Empty, AppContext, Element, Entity, SingletonEntity, View, ViewContext};

use crate::auth::AuthStateProvider;

use super::{
    settings_page::{MatchData, SettingsPageMeta, SettingsPageViewHandle},
    SettingsSection,
};

pub use super::billing_and_usage_page::BillingAndUsagePageEvent;

pub struct BillingAndUsagePageV2View;

impl BillingAndUsagePageV2View {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self
    }
}

impl SettingsPageMeta for BillingAndUsagePageV2View {
    fn section() -> SettingsSection {
        SettingsSection::BillingAndUsage
    }

    fn should_render(&self, ctx: &AppContext) -> bool {
        let is_anonymous = AuthStateProvider::as_ref(ctx)
            .get()
            .is_anonymous_or_logged_out();

        !is_anonymous
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

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}

impl From<warpui::ViewHandle<BillingAndUsagePageV2View>> for SettingsPageViewHandle {
    fn from(view_handle: warpui::ViewHandle<BillingAndUsagePageV2View>) -> Self {
        SettingsPageViewHandle::BillingAndUsageV2(view_handle)
    }
}
