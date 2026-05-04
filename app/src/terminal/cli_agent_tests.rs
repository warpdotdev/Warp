use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Local;
use smol_str::SmolStr;
use warp_editor::render::model::LineCount;
use warp_util::path::EscapeChar;
use warpui::App;

use super::{
    build_diff_hunk_prompt, build_review_prompt, build_selection_line_range_prompt,
    build_selection_substring_prompt, CLIAgent, UBER_TEAM_UID,
};
use crate::ai::agent::{AgentReviewCommentBatch, DiffSetHunk};
use crate::code::editor::line::EditorLineLocation;
use crate::code_review::comments::{
    AttachedReviewComment, AttachedReviewCommentTarget, CommentOrigin, LineDiffContent,
};
use crate::server::ids::ServerId;
use crate::server::server_api::team::MockTeamClient;
use crate::server::server_api::workspace::MockWorkspaceClient;
use crate::workspaces::team::Team;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::Workspace;

/// Helper to build an alias map from pairs.
fn aliases(pairs: &[(&str, &str)]) -> HashMap<SmolStr, String> {
    pairs
        .iter()
        .map(|(k, v)| (SmolStr::new(k), v.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers for prompt-building tests
// ---------------------------------------------------------------------------

fn make_comment(
    content: &str,
    target: AttachedReviewCommentTarget,
    outdated: bool,
) -> AttachedReviewComment {
    AttachedReviewComment {
        id: Default::default(),
        content: content.to_string(),
        target,
        last_update_time: Local::now(),
        base: None,
        head: None,
        outdated,
        origin: CommentOrigin::Native,
    }
}

fn batch(comments: Vec<AttachedReviewComment>) -> AgentReviewCommentBatch {
    AgentReviewCommentBatch {
        comments,
        diff_set: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// build_review_prompt tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_review_prompt_current_line_is_1_indexed() {
    // LineCount 0 (0-indexed) should appear as L1 in the prompt.
    let comment = make_comment(
        "fix this",
        AttachedReviewCommentTarget::Line {
            absolute_file_path: PathBuf::from("/repo/src/main.rs"),
            line: EditorLineLocation::Current {
                line_number: LineCount::from(0),
                line_range: LineCount::from(0)..LineCount::from(1),
            },
            content: LineDiffContent::default(),
        },
        false,
    );
    let prompt = build_review_prompt(&batch(vec![comment]));
    assert!(
        prompt.contains("/repo/src/main.rs L1"),
        "expected 1-indexed L1, got: {prompt}",
    );
    assert!(prompt.contains("fix this"));
}

#[test]
fn test_build_review_prompt_removed_line_is_1_indexed() {
    let comment = make_comment(
        "why was this deleted?",
        AttachedReviewCommentTarget::Line {
            absolute_file_path: PathBuf::from("/repo/old.rs"),
            line: EditorLineLocation::Removed {
                line_number: LineCount::from(9),
                line_range: LineCount::from(9)..LineCount::from(10),
                index: 0,
            },
            content: LineDiffContent::default(),
        },
        false,
    );
    let prompt = build_review_prompt(&batch(vec![comment]));
    assert!(
        prompt.contains("(deleted, was L10"),
        "expected 1-indexed L10, got: {prompt}",
    );
}

#[test]
fn test_build_review_prompt_collapsed_range_is_1_indexed_start() {
    let comment = make_comment(
        "check this hunk",
        AttachedReviewCommentTarget::Line {
            absolute_file_path: PathBuf::from("/repo/lib.rs"),
            line: EditorLineLocation::Collapsed {
                line_range: LineCount::from(4)..LineCount::from(10),
            },
            content: LineDiffContent::default(),
        },
        false,
    );
    let prompt = build_review_prompt(&batch(vec![comment]));
    // line_range is [4, 10) 0-indexed -> L5-L10 (1-indexed, both ends inclusive)
    assert!(prompt.contains("L5-L10"), "expected L5-L10, got: {prompt}",);
}

#[test]
fn test_build_review_prompt_file_level_comment() {
    let comment = make_comment(
        "needs refactoring",
        AttachedReviewCommentTarget::File {
            absolute_file_path: PathBuf::from("/repo/src/utils.rs"),
        },
        false,
    );
    let prompt = build_review_prompt(&batch(vec![comment]));
    assert!(prompt.contains("/repo/src/utils.rs: needs refactoring"));
    // Not a deleted file (empty diff_set), so no "deleted file" text.
    assert!(!prompt.contains("deleted file"));
}

#[test]
fn test_build_review_prompt_deleted_file_comment() {
    let comment = make_comment(
        "why remove this?",
        AttachedReviewCommentTarget::File {
            absolute_file_path: PathBuf::from("/repo/src/old.rs"),
        },
        false,
    );
    let mut review = batch(vec![comment]);
    review.diff_set.insert(
        "src/old.rs".to_string(),
        vec![DiffSetHunk {
            line_range: LineCount::from(0)..LineCount::from(5),
            diff_content: String::new(),
            lines_added: 0,
            lines_removed: 5,
        }],
    );
    let prompt = build_review_prompt(&review);
    assert!(
        prompt.contains("(deleted file"),
        "expected deleted file annotation, got: {prompt}",
    );
}

#[test]
fn test_build_review_prompt_general_comment() {
    let comment = make_comment(
        "overall looks good",
        AttachedReviewCommentTarget::General,
        false,
    );
    let prompt = build_review_prompt(&batch(vec![comment]));
    assert!(prompt.contains("General: overall looks good"));
}

#[test]
fn test_build_review_prompt_skips_outdated_comments() {
    let active = make_comment("keep me", AttachedReviewCommentTarget::General, false);
    let outdated = make_comment("skip me", AttachedReviewCommentTarget::General, true);
    let prompt = build_review_prompt(&batch(vec![active, outdated]));
    assert!(prompt.contains("keep me"));
    assert!(!prompt.contains("skip me"));
}

#[test]
fn test_build_review_prompt_multiple_comments() {
    let c1 = make_comment(
        "first",
        AttachedReviewCommentTarget::Line {
            absolute_file_path: PathBuf::from("/repo/a.rs"),
            line: EditorLineLocation::Current {
                line_number: LineCount::from(4),
                line_range: LineCount::from(4)..LineCount::from(5),
            },
            content: LineDiffContent::default(),
        },
        false,
    );
    let c2 = make_comment("second", AttachedReviewCommentTarget::General, false);
    let prompt = build_review_prompt(&batch(vec![c1, c2]));
    assert!(prompt.contains("/repo/a.rs L5: first"));
    assert!(prompt.contains("General: second"));
}

#[test]
fn test_build_review_prompt_exports_internal_markdown_without_punctuation_escapes() {
    let comment = make_comment("Fix this\\.", AttachedReviewCommentTarget::General, false);
    let prompt = build_review_prompt(&batch(vec![comment]));
    assert!(prompt.contains("General: Fix this."));
    assert!(!prompt.contains("Fix this\\."));
}

// ---------------------------------------------------------------------------
// build_diff_hunk_prompt tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_diff_hunk_prompt_format() {
    let prompt = build_diff_hunk_prompt(Path::new("/repo/src/main.rs"), 10, 20, 3, 2);
    assert_eq!(
        prompt,
        "/repo/src/main.rs L10-L20 (+3 -2) -- run `git diff` to see the full context.",
    );
}

// ---------------------------------------------------------------------------
// build_selection_line_range_prompt tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_selection_line_range_prompt_format() {
    let result = build_selection_line_range_prompt("src/foo.rs", 5, 10);
    assert_eq!(result, "src/foo.rs L5-L10");
}

#[test]
fn test_build_selection_substring_prompt_format() {
    let result = build_selection_substring_prompt("src/foo.rs", 5, "let x = 42;");
    assert_eq!(result, "src/foo.rs L5: let x = 42;");
}

#[test]
fn test_detect_known_agents() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            for (command, expected) in [
                ("claude", CLIAgent::Claude),
                ("gemini", CLIAgent::Gemini),
                ("codex", CLIAgent::Codex),
                ("amp", CLIAgent::Amp),
                ("droid", CLIAgent::Droid),
                ("opencode", CLIAgent::OpenCode),
                ("copilot", CLIAgent::Copilot),
                ("agent", CLIAgent::CursorCli),
                ("goose", CLIAgent::Goose),
                ("vibe", CLIAgent::Vibe),
            ] {
                assert_eq!(
                    CLIAgent::detect(command, None, None, ctx),
                    Some(expected),
                    "failed to detect {command}",
                );
            }
        });
    });
}

#[test]
fn test_detect_with_arguments() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("claude --model opus", None, None, ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("gemini chat", None, None, ctx),
                Some(CLIAgent::Gemini),
            );
        });
    });
}

#[test]
fn test_detect_vibe_acp_binary() {
    // The mistral-vibe package ships a `vibe-acp` ACP-mode binary alongside
    // the user-facing `vibe` TUI. Both must be detected as the same agent.
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("vibe-acp", None, None, ctx),
                Some(CLIAgent::Vibe),
            );
            assert_eq!(
                CLIAgent::detect("vibe-acp --some-flag", None, None, ctx),
                Some(CLIAgent::Vibe),
            );
            // Distinct binary names should not bleed into Vibe.
            assert_eq!(CLIAgent::detect("vibe-other", None, None, ctx), None);
        });
    });
}

#[test]
fn test_detect_with_leading_whitespace() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("  claude", None, None, ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("\tclaude --help", None, None, ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_no_match() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(CLIAgent::detect("ls -la", None, None, ctx), None);
            assert_eq!(CLIAgent::detect("vim", None, None, ctx), None);
            assert_eq!(CLIAgent::detect("claude_wrapper", None, None, ctx), None);
        });
    });
}

#[test]
fn test_detect_with_alias() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            let map = aliases(&[("c", "claude")]);
            assert_eq!(
                CLIAgent::detect("c", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("c --help", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_alias_not_matching() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            let map = aliases(&[("c", "cat")]);
            assert_eq!(CLIAgent::detect("c", None, Some(&map), ctx), None);
        });
    });
}

#[test]
fn test_detect_alias_multi_word_value() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            // Alias whose value starts with "gemini" but has extra words
            let map = aliases(&[("g", "gemini chat --verbose")]);
            assert_eq!(
                CLIAgent::detect("g", None, Some(&map), ctx),
                Some(CLIAgent::Gemini),
            );
        });
    });
}

#[test]
fn test_detect_with_env_var_prefix() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect(
                    "EXAMPLE=true opencode",
                    Some(EscapeChar::Backslash),
                    None,
                    ctx,
                ),
                Some(CLIAgent::OpenCode),
            );
        });
    });
}

#[test]
fn test_detect_with_multiple_env_vars() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect(
                    "FOO=1 BAR=2 opencode --flag",
                    Some(EscapeChar::Backslash),
                    None,
                    ctx,
                ),
                Some(CLIAgent::OpenCode),
            );
        });
    });
}

#[test]
fn test_detect_with_alias_and_env_var() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            let map = aliases(&[("oc", "EXAMPLE=1 opencode")]);
            assert_eq!(
                CLIAgent::detect("oc --flag", Some(EscapeChar::Backslash), Some(&map), ctx,),
                Some(CLIAgent::OpenCode),
            );
        });
    });
}

/// Creates a workspace containing a team with the given UID.
fn workspace_with_team_uid(uid: &str) -> Workspace {
    Workspace::from_local_cache(
        ServerId::from_string_lossy("test-workspace-uid-001").into(),
        "Test Workspace".to_string(),
        Some(vec![Team::from_local_cache(
            ServerId::from_string_lossy(uid),
            "Test Team".to_string(),
            None,
            None,
            None,
        )]),
    )
}

#[test]
fn test_detect_aifx_agent_run_claude_on_uber_team() {
    App::test((), |mut app| async move {
        let uber_workspace = workspace_with_team_uid(UBER_TEAM_UID);
        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(
                Arc::new(MockTeamClient::new()),
                Arc::new(MockWorkspaceClient::new()),
                vec![uber_workspace],
                ctx,
            )
        });

        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("aifx agent run claude", None, None, ctx),
                Some(CLIAgent::Claude),
            );
            // With extra args
            assert_eq!(
                CLIAgent::detect("aifx agent run claude --verbose", None, None, ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_aifx_agent_run_claude_via_alias_on_uber_team() {
    App::test((), |mut app| async move {
        let uber_workspace = workspace_with_team_uid(UBER_TEAM_UID);
        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(
                Arc::new(MockTeamClient::new()),
                Arc::new(MockWorkspaceClient::new()),
                vec![uber_workspace],
                ctx,
            )
        });

        app.update(|ctx| {
            let map = aliases(&[("ai", "aifx agent run claude")]);
            assert_eq!(
                CLIAgent::detect("ai", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("ai --flag", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_aifx_agent_run_claude_not_on_uber_team() {
    App::test((), |mut app| async move {
        // Register UserWorkspaces with no Uber team membership
        app.add_singleton_model(UserWorkspaces::default_mock);

        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("aifx agent run claude", None, None, ctx),
                None,
            );
        });
    });
}

#[test]
fn test_serialized_name_round_trips_known_agents() {
    for agent in enum_iterator::all::<CLIAgent>() {
        let name = agent.to_serialized_name();
        if agent == CLIAgent::Unknown {
            assert_eq!(name, "Unknown");
        } else {
            assert!(!name.is_empty(), "empty serialized name for {agent:?}");
        }
        assert_eq!(
            CLIAgent::from_serialized_name(&name),
            agent,
            "round-trip failed for {agent:?} with serialized name {name:?}",
        );
    }
}

#[test]
fn test_from_serialized_name_falls_back_to_unknown() {
    assert_eq!(CLIAgent::from_serialized_name(""), CLIAgent::Unknown);
    assert_eq!(
        CLIAgent::from_serialized_name("nonexistent"),
        CLIAgent::Unknown
    );
}

/// Codex on Linux: shebang scripts surface as `node /path/to/script` because
/// `/proc/PID/comm` reports the runtime, not the script. The detection must
/// recognize the script's basename as the agent identity.
/// See #9870.
#[test]
fn test_detect_node_shebang_script_codex_linux() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            // The most direct repro from the issue: node + script path.
            assert_eq!(
                CLIAgent::detect("node /usr/local/bin/codex", None, None, ctx),
                Some(CLIAgent::Codex),
            );
            // With trailing arguments.
            assert_eq!(
                CLIAgent::detect(
                    "node /usr/local/bin/codex --some-flag",
                    None,
                    None,
                    ctx
                ),
                Some(CLIAgent::Codex),
            );
            // Script with `.js` extension stripped.
            assert_eq!(
                CLIAgent::detect(
                    "node /usr/local/lib/node_modules/@openai/codex/bin/codex.js",
                    None,
                    None,
                    ctx
                ),
                Some(CLIAgent::Codex),
            );
            // Other recognized runtimes resolve the same way.
            assert_eq!(
                CLIAgent::detect("nodejs /opt/codex", None, None, ctx),
                Some(CLIAgent::Codex),
            );
            assert_eq!(
                CLIAgent::detect("bun /opt/codex.js", None, None, ctx),
                Some(CLIAgent::Codex),
            );
            // Runtime flags before the script path don't break recovery.
            assert_eq!(
                CLIAgent::detect(
                    "node --inspect /usr/local/bin/codex",
                    None,
                    None,
                    ctx
                ),
                Some(CLIAgent::Codex),
            );
        });
    });
}

/// Path-prefixed binaries also resolve via basename matching, in case Warp
/// ever surfaces a fully-qualified command (e.g. when shell history records
/// the resolved path or when a user types it explicitly).
#[test]
fn test_detect_path_prefixed_agent_binary() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("/usr/local/bin/codex", None, None, ctx),
                Some(CLIAgent::Codex),
            );
            assert_eq!(
                CLIAgent::detect("/opt/homebrew/bin/claude --model opus", None, None, ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("./goose", None, None, ctx),
                Some(CLIAgent::Goose),
            );
        });
    });
}

/// Bare runtime invocations without a recognized agent script must NOT
/// resolve to any agent — we don't want `node ./my-app.js` to false-positive.
#[test]
fn test_detect_node_script_without_agent_basename() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(CLIAgent::detect("node", None, None, ctx), None);
            assert_eq!(
                CLIAgent::detect("node /home/user/my-app.js", None, None, ctx),
                None,
            );
            assert_eq!(
                CLIAgent::detect("python /home/user/script.py", None, None, ctx),
                None,
            );
            // A runtime with only flags — no script — must not match.
            assert_eq!(
                CLIAgent::detect("node --version", None, None, ctx),
                None,
            );
        });
    });
}

/// Value-taking runtime flags (e.g. `node -e <code>`, `python -c <code>`)
/// must NOT cause the eval/script-string argument to false-positive as an
/// agent. Per oz-for-oss review on PR #10022.
#[test]
fn test_detect_node_value_taking_flags_do_not_false_positive() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            // `-e` consumes `codex` as the eval string, not as a script path.
            assert_eq!(
                CLIAgent::detect("node -e codex", None, None, ctx),
                None,
            );
            // Same with the long form.
            assert_eq!(
                CLIAgent::detect("node --eval codex", None, None, ctx),
                None,
            );
            // `--require` consumes its module argument.
            assert_eq!(
                CLIAgent::detect("node --require claude /home/user/app.js", None, None, ctx),
                None,
            );
            // `python -c` consumes the code string.
            assert_eq!(
                CLIAgent::detect("python -c claude", None, None, ctx),
                None,
            );
            // `python -m` consumes the module name.
            assert_eq!(
                CLIAgent::detect("python -m gemini", None, None, ctx),
                None,
            );

            // After the value-taking flag and its argument are consumed, a
            // legitimate agent script that follows IS still detected.
            assert_eq!(
                CLIAgent::detect("node --require some-mod /usr/local/bin/codex", None, None, ctx),
                Some(CLIAgent::Codex),
            );
            // `--key=value` form keeps the value attached to the flag, so a
            // following script positional is still found correctly.
            assert_eq!(
                CLIAgent::detect("node --inspect=127.0.0.1:9229 /usr/local/bin/codex", None, None, ctx),
                Some(CLIAgent::Codex),
            );
        });
    });
}

#[test]
fn test_detect_aifx_agent_run_claude_wrong_team() {
    App::test((), |mut app| async move {
        let other_workspace = workspace_with_team_uid("some-other-team-uid-01");
        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(
                Arc::new(MockTeamClient::new()),
                Arc::new(MockWorkspaceClient::new()),
                vec![other_workspace],
                ctx,
            )
        });

        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("aifx agent run claude", None, None, ctx),
                None,
            );
        });
    });
}
