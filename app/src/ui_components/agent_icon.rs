//! Source-facing helpers that centralize the derivation of the agent-icon shape
//! ([`IconWithStatusVariant`]) from the underlying state models. The invariant the
//! helpers enforce: any single logical agent run renders as the same brand color, glyph,
//! and ambient-vs-local treatment regardless of which surface is rendering it (vertical
//! tabs, pane header, conversation list, notifications mailbox).
//!
//! Each helper is a thin adapter over one data source. Surfaces call the helper for
//! whichever source they hold and feed the resulting variant into
//! [`render_icon_with_status`]. The pure inner functions in this module are exercised
//! directly by the cross-surface consistency tests in `agent_icon_tests.rs`.
use warp_cli::agent::Harness;
use warpui::AppContext;
use warpui::SingletonEntity;

use crate::ai::agent::conversation::ConversationStatus;
use crate::ai::agent_conversations_model::ConversationOrTask;
use crate::terminal::cli_agent_sessions::listener::agent_supports_rich_status;
use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;
use crate::terminal::view::TerminalView;
use crate::terminal::CLIAgent;
use crate::ui_components::icon_with_status::IconWithStatusVariant;

/// Returns the agent-icon variant for a live [`TerminalView`], or `None` when the terminal is
/// not an agent surface (plain terminal / shell / empty conversation).
///
/// Resolution order:
/// 1. A [`CLIAgentSessionsModel`] session with a known agent (observed reality) wins.
///    Plugin-backed sessions surface rich status; command-detected sessions don't.
/// 2. An ambient agent with a selected third-party harness uses the harness's CLI brand
///    even before the harness CLI has started running in the sandbox.
/// 3. A selected conversation or ambient Oz run falls back to the Oz agent variant.
/// 4. Everything else returns `None` so the caller renders a plain-terminal indicator.
pub(crate) fn terminal_view_agent_icon_variant(
    terminal_view: &TerminalView,
    app: &AppContext,
) -> Option<IconWithStatusVariant> {
    let cli_agent_session = CLIAgentSessionsModel::as_ref(app).session(terminal_view.id());
    let inputs = TerminalIconInputs {
        is_ambient: terminal_view.is_ambient_agent_session(app),
        cli_session: cli_agent_session.map(|session| CLISessionInputs {
            agent: session.agent,
            has_listener: session.listener.is_some(),
            status: session.status.to_conversation_status(),
            supports_rich_status: agent_supports_rich_status(&session.agent),
        }),
        ambient_selected_third_party_cli_agent: terminal_view
            .ambient_agent_view_model()
            .as_ref(app)
            .selected_third_party_cli_agent(),
        selected_conversation_status: terminal_view.selected_conversation_status_for_display(app),
        has_selected_conversation: terminal_view
            .selected_conversation_display_title(app)
            .is_some(),
    };
    agent_icon_variant_from_terminal_inputs(&inputs)
}

/// Returns the agent-icon variant for a [`ConversationOrTask`] card row.
///
/// Task rows resolve their harness from [`ConversationOrTask::harness`]; conversation
/// rows have no harness signal and always render as local Oz per the product spec.
pub(crate) fn conversation_or_task_agent_icon_variant(
    src: &ConversationOrTask<'_>,
    app: &AppContext,
) -> Option<IconWithStatusVariant> {
    let status = src.status(app);
    Some(match src {
        ConversationOrTask::Task(_) => {
            agent_icon_variant_for_task(src.harness().unwrap_or(Harness::Oz), status)
        }
        ConversationOrTask::Conversation(_) => IconWithStatusVariant::OzAgent {
            status: Some(status),
            is_ambient: false,
        },
    })
}

/// Primitive inputs to the terminal-view waterfall, gathered once from the live
/// [`TerminalView`] / [`AppContext`]. Keeping the decision logic in terms of these
/// primitives makes it testable without a live app.
struct TerminalIconInputs {
    is_ambient: bool,
    cli_session: Option<CLISessionInputs>,
    /// The CLI agent corresponding to the currently selected cloud harness, when the selection
    /// is a third-party (non-Oz) harness. `None` for Oz or when no harness is selected.
    ambient_selected_third_party_cli_agent: Option<CLIAgent>,
    /// The conversation status that the terminal view would surface in its status-icon slot.
    selected_conversation_status: Option<ConversationStatus>,
    /// Whether the terminal view currently has a selected conversation (ambient or local).
    has_selected_conversation: bool,
}

/// CLI-session-derived inputs for the terminal waterfall.
struct CLISessionInputs {
    agent: CLIAgent,
    /// Whether the session is backed by a plugin listener. Plugin-backed sessions report
    /// rich status; command-detected sessions only know that an agent is running.
    has_listener: bool,
    status: ConversationStatus,
    /// Whether the agent's session handler exposes rich status (plugin-backed handlers report
    /// rich status; Codex's OSC 9 handler does not).
    supports_rich_status: bool,
}

/// Pure waterfall from primitive inputs to an [`IconWithStatusVariant`]. Mirrors the
/// resolution order documented on [`terminal_view_agent_icon_variant`].
fn agent_icon_variant_from_terminal_inputs(
    inputs: &TerminalIconInputs,
) -> Option<IconWithStatusVariant> {
    // 1. CLI session with a known (non-Unknown) agent wins. Status is only meaningful when
    //    the session is plugin-backed and the handler exposes rich status.
    if let Some(session) = inputs
        .cli_session
        .as_ref()
        .filter(|s| !matches!(s.agent, CLIAgent::Unknown))
    {
        let status = (session.has_listener && session.supports_rich_status)
            .then(|| session.status.clone());
        return Some(IconWithStatusVariant::CLIAgent {
            agent: session.agent,
            status,
            is_ambient: inputs.is_ambient,
        });
    }

    // 2. Ambient agent with a selected third-party harness. Render the harness's brand
    //    circle immediately once the user commits, even before the harness CLI starts
    //    running in the sandbox. `Unknown` is filtered to avoid rendering an unbranded
    //    gray circle for a harness this client doesn't recognize.
    if inputs.is_ambient {
        if let Some(agent) = inputs
            .ambient_selected_third_party_cli_agent
            .filter(|agent| !matches!(agent, CLIAgent::Unknown))
        {
            return Some(IconWithStatusVariant::CLIAgent {
                agent,
                status: inputs.selected_conversation_status.clone(),
                is_ambient: true,
            });
        }
    }

    // 3. Selected conversation OR ambient (Oz) terminal: Oz agent variant.
    if inputs.has_selected_conversation || inputs.is_ambient {
        return Some(IconWithStatusVariant::OzAgent {
            status: inputs.selected_conversation_status.clone(),
            is_ambient: inputs.is_ambient,
        });
    }

    None
}

/// Pure task-card logic: maps a [`Harness`] and the task's current status into an
/// [`IconWithStatusVariant`]. Task cards are always ambient. Falls back to the Oz
/// variant for [`Harness::Oz`] and [`Harness::Unknown`], the latter so a future-server
/// harness this client doesn't recognize doesn't render an unbranded gray circle.
fn agent_icon_variant_for_task(
    harness: Harness,
    status: ConversationStatus,
) -> IconWithStatusVariant {
    let cli_agent =
        CLIAgent::from_harness(harness).filter(|agent| !matches!(agent, CLIAgent::Unknown));
    match cli_agent {
        Some(agent) => IconWithStatusVariant::CLIAgent {
            agent,
            status: Some(status),
            is_ambient: true,
        },
        None => IconWithStatusVariant::OzAgent {
            status: Some(status),
            is_ambient: true,
        },
    }
}

#[cfg(test)]
#[path = "agent_icon_tests.rs"]
mod tests;
