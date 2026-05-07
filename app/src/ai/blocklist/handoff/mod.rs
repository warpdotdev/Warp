//! Client-side pieces of the local-to-cloud Oz conversation handoff:
//!
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

pub(crate) mod touched_repos;
