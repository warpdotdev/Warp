use warpui::{text_layout::TextStyle, App};

use crate::{
    appearance::Appearance,
    terminal::{
        input::{
            decorations::{should_underline_unknown_command, InputBackgroundJobOptions},
            tests::{
                add_window_with_bootstrapped_terminal, initialize_app,
                simulate_directory_for_completion,
            },
        },
        model::session::SessionInfo,
    },
    themes::theme::AnsiColorIdentifier,
};
use warp_completer::completer::SuggestionTypeName;

/// Regression test for issue #9182: when the session has not yet finished
/// loading its set of external commands (executables on PATH), valid commands
/// like `git` would otherwise be incorrectly underlined as unknown. The
/// decision function must suppress underlining entirely until the load
/// completes.
#[test]
fn should_underline_unknown_command_suppresses_when_external_commands_not_loaded() {
    // The completer returned no description (would otherwise indicate an
    // unknown command), the token is the command position, and the symbols
    // are valid for our heuristic — but external commands haven't loaded
    // yet, so we must not underline.
    let has_loaded_external_commands = false;
    let has_no_description = true;
    let is_first_token = true;
    let valid_symbols = true;

    let result = should_underline_unknown_command(
        has_loaded_external_commands,
        has_no_description,
        is_first_token,
        valid_symbols,
    );

    assert!(
        !result,
        "must not underline command before external commands have loaded (issue #9182)"
    );
}

#[test]
fn should_underline_unknown_command_underlines_unknown_after_load_completes() {
    // After commands have loaded, an undescribed first-position token with
    // valid symbols is an unknown command and should be underlined.
    assert!(should_underline_unknown_command(
        true, /* has_loaded_external_commands */
        true, /* has_no_description */
        true, /* is_first_token */
        true, /* valid_symbols */
    ));
}

#[test]
fn should_underline_unknown_command_skips_known_commands() {
    // After load, a token with a description (i.e. a known command) must
    // not be underlined.
    assert!(!should_underline_unknown_command(
        true,  /* has_loaded_external_commands */
        false, /* has_no_description -- known */
        true,  /* is_first_token */
        true,  /* valid_symbols */
    ));
}

#[test]
fn should_underline_unknown_command_skips_non_first_tokens() {
    // Only the command position (token_index == 0) is subject to the
    // unknown-command underline. Arguments are not.
    assert!(!should_underline_unknown_command(
        true,  /* has_loaded_external_commands */
        true,  /* has_no_description */
        false, /* is_first_token -- this is an argument */
        true,  /* valid_symbols */
    ));
}

#[test]
fn should_underline_unknown_command_skips_invalid_symbols() {
    // Tokens containing characters we don't trust for the heuristic
    // (e.g. `!!`, `$foo`) should be left alone to avoid false positives.
    assert!(!should_underline_unknown_command(
        true,  /* has_loaded_external_commands */
        true,  /* has_no_description */
        true,  /* is_first_token */
        false, /* valid_symbols -- contains e.g. `!` */
    ));
}

#[test]
fn test_decorations_with_multibyte_chars() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal_colors_normal = app
            .get_singleton_model_handle::<Appearance>()
            .read(&app, |a, _| a.theme().terminal_colors().normal.to_owned());
        let error_underline_color = AnsiColorIdentifier::Red
            .to_ansi_color(&terminal_colors_normal)
            .into();
        let command_color = AnsiColorIdentifier::from(SuggestionTypeName::Command)
            .to_ansi_color(&terminal_colors_normal)
            .into();

        let session_info = SessionInfo::new_for_test();
        let session_id = session_info.session_id;

        let terminal =
            add_window_with_bootstrapped_terminal(&mut app, None, Some(session_info)).await;
        let input = terminal.read(&app, |view, _| view.input().clone());
        let editor = input.read(&app, |input, _| input.editor.clone());

        let session_id = terminal.update(&mut app, |terminal_view, ctx| {
            terminal_view
                .sessions_model()
                .update(ctx, |sessions, _ctx| {
                    // Wait until external commands have been loaded.
                    let session = sessions.get(session_id).expect("session should exist");
                    warpui::r#async::block_on(session.load_external_commands());
                });
            session_id
        });

        simulate_directory_for_completion(session_id, &terminal, &mut app, "/usr/bin");

        input
            .update(&mut app, |input, ctx| {
                input.clear_buffer_and_reset_undo_stack(ctx);
                input.user_insert("echo 'multibyte: שלום עולם'\nechoo hello world", ctx);

                // Trigger input decoration computation instead of waiting for the
                // debounced stream to trigger.
                input.run_input_background_jobs(
                    InputBackgroundJobOptions::default().with_command_decoration(),
                    ctx,
                );

                // Take the future handle from the input to ensure that it doesn't
                // get cancelled by the debounced decoration update stream.
                let future_handle = input
                    .decorations_future_handle
                    .take()
                    .expect("should have spawned decoration task");
                ctx.await_spawned_future(future_handle.future_id())
            })
            .await;

        let text_style_runs = editor.read(&app, |editor, ctx| {
            editor
                .text_style_runs(ctx)
                .map(|run| (run.text().to_owned(), run.text_style()))
                .collect::<Vec<_>>()
        });

        let expected = &[
            (
                "echo".to_string(),
                TextStyle::new().with_syntax_color(command_color),
            ),
            (" 'multibyte: שלום עולם'\n".to_string(), Default::default()),
            (
                "echoo".to_string(),
                TextStyle::new().with_error_underline_color(error_underline_color),
            ),
            (" hello world".to_string(), Default::default()),
        ];
        assert_eq!(
            text_style_runs, expected,
            "---- Expected ----\n{expected:#?}\n---- Actual ----\n{text_style_runs:#?}\n"
        );
    });
}
