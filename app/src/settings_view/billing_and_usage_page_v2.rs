use warpui::{
    elements::Empty,
    AppContext, Element, Entity, View, ViewContext,
};

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

    fn should_render(&self, _ctx: &AppContext) -> bool {
        true
    }

    fn update_filter(&mut self, _query: &str, _ctx: &mut ViewContext<Self>) -> MatchData {
        MatchData::Uncounted(true)
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
