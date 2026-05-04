//! Client-side pieces of the local-to-cloud Oz conversation handoff. The only
//! submodule today, `touched_repos`, walks the conversation's action history to
//! collect every filesystem path the local agent has touched, groups those paths
//! into git roots and orphan files, and exposes the env-overlap pick used by the
//! handoff pane bootstrap. The snapshot upload itself is driven from
//! `AmbientAgentViewModel::submit_handoff` via `upload_snapshot_for_handoff`.

pub(crate) mod touched_repos;
