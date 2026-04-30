use super::{CollapsibleElementState, CollapsibleExpansionState, ImportedCommentElementState};
use crate::settings::AISettings;
use crate::test_util::settings::initialize_settings_for_tests;
use settings::Setting;
use std::path::Path;
use warpui::{App, SingletonEntity};

#[test]
fn reasoning_auto_collapses_when_user_has_not_manually_toggled() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let mut state = CollapsibleElementState::default();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Collapsed
        ));
    });
}

#[test]
fn always_show_thinking_stays_expanded_after_finish() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        AISettings::handle(&app).update(&mut app, |settings, ctx| {
            settings
                .thinking_display_mode
                .set_value(crate::settings::ThinkingDisplayMode::AlwaysShow, ctx)
                .unwrap();
        });

        let mut state = CollapsibleElementState::default();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Expanded {
                is_finished: true,
                scroll_pinned_to_bottom: false
            }
        ));
    });
}

#[test]
fn manual_collapse_while_streaming_stays_collapsed_after_finish() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let mut state = CollapsibleElementState::default();

        state.toggle_expansion();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Collapsed
        ));
    });
}

#[test]
fn manual_reexpand_while_streaming_stays_expanded_after_finish() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        let mut state = CollapsibleElementState::default();

        state.toggle_expansion();
        state.toggle_expansion();
        app.update(|ctx| {
            state.finish_reasoning(ctx);
        });

        assert!(matches!(
            state.expansion_state,
            CollapsibleExpansionState::Expanded {
                is_finished: true,
                scroll_pinned_to_bottom: false
            }
        ));
    });
}

#[test]
fn imported_comment_code_review_button_is_enabled_without_repo_disable() {
    let (label, disabled, tooltip) =
        ImportedCommentElementState::open_in_code_review_button_state(false, false, None);

    assert_eq!(label, "Open in code review");
    assert!(!disabled);
    assert_eq!(tooltip, None);
}

#[test]
fn imported_comment_code_review_button_uses_repo_tooltip_when_repo_disabled() {
    let repo_path = Path::new("/tmp/repo");
    let (label, disabled, tooltip) =
        ImportedCommentElementState::open_in_code_review_button_state(false, true, Some(repo_path));

    assert_eq!(label, "Open in code review");
    assert!(disabled);
    assert_eq!(
        tooltip,
        Some("Navigate to /tmp/repo to open these comments".to_string())
    );
}

#[test]
fn imported_comment_code_review_button_handles_repo_disable_without_path() {
    let (label, disabled, tooltip) =
        ImportedCommentElementState::open_in_code_review_button_state(false, true, None);

    assert_eq!(label, "Open in code review");
    assert!(disabled);
    assert_eq!(tooltip, None);
}

#[test]
fn imported_comment_code_review_button_prefers_added_state_over_repo_disable() {
    let repo_path = Path::new("/tmp/repo");
    let (label, disabled, tooltip) =
        ImportedCommentElementState::open_in_code_review_button_state(true, true, Some(repo_path));

    assert_eq!(label, "Added to code review");
    assert!(disabled);
    assert_eq!(tooltip, None);
}
