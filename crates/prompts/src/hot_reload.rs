//! Hot-reload watcher for prompt template files.
//!
//! [`HotReloadComposer`] wraps [`Composer`] with a [`notify`]-based file
//! watcher. Whenever the base prompt, a role overlay, or the project overlay
//! changes on disk, a [`PromptFileChanged`] event is broadcast to all
//! subscribers. Callers subscribe, then call [`Composer::compose`] themselves
//! to pick up the new content — the underlying composer always re-reads from
//! disk on every call.
//!
//! # Dev-mode only
//!
//! This module is compiled only when the `dev` crate feature is enabled.
//! Do **not** use [`HotReloadComposer`] in production — keep [`Composer`]
//! there and remove the `dev` feature from your dependency declaration.
//!
//! # Example
//!
//! ```no_run
//! use orchestrator::Role;
//! use prompts::{Composer, ComposerConfig, hot_reload::HotReloadComposer};
//!
//! # async fn run() -> anyhow::Result<()> {
//! let config = ComposerConfig {
//!     base_path: "templates/base.md".into(),
//!     role_overlay_dir: "templates/roles".into(),
//!     project_overlay_path: Some("WARP.md".into()),
//! };
//!
//! let composer = HotReloadComposer::start(config)?;
//! let mut rx = composer.subscribe();
//!
//! loop {
//!     // Block until a file changes.
//!     let event = rx.recv().await?;
//!     tracing::info!(path = %event.path.display(), "prompt file changed — recomposing");
//!     let prompt = composer.composer().compose(Role::Planner).await?;
//!     println!("{}", prompt.system);
//! }
//! # }
//! ```

use std::{
    path::PathBuf,
    sync::mpsc as std_mpsc,
    thread,
    time::Duration,
};

use notify_debouncer_full::{
    new_debouncer_opt,
    notify::{Config, EventKind, RecursiveMode},
    DebounceEventHandler, DebounceEventResult, NoCache,
};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};

use crate::{Composer, ComposerConfig};

const DEBOUNCE_MS: u64 = 200;
const BROADCAST_CAPACITY: usize = 16;

/// A file-change event emitted by [`HotReloadComposer`].
///
/// Callers receive this via a [`broadcast::Receiver`] obtained from
/// [`HotReloadComposer::subscribe`], then call [`Composer::compose`] to
/// pick up the updated prompt.
#[derive(Debug, Clone)]
pub struct PromptFileChanged {
    /// The file path that triggered the event. May be a file inside the role
    /// overlay directory or the base/project overlay path directly.
    pub path: PathBuf,
}

/// Errors that can occur while starting [`HotReloadComposer`].
#[derive(Debug, Error)]
pub enum WatchError {
    /// The underlying OS file-watcher could not be created.
    #[error("failed to create file watcher: {0}")]
    Watcher(String),
}

/// A hot-reload wrapper around [`Composer`].
///
/// Spawns a background OS-watcher thread and a Tokio fan-out task. When any
/// watched prompt template changes, [`PromptFileChanged`] events are sent to
/// every active [`broadcast::Receiver`].
///
/// Drop [`HotReloadComposer`] to stop the watcher thread and close the
/// broadcast channel.
///
/// # Dev-mode only
///
/// Enabled by the `dev` crate feature. See the [module-level docs](self).
pub struct HotReloadComposer {
    inner: Composer,
    events_tx: broadcast::Sender<PromptFileChanged>,
    /// Dropping this signals the watcher thread to exit cleanly.
    _stop: std_mpsc::SyncSender<()>,
}

impl HotReloadComposer {
    /// Start watching the prompt files described by `config`.
    ///
    /// Spawns a background thread and a Tokio task. Path-specific watch
    /// failures (e.g. a role overlay dir that does not yet exist) are logged
    /// as warnings rather than returned as errors so that the caller can still
    /// receive events for the paths that *do* exist.
    ///
    /// Returns [`WatchError::Watcher`] only if the OS file-watcher itself
    /// cannot be created, which is exceedingly rare.
    ///
    /// # Panics
    ///
    /// Panics if called outside of a Tokio runtime context (it spawns a task
    /// via [`tokio::spawn`]).
    pub fn start(config: ComposerConfig) -> Result<Self, WatchError> {
        let (events_tx, _) = broadcast::channel::<PromptFileChanged>(BROADCAST_CAPACITY);
        // One-shot: thread signals setup result back to the caller.
        let (setup_tx, setup_rx) = std_mpsc::sync_channel::<Result<(), WatchError>>(0);
        // Watcher thread → Tokio fan-out task.
        let (raw_tx, raw_rx) = mpsc::unbounded_channel::<PathBuf>();
        // Dropping `stop_tx` unblocks the watcher thread so it can exit.
        let (stop_tx, stop_rx) = std_mpsc::sync_channel::<()>(0);

        // Fan raw path events out to broadcast subscribers.
        let fan_tx = events_tx.clone();
        tokio::spawn(async move {
            let mut rx = raw_rx;
            while let Some(path) = rx.recv().await {
                // Ignore send errors — no active subscribers is fine.
                let _ = fan_tx.send(PromptFileChanged { path });
            }
        });

        let watched_config = config.clone();
        thread::Builder::new()
            .name("prompt-hot-reload".into())
            .spawn(move || {
                watcher_thread(watched_config, raw_tx, stop_rx, setup_tx);
            })
            .map_err(|e| WatchError::Watcher(e.to_string()))?;

        // Wait for the thread to confirm the watcher started (or fail).
        setup_rx
            .recv()
            .unwrap_or_else(|_| Err(WatchError::Watcher("watcher thread exited immediately".into())))?;

        Ok(Self {
            inner: Composer::new(config),
            events_tx,
            _stop: stop_tx,
        })
    }

    /// Subscribe to change events.
    ///
    /// Each call returns an independent [`broadcast::Receiver`]. When files
    /// change, every active receiver is notified. Receivers that fall too far
    /// behind (more than [`BROADCAST_CAPACITY`] unread events) will observe a
    /// [`broadcast::error::RecvError::Lagged`] error and may miss intermediate
    /// events.
    pub fn subscribe(&self) -> broadcast::Receiver<PromptFileChanged> {
        self.events_tx.subscribe()
    }

    /// The underlying [`Composer`].
    ///
    /// Callers typically call [`Composer::compose`] after receiving a change
    /// event via [`subscribe`][Self::subscribe] to fetch the updated prompt.
    pub fn composer(&self) -> &Composer {
        &self.inner
    }
}

/// Runs inside the dedicated watcher thread.
///
/// Creates a debounced OS file watcher, registers all paths from `config`,
/// signals `setup_tx` with the result, then parks until the stop signal
/// arrives (i.e. until [`HotReloadComposer`] is dropped).
fn watcher_thread(
    config: ComposerConfig,
    tx: mpsc::UnboundedSender<PathBuf>,
    stop_rx: std_mpsc::Receiver<()>,
    setup_tx: std_mpsc::SyncSender<Result<(), WatchError>>,
) {
    let bridge = WatcherBridge { tx };
    let mut debouncer = match new_debouncer_opt(
        Duration::from_millis(DEBOUNCE_MS),
        None,
        bridge,
        NoCache,
        Config::default(),
    ) {
        Ok(d) => d,
        Err(e) => {
            let _ = setup_tx.send(Err(WatchError::Watcher(e.to_string())));
            return;
        }
    };

    // Watch the single base prompt file (non-recursive).
    if let Err(e) = debouncer.watch(&config.base_path, RecursiveMode::NonRecursive) {
        tracing::warn!(
            path = %config.base_path.display(),
            error = %e,
            "prompt hot-reload: cannot watch base path",
        );
    }

    // Watch the role overlay directory recursively so that adding/removing
    // overlay files is also detected.
    if let Err(e) = debouncer.watch(&config.role_overlay_dir, RecursiveMode::Recursive) {
        tracing::warn!(
            path = %config.role_overlay_dir.display(),
            error = %e,
            "prompt hot-reload: cannot watch role overlay dir",
        );
    }

    // Watch the project overlay only when it already exists at startup.
    // Changes to a non-existent project overlay will not be detected.
    if let Some(ref project_path) = config.project_overlay_path {
        if project_path.exists() {
            if let Err(e) = debouncer.watch(project_path, RecursiveMode::NonRecursive) {
                tracing::warn!(
                    path = %project_path.display(),
                    error = %e,
                    "prompt hot-reload: cannot watch project overlay",
                );
            }
        } else {
            tracing::debug!(
                path = %project_path.display(),
                "prompt hot-reload: project overlay absent at startup, skipping watch",
            );
        }
    }

    // Signal successful initialisation.
    let _ = setup_tx.send(Ok(()));
    tracing::info!("prompt hot-reload: watching for changes");

    // Park until HotReloadComposer is dropped (stop_tx drop → RecvError).
    let _ = stop_rx.recv();
    tracing::debug!("prompt hot-reload: watcher thread exiting");
    // `debouncer` is dropped here, which stops the OS watcher.
}

/// Bridges `notify-debouncer-full` callback events into a Tokio unbounded
/// channel so the fan-out task can forward them to broadcast subscribers.
struct WatcherBridge {
    tx: mpsc::UnboundedSender<PathBuf>,
}

impl DebounceEventHandler for WatcherBridge {
    fn handle_event(&mut self, result: DebounceEventResult) {
        match result {
            Ok(events) => {
                for event in events {
                    match event.event.kind {
                        EventKind::Create(_)
                        | EventKind::Modify(_)
                        | EventKind::Remove(_) => {
                            for path in event.paths {
                                if self.tx.send(path).is_err() {
                                    // Tokio receiver was dropped — fan-out task exited.
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(errors) => {
                for e in errors {
                    tracing::warn!(error = ?e, "prompt hot-reload: watcher error");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "hot_reload_tests.rs"]
mod tests;
