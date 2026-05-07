//! Conversion between remote codebase indexing domain types and proto-generated types.

use crate::proto;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteCodebaseIndexStatus {
    pub repo_path: String,
    pub state: RemoteCodebaseIndexState,
    pub last_updated_epoch_millis: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteCodebaseIndexState {
    NotEnabled,
    Unavailable,
}

// ── Rust → Proto ────────────────────────────────────────────

impl From<&RemoteCodebaseIndexStatus> for proto::CodebaseIndexStatus {
    fn from(status: &RemoteCodebaseIndexStatus) -> Self {
        Self {
            repo_path: status.repo_path.clone(),
            state: proto_state(status.state) as i32,
            last_updated_epoch_millis: status.last_updated_epoch_millis,
        }
    }
}

pub fn statuses_to_snapshot_proto<'a>(
    statuses: impl IntoIterator<Item = &'a RemoteCodebaseIndexStatus>,
) -> proto::CodebaseIndexStatusesSnapshot {
    proto::CodebaseIndexStatusesSnapshot {
        statuses: statuses
            .into_iter()
            .map(proto::CodebaseIndexStatus::from)
            .collect(),
    }
}

fn proto_state(state: RemoteCodebaseIndexState) -> proto::CodebaseIndexStatusState {
    match state {
        RemoteCodebaseIndexState::NotEnabled => proto::CodebaseIndexStatusState::NotEnabled,
        RemoteCodebaseIndexState::Unavailable => proto::CodebaseIndexStatusState::Unavailable,
    }
}

// ── Proto → Rust ──────────────────────────────────────────────────

pub fn proto_to_codebase_index_status(
    status: &proto::CodebaseIndexStatus,
) -> Option<RemoteCodebaseIndexStatus> {
    Some(RemoteCodebaseIndexStatus {
        repo_path: status.repo_path.clone(),
        state: proto_to_state(proto::CodebaseIndexStatusState::try_from(status.state).ok()?)?,
        last_updated_epoch_millis: status.last_updated_epoch_millis,
    })
}

pub fn proto_to_codebase_index_statuses_snapshot(
    snapshot: &proto::CodebaseIndexStatusesSnapshot,
) -> Vec<RemoteCodebaseIndexStatus> {
    snapshot
        .statuses
        .iter()
        .filter_map(proto_to_codebase_index_status)
        .collect()
}

pub fn proto_to_codebase_index_status_updated(
    update: &proto::CodebaseIndexStatusUpdated,
) -> Option<RemoteCodebaseIndexStatus> {
    proto_to_codebase_index_status(update.status.as_ref()?)
}

fn proto_to_state(state: proto::CodebaseIndexStatusState) -> Option<RemoteCodebaseIndexState> {
    match state {
        proto::CodebaseIndexStatusState::NotEnabled => Some(RemoteCodebaseIndexState::NotEnabled),
        proto::CodebaseIndexStatusState::Unavailable => Some(RemoteCodebaseIndexState::Unavailable),
        proto::CodebaseIndexStatusState::Unspecified => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_proto() {
        let status = RemoteCodebaseIndexStatus {
            repo_path: "/repo".to_string(),
            state: RemoteCodebaseIndexState::NotEnabled,
            last_updated_epoch_millis: Some(42),
        };

        let proto = proto::CodebaseIndexStatus::from(&status);
        assert_eq!(proto_to_codebase_index_status(&proto), Some(status));
    }

    #[test]
    fn unspecified_status_state_is_ignored() {
        let status = proto::CodebaseIndexStatus {
            repo_path: "/repo".to_string(),
            state: proto::CodebaseIndexStatusState::Unspecified as i32,
            last_updated_epoch_millis: None,
        };

        assert_eq!(proto_to_codebase_index_status(&status), None);
    }
}
