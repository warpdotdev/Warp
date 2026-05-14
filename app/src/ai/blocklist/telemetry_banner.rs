use warpui::AppContext;

/// OpenWarp has no telemetry sender and must not collect user-generated AI data.
pub fn should_collect_ai_ugc_telemetry(_app: &AppContext, _is_telemetry_enabled: bool) -> bool {
    false
}
