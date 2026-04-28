use warp_core::telemetry::{TelemetryContextModel, TelemetryContextProvider};
use warpui::{AppContext, ModelContext, SingletonEntity};

use crate::auth::AuthStateProvider;

pub struct AppTelemetryContextProvider {}

impl AppTelemetryContextProvider {
    pub fn new_context_provider(
        _ctx: &mut ModelContext<TelemetryContextModel>,
    ) -> TelemetryContextModel {
        Box::new(Self {})
    }
}

impl TelemetryContextProvider for AppTelemetryContextProvider {
    fn user_id(&self, ctx: &AppContext) -> Option<String> {
        let auth_state = AuthStateProvider::as_ref(ctx).get();
        auth_state.user_id().map(|uid| uid.as_string())
    }

    fn anonymous_id(&self, ctx: &AppContext) -> String {
        let auth_state = AuthStateProvider::as_ref(ctx).get();
        auth_state.anonymous_id()
    }
}

/// A no-op telemetry context provider for headless contexts (e.g. the remote
/// server daemon) that run without authentication. Telemetry events that
/// require a user/anonymous ID will silently produce empty identifiers,
/// preventing panics from an unregistered `TelemetryContextModel` singleton.
#[cfg_attr(target_family = "wasm", allow(dead_code))]
pub struct NoopTelemetryContextProvider;

impl NoopTelemetryContextProvider {
    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    pub fn new_context_provider(
        _ctx: &mut ModelContext<TelemetryContextModel>,
    ) -> TelemetryContextModel {
        Box::new(Self)
    }
}

impl TelemetryContextProvider for NoopTelemetryContextProvider {
    fn user_id(&self, _ctx: &AppContext) -> Option<String> {
        None
    }

    fn anonymous_id(&self, _ctx: &AppContext) -> String {
        String::new()
    }
}
