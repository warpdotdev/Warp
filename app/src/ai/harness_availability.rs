use serde::{Deserialize, Serialize};
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;
use warp_core::user_preferences::GetUserPreferences;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::harness_display;
use crate::auth::auth_manager::{AuthManager, AuthManagerEvent};
use crate::auth::AuthStateProvider;
use crate::network::{NetworkStatus, NetworkStatusEvent, NetworkStatusKind};
use crate::report_error;
use crate::server::server_api::ServerApiProvider;
use crate::workspaces::user_workspaces::{UserWorkspaces, UserWorkspacesEvent};

const CACHE_KEY: &str = "AvailableHarnesses";

/// Server-resolved harness availability entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarnessAvailability {
    pub harness: Harness,
    pub display_name: String,
    pub enabled: bool,
}

/// Default fallback used before the server responds.
/// Oz is enabled by default so the UI is usable pre-fetch; the server
/// list (which respects admin overrides) replaces this once available.
fn default_harnesses() -> Vec<HarnessAvailability> {
    vec![HarnessAvailability {
        harness: Harness::Oz,
        display_name: "Warp".to_string(),
        enabled: true,
    }]
}

pub enum HarnessAvailabilityEvent {
    Changed,
}

pub struct HarnessAvailabilityModel {
    harnesses: Vec<HarnessAvailability>,
}

impl HarnessAvailabilityModel {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let harnesses = get_cached(ctx).unwrap_or_else(default_harnesses);

        ctx.subscribe_to_model(&NetworkStatus::handle(ctx), |me, event, ctx| {
            if let NetworkStatusEvent::NetworkStatusChanged {
                new_status: NetworkStatusKind::Online,
            } = event
            {
                me.refresh(ctx);
            }
        });

        ctx.subscribe_to_model(&AuthManager::handle(ctx), |me, event, ctx| {
            if let AuthManagerEvent::AuthComplete = event {
                me.refresh(ctx);
            }
        });

        ctx.subscribe_to_model(&UserWorkspaces::handle(ctx), |me, event, ctx| {
            if let UserWorkspacesEvent::TeamsChanged = event {
                me.refresh(ctx);
            }
        });

        let me = Self { harnesses };
        me.refresh(ctx);
        me
    }

    pub fn available_harnesses(&self) -> &[HarnessAvailability] {
        &self.harnesses
    }

    pub fn display_name_for(&self, harness: Harness) -> &str {
        self.harnesses
            .iter()
            .find(|h| h.harness == harness)
            .map(|h| h.display_name.as_str())
            .unwrap_or_else(|| harness_display::display_name(harness))
    }

    /// Whether the harness selector should be shown (>1 known harness, including disabled).
    pub fn should_show_harness_selector(&self) -> bool {
        FeatureFlag::AgentHarness.is_enabled() && self.harnesses.len() > 1
    }

    /// Whether any harness is available at all (at least one enabled).
    pub fn has_any_enabled_harness(&self) -> bool {
        self.harnesses.iter().any(|h| h.enabled)
    }

    /// Whether a harness is both known and enabled.
    pub fn is_harness_enabled(&self, harness: Harness) -> bool {
        self.harnesses
            .iter()
            .any(|h| h.harness == harness && h.enabled)
    }

    pub fn refresh(&self, ctx: &mut ModelContext<Self>) {
        // The endpoint queries `user`, which requires auth.
        if !AuthStateProvider::as_ref(ctx).get().is_logged_in() {
            return;
        }

        let ai_client = ServerApiProvider::as_ref(ctx).get_ai_client();
        ctx.spawn(
            async move { ai_client.get_available_harnesses().await },
            |me, result, ctx| match result {
                Ok(new_harnesses) => {
                    if new_harnesses != me.harnesses {
                        me.harnesses = new_harnesses;
                        me.cache(ctx);
                        ctx.emit(HarnessAvailabilityEvent::Changed);
                    }
                }
                Err(e) => {
                    report_error!(e.context("Failed to fetch available harnesses"));
                }
            },
        );
    }

    fn cache(&self, ctx: &ModelContext<Self>) {
        if let Ok(serialized) = serde_json::to_string(&self.harnesses) {
            if let Err(e) = ctx
                .private_user_preferences()
                .write_value(CACHE_KEY, serialized)
            {
                report_error!(anyhow::anyhow!(e).context("Failed to cache available harnesses"));
            }
        }
    }
}

fn get_cached(ctx: &ModelContext<HarnessAvailabilityModel>) -> Option<Vec<HarnessAvailability>> {
    let raw = ctx
        .private_user_preferences()
        .read_value(CACHE_KEY)
        .ok()??;
    serde_json::from_str::<Vec<HarnessAvailability>>(&raw).ok()
}

impl Entity for HarnessAvailabilityModel {
    type Event = HarnessAvailabilityEvent;
}

impl SingletonEntity for HarnessAvailabilityModel {}
