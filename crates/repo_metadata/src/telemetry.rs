#[allow(dead_code)]
#[derive(Clone)]
pub enum RepoMetadataTelemetryEvent {
    BuildTreeFailed { error: String },
}
