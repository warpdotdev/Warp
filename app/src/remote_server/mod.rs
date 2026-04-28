// Re-export everything from the `remote_server` crate so existing
// `crate::remote_server::*` imports in `app` continue to work.
pub use remote_server::*;

#[cfg(not(target_family = "wasm"))]
pub mod server_model;
#[cfg(not(target_family = "wasm"))]
pub mod ssh_transport;
#[cfg(unix)]
pub mod unix;

/// Run the `remote-server-proxy` subcommand.
#[cfg(unix)]
pub fn run_proxy() -> anyhow::Result<()> {
    unix::run_proxy()
}

#[cfg(not(unix))]
pub fn run_proxy() -> anyhow::Result<()> {
    anyhow::bail!("remote-server-proxy is not supported on this platform")
}

/// Run the `remote-server-daemon` subcommand.
#[cfg(unix)]
pub fn run_daemon() -> anyhow::Result<()> {
    unix::run_daemon()
}

#[cfg(not(unix))]
pub fn run_daemon() -> anyhow::Result<()> {
    anyhow::bail!("remote-server-daemon is not supported on this platform")
}

/// Start the WarpUI headless app with all daemon singleton models.
///
/// This is the platform-agnostic core of every `run_daemon` implementation.
/// Platform-specific code (Unix sockets, Windows named pipes, …) binds a
/// listener and calls this function with the appropriate `ServerModel`
/// constructor — everything else (DirectoryWatcher, DetectedRepositories,
/// RepoMetadataModel, FileModel) is shared.
///
/// # Example
/// ```ignore
/// // In unix/mod.rs:
/// super::run_daemon_app(move |ctx| ServerModel::new(unix_listener, ctx))
/// ```
#[cfg(not(target_family = "wasm"))]
pub(super) fn run_daemon_app(
    server_model_init: impl FnOnce(&mut warpui::ModelContext<server_model::ServerModel>) -> server_model::ServerModel
        + 'static,
) -> anyhow::Result<()> {
    use warpui::platform::app::AppCallbacks;
    use warpui::platform::AppBuilder;

    AppBuilder::new_headless(AppCallbacks::default(), Box::new(()), None).run(|ctx| {
        // Rotate log files from the previous daemon invocation in the background.
        ctx.background_executor()
            .spawn(warp_logging::rotate_log_files())
            .detach();

        use repo_metadata::repositories::DetectedRepositories;
        use repo_metadata::watcher::DirectoryWatcher;
        use repo_metadata::RepoMetadataModel;
        // Order matters: DetectedRepositories must be registered before
        // RepoMetadataModel because LocalRepoMetadataModel::new()
        // subscribes to DetectedRepositories::handle(ctx).
        ctx.add_singleton_model(DirectoryWatcher::new);
        ctx.add_singleton_model(|_ctx| DetectedRepositories::default());
        ctx.add_singleton_model(RepoMetadataModel::new_with_incremental_updates);
        ctx.add_singleton_model(warp_files::FileModel::new);
        ctx.add_singleton_model(server_model_init);
    })?;
    Ok(())
}
