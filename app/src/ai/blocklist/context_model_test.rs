//! Unit tests for [`BlocklistAIContextModel::has_locking_attachment`] and the
//! `note_image_attachment_started` / `note_image_attachment_completed` counter mechanics.
//!
//! These tests deliberately bypass the production [`BlocklistAIContextModel::new`] constructor
//! (which subscribes to several singletons) and instead use [`BlocklistAIContextModel::new_for_test`]
//! together with [`super::agent_view::AgentViewController::new`] backed by
//! [`crate::terminal::view::ambient_agent::AmbientAgentViewModel::new_for_test`]. That keeps the
//! fixture small enough to focus on the lock/counter logic without standing up `BlocklistAIHistoryModel`,
//! `LLMPreferences`, `CloudModel`, `UpdateManager`, or `AppExecutionMode`.

use std::sync::Arc;

use parking_lot::FairMutex;
use warpui::r#async::executor::Background;
use warpui::{App, EntityId, ModelHandle};

use super::{BlocklistAIContextModel, PendingAttachment, PendingFile};
use crate::ai::agent::ImageContext;
use crate::ai::blocklist::agent_view::{AgentViewController, EphemeralMessageModel};
use crate::terminal::color::{self, Colors};
use crate::terminal::event_listener::ChannelEventListener;
use crate::terminal::model::test_utils::block_size;
use crate::terminal::model::{BlockId, TerminalModel};
use crate::terminal::view::ambient_agent::AmbientAgentViewModel;

/// Builds a [`BlocklistAIContextModel`] with stub dependencies. None of the dependencies are
/// exercised by the methods under test; they only need to satisfy the struct's field types.
fn build_test_context_model(app: &mut App) -> ModelHandle<BlocklistAIContextModel> {
    let terminal_model = Arc::new(FairMutex::new(TerminalModel::new_for_test(
        block_size(),
        color::List::from(&Colors::default()),
        ChannelEventListener::new_for_test(),
        Arc::new(Background::default()),
        false, /* should_show_bootstrap_block */
        None,  /* restored_blocks */
        false, /* honor_ps1 */
        false, /* is_inverted */
        None,  /* session_startup_path */
    )));
    let terminal_view_id = EntityId::new();

    let ambient_agent_view_model = app.add_model(|ctx| {
        AmbientAgentViewModel::new_for_test(
            terminal_view_id,
            false, /* has_parent_terminal */
            ctx,
        )
    });
    let ephemeral_message_model = app.add_model(|_| EphemeralMessageModel::new());
    let agent_view_controller = app.add_model(|ctx| {
        AgentViewController::new(
            terminal_model.clone(),
            terminal_view_id,
            ambient_agent_view_model,
            ephemeral_message_model,
            ctx,
        )
    });

    app.add_model(|_| {
        BlocklistAIContextModel::new_for_test(
            terminal_model,
            terminal_view_id,
            agent_view_controller,
        )
    })
}

fn make_image_attachment(file_name: &str) -> PendingAttachment {
    PendingAttachment::Image(ImageContext {
        data: String::new(),
        mime_type: "image/png".to_owned(),
        file_name: file_name.to_owned(),
        is_figma: false,
    })
}

fn make_file_attachment(file_name: &str) -> PendingAttachment {
    PendingAttachment::File(PendingFile {
        file_name: file_name.to_owned(),
        file_path: file_name.into(),
        mime_type: "text/plain".to_owned(),
    })
}

#[test]
fn has_locking_attachment_is_false_for_default_state() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.read(&app, |m, _| {
            assert!(!m.has_locking_attachment());
            assert_eq!(m.pending_image_attachments_in_progress_for_test(), 0);
        });
    });
}

#[test]
fn has_locking_attachment_is_true_with_pending_block_id() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, _| {
            m.insert_pending_block_id_for_test(BlockId::new());
        });

        model.read(&app, |m, _| assert!(m.has_locking_attachment()));
    });
}

#[test]
fn has_locking_attachment_is_true_with_pending_selected_text() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, _| {
            m.set_pending_selected_text_for_test(Some("hello".to_owned()));
        });

        model.read(&app, |m, _| assert!(m.has_locking_attachment()));
    });
}

#[test]
fn has_locking_attachment_is_true_with_pending_image_attachment() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, _| {
            m.append_pending_attachments_for_test(vec![make_image_attachment("a.png")]);
        });

        model.read(&app, |m, _| assert!(m.has_locking_attachment()));
    });
}

#[test]
fn has_locking_attachment_is_false_with_only_file_attachments() {
    // Files are explicitly *not* locking attachments — only images, blocks, selected text, or an
    // in-progress image-attach pipeline force the input into AI mode.
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, _| {
            m.append_pending_attachments_for_test(vec![
                make_file_attachment("notes.txt"),
                make_file_attachment("readme.md"),
            ]);
        });

        model.read(&app, |m, _| assert!(!m.has_locking_attachment()));
    });
}

#[test]
fn has_locking_attachment_ignores_non_image_attachments_when_image_present() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, _| {
            m.append_pending_attachments_for_test(vec![
                make_file_attachment("notes.txt"),
                make_image_attachment("a.png"),
            ]);
        });

        model.read(&app, |m, _| assert!(m.has_locking_attachment()));
    });
}

#[test]
fn note_image_attachment_started_increments_counter_and_locks_input() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, ctx| m.note_image_attachment_started(ctx));

        model.read(&app, |m, _| {
            assert_eq!(m.pending_image_attachments_in_progress_for_test(), 1);
            // The counter alone — without any actual `pending_attachments` entry — must lock the
            // input so paste / drag-and-drop flows can't slip into NLD before the async pipeline
            // appends the resulting `ImageContext`.
            assert!(m.has_locking_attachment());
        });
    });
}

#[test]
fn note_image_attachment_completed_decrements_counter_and_unlocks_input() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, ctx| {
            m.note_image_attachment_started(ctx);
            m.note_image_attachment_completed(ctx);
        });

        model.read(&app, |m, _| {
            assert_eq!(m.pending_image_attachments_in_progress_for_test(), 0);
            assert!(!m.has_locking_attachment());
        });
    });
}

#[test]
fn note_image_attachment_started_supports_multiple_concurrent_pipelines() {
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, ctx| {
            m.note_image_attachment_started(ctx);
            m.note_image_attachment_started(ctx);
            m.note_image_attachment_started(ctx);
        });

        model.read(&app, |m, _| {
            assert_eq!(m.pending_image_attachments_in_progress_for_test(), 3);
            assert!(m.has_locking_attachment());
        });

        // One completion should not release the lock while two pipelines are still in flight.
        model.update(&mut app, |m, ctx| m.note_image_attachment_completed(ctx));
        model.read(&app, |m, _| {
            assert_eq!(m.pending_image_attachments_in_progress_for_test(), 2);
            assert!(m.has_locking_attachment());
        });

        model.update(&mut app, |m, ctx| {
            m.note_image_attachment_completed(ctx);
            m.note_image_attachment_completed(ctx);
        });
        model.read(&app, |m, _| {
            assert_eq!(m.pending_image_attachments_in_progress_for_test(), 0);
            assert!(!m.has_locking_attachment());
        });
    });
}

#[test]
fn note_image_attachment_completed_saturates_at_zero() {
    // Defensive: a stray `_completed` without a matching `_started` (or a double-completion) must
    // not underflow `usize` and silently lock the input forever.
    App::test((), |mut app| async move {
        let model = build_test_context_model(&mut app);

        model.update(&mut app, |m, ctx| {
            m.note_image_attachment_completed(ctx);
            m.note_image_attachment_completed(ctx);
        });

        model.read(&app, |m, _| {
            assert_eq!(m.pending_image_attachments_in_progress_for_test(), 0);
            assert!(!m.has_locking_attachment());
        });
    });
}
