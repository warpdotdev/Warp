//! Unified repo detection for both local and remote sessions.
//!
//! Provides a single entry point ([`detect_possible_git_repo`]) that dispatches
//! to either local filesystem detection or remote server detection depending on
//! the session type. Callers spawn the returned future and handle the result
//! uniformly regardless of whether the session is local or remote.

use std::future::Future;

use futures::future::Either;
use repo_metadata::repositories::{DetectedRepositories, RepoDetectionSource};
use warp_core::SessionId;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warpui::{SingletonEntity, View, ViewContext};

use crate::remote_server::manager::RemoteServerManager;

/// Describes whether the active session is local or remote.
pub enum RepoDetectionSessionType {
    /// A local terminal session — repo detection runs on the local filesystem.
    Local,
    /// A remote SSH session — repo detection is delegated to the remote server
    /// via `navigate_to_directory`.
    Remote { session_id: SessionId },
}

/// Detects the git repository root for the given working directory.
///
/// - **Local sessions**: delegates to
///   [`DetectedRepositories::detect_possible_local_git_repo`], which spawns a
///   background filesystem check. The returned future resolves with the local
///   repo root wrapped in [`LocalOrRemotePath::Local`].
///
/// - **Remote sessions**: checks whether the remote server is connected via
///   [`RemoteServerManager::is_session_potentially_active`]. If not, resolves
///   to `None` immediately (no point in detection). Otherwise, calls
///   [`RemoteServerManager::navigate_to_directory`] and awaits the response.
///   When `is_git` is true, resolves with the repo root wrapped in
///   [`LocalOrRemotePath::Remote`]. The `NavigatedToDirectory` event is still
///   emitted for other subscribers (file tree, etc.).
///
/// The caller is responsible for registering remote repo roots in
/// `DetectedRepositories` and triggering downstream side effects (git status,
/// code review, etc.) in the spawn callback.
pub fn detect_possible_git_repo<V: View>(
    session_type: RepoDetectionSessionType,
    active_directory: &str,
    source: RepoDetectionSource,
    ctx: &mut ViewContext<V>,
) -> impl Future<Output = Option<LocalOrRemotePath>> {
    match session_type {
        RepoDetectionSessionType::Local => {
            let fut = DetectedRepositories::handle(ctx).update(ctx, |repos, ctx| {
                repos.detect_possible_local_git_repo(active_directory, source, ctx)
            });
            Either::Left(async move { fut.await.map(LocalOrRemotePath::Local) })
        }
        RepoDetectionSessionType::Remote { session_id } => {
            let fut = if RemoteServerManager::as_ref(ctx).is_session_potentially_active(session_id)
            {
                let inner = RemoteServerManager::handle(ctx).update(ctx, |mgr, ctx| {
                    mgr.navigate_to_directory(session_id, active_directory.to_string(), ctx)
                });
                Some(inner)
            } else {
                None
            };
            Either::Right(async move {
                let inner = fut?;
                match inner.await {
                    Some((remote_path, true)) => Some(LocalOrRemotePath::Remote(remote_path)),
                    _ => None,
                }
            })
        }
    }
}
