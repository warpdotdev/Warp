use super::*;

#[test]
fn test_program_invalid_bash() {
    // This test assumes there is no bash binary at /some/weird/path/bash.
    let shell_path = "/some/weird/path/bash".to_owned();
    assert!(supported_shell_path_and_type(&shell_path).is_none());
}

#[test]
fn test_program_invalid_zsh() {
    // This test assumes there is no bash zsh at /some/weird/path/bash.
    let shell_path = "/some/weird/path/zsh".to_owned();
    assert!(supported_shell_path_and_type(&shell_path).is_none());
}

#[test]
fn test_program_unknown_shell() {
    let shell_path = "/some/weird/path/wtfsh".to_owned();
    assert!(supported_shell_path_and_type(&shell_path).is_none());
}

#[test]
fn test_program_nu_shell() {
    assert!(matches!(
        parse_shell_type_from_path(Path::new("/usr/bin/nu")),
        Some((_, ShellType::Nu))
    ));
}

#[test]
fn test_program_nu_exe_shell() {
    assert!(matches!(
        parse_shell_type_from_path(Path::new(
            "C:\\Users\\user\\scoop\\apps\\nu\\current\\nu.exe"
        )),
        Some((_, ShellType::Nu))
    ));
    assert!(
        parse_shell_type_from_path(Path::new("C:\\Users\\user\\scoop\\apps\\menu.exe")).is_none()
    );
}

#[test]
fn test_wsl_shell_type_from_path_matches_basename() {
    assert_eq!(wsl_shell_type_from_path("/usr/bin/nu"), Some(ShellType::Nu));
    assert_eq!(wsl_shell_type_from_path("/usr/bin/menu.exe"), None);
}

#[test]
fn test_trim_wsl_err_from_output() {
    assert_eq!(
        take_until_utf16_crlf(b"/bin/bash\n".to_vec()),
        b"/bin/bash\n".to_vec()
    );
    assert_eq!(
        take_until_utf16_crlf(b"/bin/bash\n\r\0\n\0W\0A\0R\0N\0I\0N\0G\0".to_vec()),
        b"/bin/bash\n".to_vec()
    );
}
