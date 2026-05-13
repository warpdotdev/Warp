use warp_completer::ParsedTokensSnapshot;
use warp_completer::util::parse_current_commands_and_tokens;

use crate::test_utils::CompletionContext;

use super::*;

async fn mock_parsed_input_token(buffer_text: String) -> ParsedTokensSnapshot {
    let completion_context = CompletionContext::new();
    parse_current_commands_and_tokens(buffer_text, &completion_context).await
}

fn clear_all_token_descriptions(snapshot: &mut ParsedTokensSnapshot) {
    for token in snapshot.parsed_tokens.iter_mut() {
        token.token_description = None;
    }
}

#[test]
fn test_is_likely_shell_command_one_off_keyword_short_circuits() {
    futures::executor::block_on(async move {
        // First token in `ONE_OFF_SHELL_COMMAND_KEYWORDS` should short-circuit to true
        let mut token = mock_parsed_input_token("sudo apt update".to_string()).await;
        let word_tokens_count = token.parsed_tokens.len();
        clear_all_token_descriptions(&mut token);
        assert!(is_likely_shell_command(&token, word_tokens_count).await);

        // Same short-circuit for `echo`.
        let mut token = mock_parsed_input_token("echo hello world".to_string()).await;
        let word_tokens_count = token.parsed_tokens.len();
        clear_all_token_descriptions(&mut token);
        assert!(is_likely_shell_command(&token, word_tokens_count).await);
    });
}

#[test]
fn test_is_likely_shell_command_first_token_with_description_short_input() {
    futures::executor::block_on(async move {
        // First token has a description (real command) and total word tokens < 3 should short-circuit to true.
        let token = mock_parsed_input_token("cargo --version".to_string()).await;
        assert!(is_likely_shell_command(&token, 2).await);
    });
}

#[test]
fn test_is_likely_shell_command_no_descriptions_returns_false() {
    futures::executor::block_on(async move {
        // None of the tokens have descriptions and none are one-off keywords should return false
        let mut token = mock_parsed_input_token("install --foo=bar baz".to_string()).await;
        let word_tokens_count = token.parsed_tokens.len();
        clear_all_token_descriptions(&mut token);
        assert!(!is_likely_shell_command(&token, word_tokens_count).await);
    });
}

#[test]
fn test_is_likely_shell_command_shell_syntax_no_longer_votes() {
    futures::executor::block_on(async move {
        // Tokens with shell-syntax characters (`-`, `=`, `/`) but not in whiltelist keywords
        // or not with token_description should return false
        let mut token =
            mock_parsed_input_token("git --foo=bar /path/to/file --baz".to_string()).await;
        let word_tokens_count = token.parsed_tokens.len();
        // Keep only the first token's description (mocking the completer
        // recognizing `git` but nothing else).
        for (idx, t) in token.parsed_tokens.iter_mut().enumerate() {
            if idx != 0 {
                t.token_description = None;
            }
        }
        // word_tokens_count >= 3 disables the short-input shortcut, so the only
        // path to true is the threshold check: 1/5 likely tokens = 0.2 < 0.5.
        assert!(word_tokens_count >= 3);
        assert!(!is_likely_shell_command(&token, word_tokens_count).await);
    });
}

#[test]
fn test_is_likely_shell_command_url_like_token_in_nl_prompt() {
    futures::executor::block_on(async move {
        // Natural-language prompt containing a URL should return false
        let mut token = mock_parsed_input_token(
            "read this https://example.com/foo-bar and summarize it".to_string(),
        )
        .await;
        let word_tokens_count = token.parsed_tokens.len();
        clear_all_token_descriptions(&mut token);
        assert!(!is_likely_shell_command(&token, word_tokens_count).await);
    });
}

#[test]
fn test_is_likely_shell_command_file_path_in_nl_prompt() {
    futures::executor::block_on(async move {
        // Natural-language prompt containing a file path should return false
        let mut token =
            mock_parsed_input_token("look at this /users/foo/bar.log file".to_string()).await;
        let word_tokens_count = token.parsed_tokens.len();
        clear_all_token_descriptions(&mut token);
        assert!(!is_likely_shell_command(&token, word_tokens_count).await);
    });
}

#[test]
fn test_is_likely_shell_command_majority_described_tokens() {
    futures::executor::block_on(async move {
        // Completer recognizes the majority of tokens (>= 50% for
        // 5+ tokens) should return true
        let token =
            mock_parsed_input_token("cargo build --release --workspace --all-features".to_string())
                .await;
        let word_tokens_count = token.parsed_tokens.len();
        assert!(is_likely_shell_command(&token, word_tokens_count).await);
    });
}
