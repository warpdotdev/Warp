#[derive(Debug)]
#[cfg_attr(not(windows), expect(dead_code))]
pub(super) enum AntivirusInfoTelemetryEvent {
    AntivirusDetected { name: String },
}
