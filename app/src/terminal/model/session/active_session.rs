use std::sync::Arc;
use warpui::{AppContext, Entity, ModelContext, ModelHandle};

use crate::{
    ai_assistant::execution_context::WarpAiExecutionContext,
    terminal::{
        model::session::SessionsEvent,
        model_events::{ModelEvent, ModelEventDispatcher},
        shell::ShellType,
        ShellLaunchData,
    },
};

use super::{Session, SessionType, Sessions};

pub struct ActiveSession {
    model_event_dispatcher: ModelHandle<ModelEventDispatcher>,
    sessions: ModelHandle<Sessions>,

    /// The current working directory of the terminal session.
    current_working_directory: Option<String>,
}

impl ActiveSession {
    pub fn new(
        sessions: ModelHandle<Sessions>,
        model_event_dispatcher: ModelHandle<ModelEventDispatcher>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&model_event_dispatcher, move |me, event, ctx| {
            if let ModelEvent::BlockMetadataReceived(block_metadata_received_event) = event {
                let new_pwd = block_metadata_received_event
                    .block_metadata
                    .current_working_directory()
                    .map(|cwd| cwd.to_owned());
                if me.current_working_directory != new_pwd {
                    me.current_working_directory = new_pwd;
                    ctx.emit(ActiveSessionEvent::UpdatedPwd);
                }
            }
        });

        ctx.subscribe_to_model(&sessions, |me, event, ctx| {
            if let SessionsEvent::SessionBootstrapped(bootstrap_event) = event {
                if Some(bootstrap_event.session_id)
                    == me.model_event_dispatcher.as_ref(ctx).active_session_id()
                {
                    ctx.emit(ActiveSessionEvent::Bootstrapped);
                }
            }
        });

        Self {
            sessions,
            model_event_dispatcher,
            current_working_directory: None,
        }
    }

    pub fn session(&self, app: &AppContext) -> Option<Arc<Session>> {
        self.model_event_dispatcher
            .as_ref(app)
            .active_session_id()
            .and_then(|session_id| self.sessions.as_ref(app).get(session_id))
    }

    pub fn session_type(&self, app: &AppContext) -> Option<SessionType> {
        self.session(app).map(|session| session.session_type())
    }

    pub fn shell_type(&self, app: &AppContext) -> Option<ShellType> {
        self.session(app)
            .as_ref()
            .map(|session| session.shell().shell_type())
    }

    pub fn shell_launch_data(&self, app: &AppContext) -> Option<ShellLaunchData> {
        self.session(app)
            .as_ref()
            .and_then(|session| session.launch_data().cloned())
    }

    pub fn current_working_directory(&self) -> Option<&String> {
        self.current_working_directory.as_ref()
    }

    /// Returns the `WarpAiExecutionContext` for the active session.
    pub fn ai_execution_environment(&self, app: &AppContext) -> Option<WarpAiExecutionContext> {
        self.session(app).as_ref().map(WarpAiExecutionContext::new)
    }
}

pub enum ActiveSessionEvent {
    /// The active session's working directory changed.
    UpdatedPwd,
    /// The active session finished bootstrapping.
    Bootstrapped,
}

impl Entity for ActiveSession {
    type Event = ActiveSessionEvent;
}
