use crate::util::extensions::TrimStringExt;

#[test]
fn test_trim_newline() {
    let mut string = "".to_string();
    string.trim_trailing_newline();
    assert_eq!("", string);

    let mut string = "test\n".to_string();
    string.trim_trailing_newline();
    assert_eq!("test", string);

    let mut string = "   test    \n".to_string();
    string.trim_trailing_newline();
    assert_eq!("   test    ", string);
}

/// TODO(CORE-3626): write an equivalent test with Windows paths.
#[cfg(not(windows))]
#[test]
fn test_resolve_command() {
    use crate::util::path::resolve_executable;
    use std::path::Path;

    assert_eq!(
        &resolve_executable("/bin/sh").unwrap(),
        Path::new("/bin/sh")
    );
    assert_eq!(
        &resolve_executable("env").unwrap(),
        Path::new("/usr/bin/env")
    );
    // This path exists in the Warp repo, so it should resolve. The `../`
    // is because Rust unit tests run from the root of the crate (`app` in
    // this case).
    assert_eq!(
        &resolve_executable("../script/run").unwrap(),
        Path::new("../script/run")
    );
    // `pwd` should always exist (it's also a shell builtin), but we won't
    // assume a specific location.
    assert!(resolve_executable("pwd").is_some());
    assert!(resolve_executable("unlikely-command").is_none());
    assert!(resolve_executable("nonexistent/relative/path").is_none());
    // src/main.rs does exist, but is not executable.
    assert!(resolve_executable("src/main.rs").is_none());
    // Note the trailing space.
    assert!(resolve_executable("zsh ").is_none());
}
