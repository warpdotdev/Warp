use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// Small convenience: set of listing commands for tests. Matches the default set
// unless a test specifically overrides it.
const LS_ONLY: &[&str] = &["ls"];
const DEFAULTS: &[&str] = DEFAULT_LISTING_COMMANDS;

/// Test-only wrapper matching the pre-alias signature of `listing_command_argument_dir`.
/// Most tests don't care about alias resolution, so pass `None` by default. Tests that
/// specifically exercise alias handling call `listing_command_argument_dir` directly.
fn listing_command_argument_dir(
    command: &str,
    pwd: &Path,
    listing_commands: &[&str],
) -> Option<PathBuf> {
    super::listing_command_argument_dir(command, None, pwd, listing_commands)
}

#[test]
fn returns_none_for_non_listing_command() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    assert_eq!(
        listing_command_argument_dir("cat subdir/foo", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("git status", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("find . -name foo", tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn returns_none_for_ls_with_no_arg() {
    let tmp = tempdir().unwrap();
    assert_eq!(
        listing_command_argument_dir("ls", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("ls -la", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("ls --color=always", tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn returns_dir_for_ls_subdir() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    let got = listing_command_argument_dir("ls subdir", tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("subdir")));
}

#[test]
fn returns_dir_for_ls_subdir_trailing_slash() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    let got = listing_command_argument_dir("ls subdir/", tmp.path(), LS_ONLY);
    // PathBuf::join(Path::new("subdir/")) normalizes to "subdir", so compare canonically.
    assert_eq!(
        got.map(|p| p.canonicalize().unwrap()),
        Some(tmp.path().join("subdir").canonicalize().unwrap())
    );
}

#[test]
fn skips_flags_before_positional() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    for cmd in &[
        "ls -la subdir",
        "ls -la -A subdir",
        "ls --color=always subdir",
        "ls -l --color=always -A subdir",
    ] {
        let got = listing_command_argument_dir(cmd, tmp.path(), LS_ONLY);
        assert_eq!(
            got,
            Some(tmp.path().join("subdir")),
            "command {cmd:?} should resolve to subdir"
        );
    }
}

#[test]
fn returns_none_if_positional_does_not_exist() {
    let tmp = tempdir().unwrap();
    // no subdir created
    assert_eq!(
        listing_command_argument_dir("ls nonexistent", tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn returns_none_if_positional_is_a_file_not_a_dir() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("somefile"), b"hi").unwrap();

    // `ls somefile` is valid usage but it's listing a file, not a directory; a
    // file argument gives no useful directory root for output, so we return None
    // and let the default resolution (block.pwd) apply.
    assert_eq!(
        listing_command_argument_dir("ls somefile", tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn absolute_path_argument() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();
    let abs = tmp.path().join("subdir");

    // Pass an unrelated pwd; the absolute arg should win.
    let unrelated = tempdir().unwrap();
    let got =
        listing_command_argument_dir(&format!("ls {}", abs.display()), unrelated.path(), LS_ONLY);
    assert_eq!(got, Some(abs));
}

#[test]
fn quoted_argument_with_spaces() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("dir with spaces")).unwrap();

    let got = listing_command_argument_dir(r#"ls "dir with spaces""#, tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("dir with spaces")));

    let got_single = listing_command_argument_dir(r#"ls 'dir with spaces'"#, tmp.path(), LS_ONLY);
    assert_eq!(got_single, Some(tmp.path().join("dir with spaces")));
}

#[test]
fn rejects_multi_directory_operands() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("a")).unwrap();
    fs::create_dir(tmp.path().join("b")).unwrap();

    // Both positionals are directories → ambiguous, return None.
    assert_eq!(
        listing_command_argument_dir("ls a b", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("ls -la a b", tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn allows_single_dir_with_file_second_arg() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("a")).unwrap();
    fs::write(tmp.path().join("somefile"), b"hi").unwrap();

    // First positional is a dir, second is a file → not ambiguous multi-dir.
    let got = listing_command_argument_dir("ls a somefile", tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("a")));
}

#[test]
fn returns_none_for_malformed_command() {
    let tmp = tempdir().unwrap();
    // shlex::split returns None for unclosed quotes.
    assert_eq!(
        listing_command_argument_dir(r#"ls "unterminated"#, tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn returns_none_for_empty_command() {
    let tmp = tempdir().unwrap();
    assert_eq!(listing_command_argument_dir("", tmp.path(), LS_ONLY), None);
    assert_eq!(
        listing_command_argument_dir("   ", tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn honors_custom_listing_command_set() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    // By default, `eza` is a listing command via DEFAULTS.
    assert_eq!(
        listing_command_argument_dir("eza subdir", tmp.path(), DEFAULTS),
        Some(tmp.path().join("subdir"))
    );

    // But if the user narrows to just "ls", `eza` no longer triggers.
    assert_eq!(
        listing_command_argument_dir("eza subdir", tmp.path(), LS_ONLY),
        None
    );

    // A user-defined alias like "ll" can be added to the set.
    let user_set: &[&str] = &["ls", "ll"];
    assert_eq!(
        listing_command_argument_dir("ll subdir", tmp.path(), user_set),
        Some(tmp.path().join("subdir"))
    );
}

#[test]
fn skips_leading_env_var_assignments() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    let got = listing_command_argument_dir("LS_COLORS=auto ls subdir", tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("subdir")));

    let got_multi = listing_command_argument_dir("FOO=1 BAR=2 ls subdir", tmp.path(), LS_ONLY);
    assert_eq!(got_multi, Some(tmp.path().join("subdir")));
}

#[test]
fn tilde_expansion_of_argument() {
    // Only run if HOME is set to a real directory that exists.
    let Some(home) = dirs::home_dir() else {
        return;
    };
    if !home.is_dir() {
        return;
    }

    let got = listing_command_argument_dir("ls ~", home.parent().unwrap_or(&home), LS_ONLY);
    assert_eq!(got, Some(home.clone()));

    // `~/` on its own expands to $HOME.
    let got_slash = listing_command_argument_dir("ls ~/", home.parent().unwrap_or(&home), LS_ONLY);
    assert_eq!(got_slash, Some(home));
}

#[test]
fn does_not_double_join_when_output_paths_are_already_rooted() {
    // This test documents the intended contract: `listing_command_argument_dir`
    // only extracts the command's literal directory argument. It does NOT and
    // MUST NOT try to be clever about commands like `find DIR -name foo` where
    // the output already contains `DIR/foo` — for those, the caller should not
    // invoke this helper (or the command should not be in `listing_commands`).
    //
    // This is enforced by `listing_commands` being an allowlist: callers
    // opt commands in explicitly. `find` is not in the default set.
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    assert_eq!(
        listing_command_argument_dir("find subdir -name foo", tmp.path(), DEFAULTS),
        None,
        "find is not in DEFAULT_LISTING_COMMANDS — confirms allowlist semantics"
    );
}

#[test]
fn realistic_ls_variants() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("specs")).unwrap();

    // The exact forms users type every day.
    let variants = &[
        "ls specs",
        "ls specs/",
        "ls -la specs",
        "ls -la specs/",
        "ls -A specs",
        "ls --color=always specs",
        "ls -l --color=always specs",
    ];
    for v in variants {
        let got = listing_command_argument_dir(v, tmp.path(), LS_ONLY);
        assert_eq!(
            got.map(|p| p.canonicalize().unwrap()),
            Some(tmp.path().join("specs").canonicalize().unwrap()),
            "variant {v:?} should resolve to specs"
        );
    }
}

// -- Alias resolution tests --
//
// These tests exercise the `resolved_command_name` override that callers use when
// they've already resolved shell aliases upstream (in Warp, via
// `Block::top_level_command(sessions)`).

#[test]
fn alias_resolved_name_triggers_listing_lookup() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    // Raw command is `ll subdir` (the user's alias); resolved name is `ls`.
    // `ll` is not in LS_ONLY, so without the resolved-name override this would
    // return None. With the override, it resolves as if the command were `ls subdir`.
    let got = super::listing_command_argument_dir("ll subdir", Some("ls"), tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("subdir")));
}

#[test]
fn alias_resolved_name_respects_flags_in_raw_command() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    // Common alias: `alias la='ls -A'`. The user types `la subdir`; after alias
    // expansion the effective command is `ls -A subdir`. We don't need to see the
    // `-A` to parse correctly — our tokenizer still skips flags in the raw command
    // and picks up `subdir` as the first positional.
    let got = super::listing_command_argument_dir("la subdir", Some("ls"), tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("subdir")));
}

#[test]
fn alias_resolved_name_overrides_raw_token_for_matching_only() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    // If the resolved name is NOT a listing command, we return None even if the
    // raw first token (`ls`) would have matched. The resolved name is authoritative.
    let got = super::listing_command_argument_dir("ls subdir", Some("bat"), tmp.path(), LS_ONLY);
    assert_eq!(got, None);
}

#[test]
fn alias_resolution_uses_raw_tokens_for_positional_extraction() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("user_typed_dir")).unwrap();

    // Known limitation: aliases with baked-in positional args are not expanded.
    // `alias lsd='ls /tmp'; lsd user_typed_dir` → raw tokens are
    // `["lsd", "user_typed_dir"]`, resolved name is `ls`. We pick
    // `user_typed_dir` as the first positional (the user's typed arg), not
    // `/tmp` (the aliased arg). This test documents that behavior.
    let got =
        super::listing_command_argument_dir("lsd user_typed_dir", Some("ls"), tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("user_typed_dir")));
}

#[test]
fn alias_resolution_with_no_resolved_name_falls_back_to_raw_token() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    // When `resolved_command_name` is `None`, behavior matches the pre-alias
    // contract (match against raw first token).
    let got = super::listing_command_argument_dir("ls subdir", None, tmp.path(), LS_ONLY);
    assert_eq!(got, Some(tmp.path().join("subdir")));

    let got_miss = super::listing_command_argument_dir("ll subdir", None, tmp.path(), LS_ONLY);
    assert_eq!(got_miss, None, "without resolved name, ll misses");
}

#[test]
fn rejects_recursive_flag_short() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    assert_eq!(
        listing_command_argument_dir("ls -R subdir", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("ls -lR subdir", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("ls -laR subdir", tmp.path(), LS_ONLY),
        None
    );
}

#[test]
fn rejects_recursive_flag_long() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    assert_eq!(
        listing_command_argument_dir("ls --recursive subdir", tmp.path(), LS_ONLY),
        None
    );
    assert_eq!(
        listing_command_argument_dir("ls -la --recursive subdir", tmp.path(), LS_ONLY),
        None
    );
}
