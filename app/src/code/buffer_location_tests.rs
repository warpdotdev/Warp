use lsp::LspManagerModel;
use remote_server::proto::TextEdit;
use repo_metadata::repositories::DetectedRepositories;
use repo_metadata::watcher::DirectoryWatcher;
use repo_metadata::RepoMetadataModel;
use warp_files::FileModel;
use warp_util::content_version::ContentVersion;
use warp_util::host_id::HostId;
use warp_util::standardized_path::StandardizedPath;
use warpui::{App, ModelHandle, SingletonEntity};

use crate::code::global_buffer_model::{GlobalBufferModel, GlobalBufferModelEvent, LineColumnEdit};
use crate::test_util::settings::initialize_settings_for_tests;

// ── Test setup ────────────────────────────────────────────────────

/// Minimum singletons required by `GlobalBufferModel::new`.
fn init_app(app: &mut App) {
    initialize_settings_for_tests(app);
    app.add_singleton_model(|_| LspManagerModel::new());
    app.add_singleton_model(DirectoryWatcher::new);
    app.add_singleton_model(|_| DetectedRepositories::default());
    app.add_singleton_model(RepoMetadataModel::new);
    app.add_singleton_model(FileModel::new);
}

/// Returns the `GlobalBufferModel` singleton handle.
fn gbm(app: &App) -> ModelHandle<GlobalBufferModel> {
    GlobalBufferModel::handle(app)
}

/// Reads the text content of a buffer tracked by `GlobalBufferModel`.
fn content(app: &App, file_id: warp_util::file::FileId) -> String {
    let handle = gbm(app);
    app.read(|ctx| {
        handle
            .as_ref(ctx)
            .content_for_file(file_id, ctx)
            .unwrap_or_default()
    })
}

/// Returns the server_version from the ServerLocal sync clock.
fn server_version(app: &App, file_id: warp_util::file::FileId) -> ContentVersion {
    let handle = gbm(app);
    app.read(|ctx| {
        handle
            .as_ref(ctx)
            .sync_clock_for_server_local(file_id)
            .unwrap()
            .server_version
    })
}

/// Helper: creates a proto `TextEdit` for use with `apply_client_edit`.
fn text_edit(
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    text: &str,
) -> TextEdit {
    TextEdit {
        start_line,
        start_column,
        end_line,
        end_column,
        text: text.to_string(),
    }
}

/// Helper: creates a `LineColumnEdit` for use with `handle_buffer_updated_push`.
fn line_col_edit(
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    text: &str,
) -> LineColumnEdit {
    LineColumnEdit {
        start_line,
        start_column,
        end_line,
        end_column,
        text: text.to_string(),
    }
}

fn test_host_id() -> HostId {
    HostId::new("test-host".to_string())
}

fn test_path() -> StandardizedPath {
    StandardizedPath::try_new("/test/file.txt").unwrap()
}

// ── Flow 1: Open server-local buffer ──────────────────────────────

#[test]
fn open_server_local_creates_buffer_and_is_server_local() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let file_id = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_open.txt".into(), ctx);
            state.file_id
        });

        let handle = gbm(&app);
        app.read(|ctx| {
            assert!(handle.as_ref(ctx).is_server_local(file_id));
            assert!(handle.as_ref(ctx).sync_clock_for_server_local(file_id).is_some());
        });
    })
}

// ── Flow 2: Client edit via apply_client_edit ─────────────────────

#[test]
fn apply_client_edit_accepted_when_version_matches() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        // Open a server-local buffer and manually populate it with content
        // (simulating what FileModel::FileLoaded would do).
        // Keep _buffer_state alive so the WeakModelHandle in GlobalBufferModel
        // can be upgraded (the ModelHandle<Buffer> is the only strong reference).
        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_edit.txt".into(), ctx);
            // Manually populate content (bypassing async file load).
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "hello\nworld",
                ContentVersion::new(),
                ContentVersion::new(),
                true, // is_initial_load
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        // Read the server_version before the edit.
        let sv = server_version(&app, file_id);

        // Apply a client edit: insert " there" at end of line 0.
        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.apply_client_edit(
                file_id,
                &[text_edit(0, 5, 0, 5, " there")],
                sv, // expected_server_version matches
                ContentVersion::new(),
                ctx,
            )
        });

        assert!(accepted);
        assert_eq!(content(&app, file_id), "hello there\nworld");
    })
}

#[test]
fn apply_client_edit_rejected_when_version_stale() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_reject.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "original",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        // Use a stale server_version (not the current one).
        let stale_sv = ContentVersion::from_raw(99999);

        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.apply_client_edit(
                file_id,
                &[text_edit(0, 8, 0, 8, " edit")],
                stale_sv,
                ContentVersion::new(),
                ctx,
            )
        });

        assert!(!accepted);
        assert_eq!(content(&app, file_id), "original");
    })
}

#[test]
fn apply_client_edit_replaces_range() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_replace.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "hello world",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        let sv = server_version(&app, file_id);

        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            // Replace "world" (col 6..11) with "rust".
            gbm.apply_client_edit(
                file_id,
                &[text_edit(0, 6, 0, 11, "rust")],
                sv,
                ContentVersion::new(),
                ctx,
            )
        });

        assert!(accepted);
        assert_eq!(content(&app, file_id), "hello rust");
    })
}

#[test]
fn apply_client_edit_across_lines() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_multiline.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "line1\nline2\nline3",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        let sv = server_version(&app, file_id);

        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            // Delete from end of line1 to start of line3.
            gbm.apply_client_edit(
                file_id,
                &[text_edit(0, 5, 2, 0, "\n")],
                sv,
                ContentVersion::new(),
                ctx,
            )
        });

        assert!(accepted);
        assert_eq!(content(&app, file_id), "line1\nline3");
    })
}

// ── Flow 3: Server push via handle_buffer_updated_push ────────────

#[test]
fn handle_buffer_updated_push_accepted_when_version_matches() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let host_id = test_host_id();
        let path = test_path();

        // Seed a remote buffer (client_version starts at 0).
        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.seed_remote_buffer_for_test(
                host_id.clone(),
                path.clone(),
                "hello\nworld",
                42, // server_version
                ctx,
            )
        });
        let file_id = _buffer_state.file_id;

        // Push an edit with expected_client_version = 0 (matches initial).
        gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.handle_buffer_updated_push(
                &host_id,
                path.as_str(),
                43, // new_server_version
                0,  // expected_client_version (matches the seeded 0)
                &[line_col_edit(0, 5, 0, 5, " there")],
                ctx,
            );
        });

        assert_eq!(content(&app, file_id), "hello there\nworld");
    })
}

#[test]
fn handle_buffer_updated_push_conflict_when_client_version_stale() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let host_id = test_host_id();
        let path = test_path();

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            // Seed with client_version = 0.
            gbm.seed_remote_buffer_for_test(
                host_id.clone(),
                path.clone(),
                "original",
                42,
                ctx,
            )
        });
        let file_id = _buffer_state.file_id;

        // Collect events.
        let (event_tx, event_rx) = async_channel::unbounded::<bool>();
        let gbm_handle = gbm(&app);
        app.update(|ctx| {
            ctx.subscribe_to_model(&gbm_handle, move |_, event, _| {
                if matches!(event, GlobalBufferModelEvent::RemoteBufferConflict { .. }) {
                    let _ = event_tx.try_send(true);
                }
            });
        });

        // Push with expected_client_version = 999 (does not match 0).
        gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.handle_buffer_updated_push(
                &host_id,
                path.as_str(),
                43,
                999, // stale expected_client_version
                &[line_col_edit(0, 0, 0, 8, "replaced")],
                ctx,
            );
        });

        // Content should be unchanged.
        assert_eq!(content(&app, file_id), "original");

        // Should have emitted a RemoteBufferConflict event.
        assert!(event_rx.try_recv().is_ok());
    })
}

// ── Flow 4: Close / deallocate ────────────────────────────────────

#[test]
fn remove_deallocates_buffer() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_remove.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "content",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        // Verify it exists.
        assert_eq!(content(&app, file_id), "content");

        // Remove it.
        gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.remove(file_id, ctx);
        });

        // content_for_file should now return None (empty string via unwrap_or_default).
        assert_eq!(content(&app, file_id), "");
    })
}

// ── Version tracking ──────────────────────────────────────────────

#[test]
fn apply_client_edit_updates_sync_clock() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_clock.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "hello",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        let sv = server_version(&app, file_id);
        let new_cv = ContentVersion::new();

        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.apply_client_edit(
                file_id,
                &[text_edit(0, 5, 0, 5, " world")],
                sv,
                new_cv,
                ctx,
            )
        });
        assert!(accepted);

        // client_version should be updated; server_version unchanged.
        let handle = gbm(&app);
        app.read(|ctx| {
            let clock = handle.as_ref(ctx).sync_clock_for_server_local(file_id).unwrap();
            assert_eq!(clock.client_version, new_cv);
            assert_eq!(clock.server_version, sv);
        });
    })
}

#[test]
fn server_push_updates_sync_clock() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let host_id = test_host_id();
        let path = test_path();

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.seed_remote_buffer_for_test(host_id.clone(), path.clone(), "hello", 42, ctx)
        });
        let file_id = _buffer_state.file_id;

        gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.handle_buffer_updated_push(
                &host_id,
                path.as_str(),
                43,
                0,
                &[line_col_edit(0, 5, 0, 5, " world")],
                ctx,
            );
        });

        // server_version should be updated; client_version unchanged.
        let handle = gbm(&app);
        app.read(|ctx| {
            let clock = handle.as_ref(ctx).sync_clock_for_remote_test(file_id).unwrap();
            assert_eq!(clock.server_version, ContentVersion::from_raw(43));
            assert_eq!(clock.client_version, ContentVersion::from_raw(0));
        });
    })
}

// ── Round-trip: sequential operations ─────────────────────────────

#[test]
fn sequential_client_edits_accepted() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_seq_client.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "abc",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        let sv = server_version(&app, file_id);
        let cv1 = ContentVersion::new();

        // First edit: append "d".
        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.apply_client_edit(file_id, &[text_edit(0, 3, 0, 3, "d")], sv, cv1, ctx)
        });
        assert!(accepted);
        assert_eq!(content(&app, file_id), "abcd");

        // Second edit: append "e". server_version is unchanged.
        let cv2 = ContentVersion::new();
        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.apply_client_edit(file_id, &[text_edit(0, 4, 0, 4, "e")], sv, cv2, ctx)
        });
        assert!(accepted);
        assert_eq!(content(&app, file_id), "abcde");

        // Final clock state.
        let handle = gbm(&app);
        app.read(|ctx| {
            let clock = handle.as_ref(ctx).sync_clock_for_server_local(file_id).unwrap();
            assert_eq!(clock.client_version, cv2);
            assert_eq!(clock.server_version, sv);
        });
    })
}

#[test]
fn sequential_server_pushes_accepted() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let host_id = test_host_id();
        let path = test_path();

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.seed_remote_buffer_for_test(host_id.clone(), path.clone(), "ab", 10, ctx)
        });
        let file_id = _buffer_state.file_id;

        // First push: append "c".
        gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.handle_buffer_updated_push(
                &host_id,
                path.as_str(),
                11,
                0,
                &[line_col_edit(0, 2, 0, 2, "c")],
                ctx,
            );
        });
        assert_eq!(content(&app, file_id), "abc");

        // Second push: append "d". client_version still 0.
        gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.handle_buffer_updated_push(
                &host_id,
                path.as_str(),
                12,
                0,
                &[line_col_edit(0, 3, 0, 3, "d")],
                ctx,
            );
        });
        assert_eq!(content(&app, file_id), "abcd");

        // Final clock state.
        let handle = gbm(&app);
        app.read(|ctx| {
            let clock = handle.as_ref(ctx).sync_clock_for_remote_test(file_id).unwrap();
            assert_eq!(clock.server_version, ContentVersion::from_raw(12));
            assert_eq!(clock.client_version, ContentVersion::from_raw(0));
        });
    })
}

// ── Conflict resolution ──────────────────────────────────────────

#[test]
fn resolve_conflict_updates_content_and_clock() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_resolve.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "original",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        let acked_sv = ContentVersion::new();

        // resolve_conflict may fail on the disk-save portion in tests;
        // the in-memory content and clock update are not gated on save success.
        let _ = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.resolve_conflict(file_id, acked_sv, "resolved content", ctx)
        });

        assert_eq!(content(&app, file_id), "resolved content");

        let handle = gbm(&app);
        app.read(|ctx| {
            let clock = handle.as_ref(ctx).sync_clock_for_server_local(file_id).unwrap();
            assert_eq!(clock.server_version, acked_sv);
        });
    })
}

// ── Batched edits ────────────────────────────────────────────────

#[test]
fn apply_client_edit_multiple_edits_in_batch() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            let state = gbm.open_server_local("/tmp/test_batch_client.txt".into(), ctx);
            gbm.populate_buffer_with_read_content(
                state.file_id,
                "aaa bbb ccc",
                ContentVersion::new(),
                ContentVersion::new(),
                true,
                ctx,
            );
            state
        });
        let file_id = _buffer_state.file_id;

        let sv = server_version(&app, file_id);

        let accepted = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.apply_client_edit(
                file_id,
                &[
                    text_edit(0, 0, 0, 3, "xxx"),
                    text_edit(0, 8, 0, 11, "zzz"),
                ],
                sv,
                ContentVersion::new(),
                ctx,
            )
        });

        assert!(accepted);
        assert_eq!(content(&app, file_id), "xxx bbb zzz");
    })
}

#[test]
fn handle_buffer_updated_push_multiple_edits_in_batch() {
    App::test((), |mut app| async move {
        init_app(&mut app);
        app.add_singleton_model(GlobalBufferModel::new);

        let host_id = test_host_id();
        let path = test_path();

        let _buffer_state = gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.seed_remote_buffer_for_test(
                host_id.clone(),
                path.clone(),
                "aaa bbb ccc",
                1,
                ctx,
            )
        });
        let file_id = _buffer_state.file_id;

        gbm(&app).update(&mut app, |gbm, ctx| {
            gbm.handle_buffer_updated_push(
                &host_id,
                path.as_str(),
                2,
                0,
                &[
                    line_col_edit(0, 0, 0, 3, "xxx"),
                    line_col_edit(0, 8, 0, 11, "zzz"),
                ],
                ctx,
            );
        });

        assert_eq!(content(&app, file_id), "xxx bbb zzz");
    })
}
