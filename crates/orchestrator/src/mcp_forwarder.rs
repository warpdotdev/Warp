//! MCP server forwarding to the currently active [`Agent`].
//!
//! The Helm process may have several agents registered at any one time. When
//! the user switches between them, any MCP server connections in flight need
//! to follow: tool calls must reach the *new* active agent, not the old one.
//!
//! This module owns that bookkeeping. [`McpForwarder`] holds the single
//! "active agent" reference and broadcasts changes via a
//! [`tokio::sync::watch`] channel. App-layer code that bridges MCP traffic
//! to the agent layer subscribes to the watch and re-targets its connections
//! whenever the forwarding target changes.
//!
//! # Design notes
//!
//! * The forwarder itself is `Send + Sync` — it can be held inside an
//!   [`Arc`] and shared freely across async tasks and threads.
//! * [`McpForwarder::set_active`] and [`McpForwarder::clear_active`] both
//!   return `bool` indicating whether the value actually changed. This lets
//!   callers avoid spurious reconnect work when the agent is re-confirmed
//!   rather than switched.
//! * The watch channel preserves only the *latest* target; there is no
//!   history of past switches. Subscribers that care about ordering should
//!   record the previous value themselves before calling `borrow_and_update`.
//!
//! # Example
//!
//! ```rust
//! use orchestrator::{AgentId, McpForwarder, ForwardingTarget};
//!
//! let forwarder = McpForwarder::new();
//!
//! // Subscribe before the first switch so we don't miss the initial change.
//! let mut rx = forwarder.subscribe();
//!
//! let agent_a = AgentId("agent-a".to_string());
//! assert!(forwarder.set_active(agent_a.clone()), "first switch changes target");
//! assert!(!forwarder.set_active(agent_a.clone()), "same agent is a no-op");
//!
//! // The watch was notified once.
//! assert!(rx.has_changed().unwrap());
//! ```
//!
//! [`Arc`]: std::sync::Arc

use tokio::sync::watch;

use crate::AgentId;

/// The current target for MCP tool-call forwarding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardingTarget {
    /// MCP tool calls should be routed to this agent.
    Agent(AgentId),
    /// No agent is active; incoming MCP tool calls cannot be forwarded.
    None,
}

impl ForwardingTarget {
    /// Returns `true` when a specific agent is set as the target.
    pub fn is_active(&self) -> bool {
        matches!(self, ForwardingTarget::Agent(_))
    }

    /// Extracts the [`AgentId`] if a specific agent is targeted.
    pub fn agent_id(&self) -> Option<&AgentId> {
        match self {
            ForwardingTarget::Agent(id) => Some(id),
            ForwardingTarget::None => None,
        }
    }
}

/// Routes MCP server connections to the currently active [`Agent`].
///
/// The forwarder maintains a single "active agent" reference. Callers that
/// bridge MCP tool traffic (tool calls and their results) with the agent layer
/// query [`McpForwarder::active_agent_id`] or subscribe to target changes via
/// [`McpForwarder::subscribe`]. When the active agent is replaced — because
/// the user switches agents or a session ends — the watch channel notifies all
/// subscribers so they can re-target their MCP connections without dropping
/// in-flight tool calls.
///
/// Wrap in [`Arc`] to share across tasks:
///
/// ```rust
/// use std::sync::Arc;
/// use orchestrator::McpForwarder;
///
/// let forwarder = Arc::new(McpForwarder::new());
/// let clone = Arc::clone(&forwarder);
/// ```
///
/// [`Arc`]: std::sync::Arc
pub struct McpForwarder {
    tx: watch::Sender<ForwardingTarget>,
}

impl McpForwarder {
    /// Construct a new [`McpForwarder`] with no active agent.
    pub fn new() -> Self {
        let (tx, _initial_rx) = watch::channel(ForwardingTarget::None);
        Self { tx }
    }

    /// Returns the ID of the currently active agent, if any.
    ///
    /// This is a momentary snapshot; the active agent may change immediately
    /// after this call returns if another task calls [`set_active`] or
    /// [`clear_active`]. For reactive behaviour, use [`subscribe`] instead.
    ///
    /// [`set_active`]: McpForwarder::set_active
    /// [`clear_active`]: McpForwarder::clear_active
    /// [`subscribe`]: McpForwarder::subscribe
    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.tx.borrow().agent_id().cloned()
    }

    /// Returns the current [`ForwardingTarget`] as a snapshot.
    pub fn current_target(&self) -> ForwardingTarget {
        self.tx.borrow().clone()
    }

    /// Set `id` as the active agent, forwarding all subsequent MCP tool calls
    /// to it.
    ///
    /// Returns `true` if the target changed (i.e. `id` differs from the
    /// previously active agent), `false` if `id` was already the active agent
    /// and no notification was sent. Callers can skip re-targeting MCP
    /// connections when this returns `false`.
    pub fn set_active(&self, id: AgentId) -> bool {
        let already_active = matches!(
            &*self.tx.borrow(),
            ForwardingTarget::Agent(current) if *current == id
        );
        if already_active {
            return false;
        }
        // Use `send_replace` rather than `send`: the latter returns
        // `Err(SendError)` when there are no live receivers and *does not*
        // update the stored value, which would silently lose the target if
        // no subscriber had attached yet. `send_replace` always overwrites
        // the stored value and notifies any receivers that do exist.
        self.tx.send_replace(ForwardingTarget::Agent(id));
        true
    }

    /// Clear the active agent. MCP tool calls cannot be forwarded until a new
    /// agent is set via [`set_active`].
    ///
    /// Returns `true` if the target changed (i.e. there was a previously
    /// active agent), `false` if the target was already [`ForwardingTarget::None`].
    ///
    /// [`set_active`]: McpForwarder::set_active
    pub fn clear_active(&self) -> bool {
        if matches!(&*self.tx.borrow(), ForwardingTarget::None) {
            return false;
        }
        // See `set_active` for why we prefer `send_replace` over `send`.
        self.tx.send_replace(ForwardingTarget::None);
        true
    }

    /// Subscribe to target changes.
    ///
    /// The returned [`watch::Receiver`] always holds the most recent
    /// [`ForwardingTarget`]. Callers use
    /// [`Receiver::borrow_and_update`][tokio::sync::watch::Receiver::borrow_and_update]
    /// or `changed().await` to react to agent switches.
    pub fn subscribe(&self) -> watch::Receiver<ForwardingTarget> {
        self.tx.subscribe()
    }
}

impl Default for McpForwarder {
    fn default() -> Self {
        Self::new()
    }
}
