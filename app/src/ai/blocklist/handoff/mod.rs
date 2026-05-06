//! Client-side pieces of the local-to-cloud Oz conversation handoff:
//!
//! - `launch`: carries the compose/auto-submit request payload from the input
//!   that triggered handoff into the fresh cloud pane.
//! - `touched_repos`: walks the conversation's action history to collect every
//!   filesystem path the local agent has touched, groups those paths into git
//!   roots and orphan files, and exposes the env-overlap pick used by the
//!   handoff pane bootstrap.
//!
//! The chip-click open path lives in `Workspace::start_local_to_cloud_handoff`
//! and drives the conversation fork + async snapshot upload directly via
//! `AIClient::fork_conversation` and `agent_sdk::driver::upload_snapshot_for_handoff`.
//! The actual cloud-agent spawn happens inside the handoff pane's
//! `AmbientAgentViewModel::submit_handoff`, which reads the cached
//! `forked_conversation_id` and `snapshot_upload` off `PendingHandoff`.
mod launch;

use crate::features::FeatureFlag;

pub(crate) use launch::{
    CloudLaunchAttachments, CloudLaunchEntrypoint, CloudLaunchRequest, CloudLaunchRequestId,
};

pub(crate) fn is_local_to_cloud_handoff_available() -> bool {
    FeatureFlag::OzHandoff.is_enabled()
        && FeatureFlag::HandoffLocalCloud.is_enabled()
        && cfg!(all(feature = "local_fs", not(target_family = "wasm")))
}

// `launch` compiles on all targets (only depends on `server::ids` / `server::server_api`).
// `touched_repos` requires `local_fs` for filesystem-walking APIs.
#[cfg(feature = "local_fs")]
pub(crate) mod touched_repos;
