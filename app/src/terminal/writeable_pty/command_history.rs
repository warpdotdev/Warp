use std::collections::HashSet;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use lazy_static::lazy_static;
use parking_lot::FairMutex;
use warp_terminal::shell::{read_shell_history, write_command_to_shell_history, ShellType};
use warpui::{AppContext, ModelHandle, SingletonEntity};

use crate::persistence::StartedCommandMetadata;
use crate::terminal::model::session::SessionId;
use crate::terminal::{view::ExecuteCommandEvent, TerminalModel};
use crate::terminal::{History, HistoryEntry};
use crate::{persistence::ModelEvent, terminal::model::session::Sessions};

lazy_static! {
    static ref SHELL_HISTORY_SYNC_INITIALIZED: std::sync::Mutex<HashSet<String>> =
        std::sync::Mutex::new(HashSet::new());
}

pub fn ensure_shell_history_synced(session_id: SessionId, shell_type: ShellType, ctx: &mut AppContext) {
    let key = format!("{:?}-{:?}", session_id, shell_type);

    let mut initialized = SHELL_HISTORY_SYNC_INITIALIZED.lock().unwrap();
    if initialized.contains(&key) {
        return;
    }
    initialized.insert(key.clone());
    drop(initialized);

    let existing_commands: HashSet<String> = {
        let history = History::handle(ctx);
        history
            .commands_shared(session_id)
            .map(|commands| {
                commands
                    .iter()
                    .map(|c| c.command.clone())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default()
    };

    match read_shell_history(shell_type, &existing_commands) {
        Ok(shell_commands) => {
            if !shell_commands.is_empty() {
                log::info!(
                    "Syncing {} commands from shell history for session {:?}",
                    shell_commands.len(),
                    session_id
                );
                History::handle(ctx).update(ctx, move |history, _| {
                    let entries: Vec<HistoryEntry> = shell_commands
                        .into_iter()
                        .map(HistoryEntry::command_only)
                        .collect();
                    history.append_commands(session_id, entries);
                });
            }
        }
        Err(e) => {
            log::debug!("Could not read shell history: {}", e);
        }
    }
}

pub fn update_command_history(
    event: &ExecuteCommandEvent,
    model: &Arc<FairMutex<TerminalModel>>,
    model_event_sender: Option<&SyncSender<ModelEvent>>,
    sessions: &ModelHandle<Sessions>,
    ctx: &mut AppContext,
) {
    let model = model.lock();
    let active_block = model.block_list().active_block();
    let session_id = event.session_id;
    let Some(session) = sessions.as_ref(ctx).get(session_id) else {
        return;
    };

    let shell = session.shell();
    if !shell.should_add_command_to_history(&event.command) {
        return;
    }

    let is_agent_executed = event.source.is_ai_command();

    let session_ref = &*session;
    History::handle(ctx).update(ctx, move |history, _| {
        history.append_commands(
            session_id,
            vec![HistoryEntry::for_session_command(
                event.command.to_string(),
                active_block,
                session_ref,
                event.workflow_id.to_owned(),
                event.workflow_command.to_owned(),
                is_agent_executed,
            )],
        )
    });

    if let Some(sender) = model_event_sender {
        let sender_clone = sender.clone();
        let insert_command_event = ModelEvent::InsertCommand {
            metadata: StartedCommandMetadata {
                command: event.command.to_owned(),
                start_ts: active_block.start_ts().copied(),
                pwd: active_block.pwd().map(|pwd| pwd.to_owned()),
                shell: Some(session.shell().shell_type().name().to_owned()),
                username: Some(session.user().to_owned()),
                hostname: Some(session.hostname().to_owned()),
                session_id: Some(session_id),
                cloud_workflow_id: event.workflow_id.to_owned(),
                workflow_command: event.workflow_command.to_owned(),
                git_branch: active_block
                    .git_branch()
                    .map(|git_branch| git_branch.to_owned()),
                is_agent_executed,
            },
        };
        ctx.background_executor()
            .spawn(async move {
                if let Err(e) = sender_clone.send(insert_command_event) {
                    log::error!("Error sending ModelEvent: {e:?}");
                }
            })
            .detach();
    }

    let shell_type = session.shell().shell_type();
    let command = event.command.to_string();
    ctx.background_executor()
        .spawn(async move {
            if let Err(e) = write_command_to_shell_history(shell_type, &command) {
                log::debug!("Failed to write command to shell history: {}", e);
            }
        })
        .detach();
}
