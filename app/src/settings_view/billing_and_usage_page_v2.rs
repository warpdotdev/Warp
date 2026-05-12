//! V2 rendering for the Billing & Usage settings page.
//!
//! Gated behind `FeatureFlag::BillingAndUsagePageV2`. When the flag is enabled,
//! the page-level `render` method delegates here instead of the v1 widget-based
//! layout.

use warpui::{elements::Empty, AppContext, Element};

use super::BillingAndUsagePageView;

/// Top-level v2 render for the entire Billing & Usage page.
pub fn render_page(
    _view: &BillingAndUsagePageView,
    _app: &AppContext,
) -> Box<dyn Element> {
    Empty::new().finish()
}
