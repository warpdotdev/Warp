//! Per-session identifier exposed to local shells via `WARP_SESSION_ID`
//! and used by the `warp://session/<id>` deep link to focus a specific
//! Warp tab/pane.
//!
//! See `app/src/uri/mod.rs` for the deep-link handler and
//! `app/src/terminal/local_tty/terminal_manager.rs` for the env-var
//! injection point.
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use uuid::Uuid;
use warpui::{
    AppContext, Entity, EntityId, ModelContext, SingletonEntity, ViewHandle, WeakViewHandle,
};

use crate::terminal::TerminalView;

/// A stable, per-pane session identifier.
///
/// Generated when a local PTY-backed `TerminalView` is created, exposed to
/// the shell via the `WARP_SESSION_ID` environment variable, and used as
/// the lookup key for the `warp://session/<id>` deep link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WarpSessionId(Uuid);

impl WarpSessionId {
    /// Generate a fresh random session id.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for WarpSessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WarpSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use the canonical hyphenated lowercase form so external tools
        // that pass the value through `open warp://session/$WARP_SESSION_ID`
        // get a stable, parseable representation.
        self.0.as_hyphenated().fmt(f)
    }
}

impl FromStr for WarpSessionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Singleton registry mapping `WarpSessionId` -> `TerminalView`.
///
/// Backed by weak handles so a closed terminal pane is naturally garbage
/// collected; lookups also opportunistically prune dead entries. Entries
/// are inserted by the local TTY `TerminalManager` after the view is
/// created and removed when the manager is dropped.
pub struct WarpSessionRegistry {
    sessions: HashMap<WarpSessionId, WeakViewHandle<TerminalView>>,
}

impl Default for WarpSessionRegistry {
    fn default() -> Self {
        Self::new_inner()
    }
}

impl WarpSessionRegistry {
    /// Constructor compatible with `AppContext::add_singleton_model`.
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self::new_inner()
    }

    fn new_inner() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Register a new session. Subsequent `warp://session/<id>` deep
    /// links matching this id will route to `view`.
    pub fn register(&mut self, id: WarpSessionId, view: WeakViewHandle<TerminalView>) {
        self.sessions.insert(id, view);
    }

    /// Forget a session id. Safe to call multiple times.
    pub fn unregister(&mut self, id: &WarpSessionId) {
        self.sessions.remove(id);
    }

    /// Look up the live `TerminalView` for `id`, if one is still alive.
    ///
    /// The registry stores weak handles so a closed pane reports `None`
    /// without any explicit cleanup. Stale entries are reclaimed when the
    /// owning [`TerminalManager`] observes its `on_view_detached` close
    /// hook.
    pub fn lookup_view(
        &self,
        id: &WarpSessionId,
        ctx: &AppContext,
    ) -> Option<ViewHandle<TerminalView>> {
        self.sessions.get(id)?.upgrade(ctx)
    }

    /// Look up the `EntityId` of the `TerminalView` for `id`, if any.
    pub fn lookup_view_id(&self, id: &WarpSessionId, ctx: &AppContext) -> Option<EntityId> {
        self.lookup_view(id, ctx).map(|view| view.id())
    }
}

impl Entity for WarpSessionRegistry {
    type Event = ();
}

impl SingletonEntity for WarpSessionRegistry {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_session_id_round_trip() {
        let id = WarpSessionId::new();
        let formatted = id.to_string();
        let parsed: WarpSessionId = formatted.parse().expect("formatted id should parse");
        assert_eq!(id, parsed);
        // Hyphenated form is 36 chars (8-4-4-4-12).
        assert_eq!(formatted.len(), 36);
    }

    #[test]
    fn warp_session_id_rejects_garbage() {
        assert!("not-a-uuid".parse::<WarpSessionId>().is_err());
        assert!("".parse::<WarpSessionId>().is_err());
    }

    #[test]
    fn warp_session_id_new_returns_unique_values() {
        let a = WarpSessionId::new();
        let b = WarpSessionId::new();
        assert_ne!(a, b);
    }
}
