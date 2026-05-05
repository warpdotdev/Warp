use warpui::{App, EntityId};

use super::*;
use crate::test_util::terminal::initialize_app_for_terminal_view;

fn attachment() -> AttachmentInput {
    AttachmentInput {
        file_name: "context.txt".to_owned(),
        mime_type: "text/plain".to_owned(),
        data: "hello".to_owned(),
    }
}

fn pending_launch() -> PendingCloudLaunch {
    PendingCloudLaunch {
        prompt: "fix tests".to_owned(),
        attachments: CloudLaunchAttachments {
            request_attachments: vec![attachment()],
            display_attachments: vec![],
        },
    }
}

fn pending_handoff() -> PendingHandoff {
    PendingHandoff {
        forked_conversation_id: "forked-conversation".to_owned(),
        touched_workspace: None,
        snapshot_upload: SnapshotUploadStatus::Pending,
        submission_state: HandoffSubmissionState::Idle,
        auto_submit: Some(pending_launch()),
        explicit_environment_id: None,
    }
}

fn add_model(app: &mut App) -> warpui::ModelHandle<AmbientAgentViewModel> {
    app.add_model(|ctx| AmbientAgentViewModel::new(EntityId::new(), ctx))
}

#[test]
fn queue_handoff_auto_submit_enters_waiting_state_without_consuming_launch() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let model = add_model(&mut app);

        model.update(&mut app, |model, ctx| {
            model.set_pending_handoff(Some(pending_handoff()), ctx);
        });

        let queued = model.update(&mut app, |model, ctx| model.queue_handoff_auto_submit(ctx));

        assert!(queued);
        model.read(&app, |model, _| {
            assert!(matches!(
                model.status(),
                Status::WaitingForSession {
                    kind: SessionStartupKind::InitialRun,
                    ..
                }
            ));
            let request = model.request().expect("request should be populated");
            assert_eq!(request.prompt, "fix tests");
            assert_eq!(
                request.conversation_id.as_deref(),
                Some("forked-conversation")
            );
            assert_eq!(request.attachments.len(), 1);
            assert!(request.initial_snapshot_token.is_none());

            let handoff = model
                .pending_handoff
                .as_ref()
                .expect("handoff should remain");
            assert_eq!(handoff.submission_state, HandoffSubmissionState::Queued);
            assert!(handoff.auto_submit.is_some());
        });

        let queued_again =
            model.update(&mut app, |model, ctx| model.queue_handoff_auto_submit(ctx));
        assert!(!queued_again);
    });
}

#[test]
fn maybe_auto_submit_handoff_waits_for_workspace_and_snapshot_then_consumes_launch() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let model = add_model(&mut app);

        model.update(&mut app, |model, ctx| {
            model.set_pending_handoff(Some(pending_handoff()), ctx);
            assert!(model.maybe_auto_submit_handoff(ctx).is_none());

            model.set_pending_handoff_workspace(TouchedWorkspace::default(), ctx);
            assert!(model.maybe_auto_submit_handoff(ctx).is_none());

            model.set_pending_handoff_snapshot_upload(
                SnapshotUploadStatus::SkippedEmptyWorkspace,
                ctx,
            );
            let launch = model
                .maybe_auto_submit_handoff(ctx)
                .expect("ready handoff should auto-submit");
            assert_eq!(launch.prompt, "fix tests");
            assert!(model.maybe_auto_submit_handoff(ctx).is_none());
        });
    });
}

#[test]
fn record_handoff_snapshot_upload_failed_restores_queued_launch_state() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let model = add_model(&mut app);

        model.update(&mut app, |model, ctx| {
            model.set_pending_handoff(Some(pending_handoff()), ctx);
            assert!(model.queue_handoff_auto_submit(ctx));

            let launch = model
                .record_handoff_snapshot_upload_failed("upload failed".to_owned(), ctx)
                .expect("queued launch should be returned for input restoration");

            assert_eq!(launch.prompt, "fix tests");
            assert!(matches!(model.status(), Status::Composing));
            assert!(model.request().is_none());

            let handoff = model
                .pending_handoff
                .as_ref()
                .expect("handoff should remain");
            assert_eq!(handoff.submission_state, HandoffSubmissionState::Idle);
            assert!(handoff.auto_submit.is_none());
            assert!(matches!(
                handoff.snapshot_upload,
                SnapshotUploadStatus::Failed(ref error) if error == "upload failed"
            ));
        });
    });
}
