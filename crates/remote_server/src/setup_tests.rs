use super::*;

#[test]
fn parse_uname_linux_x86_64() {
    let platform = parse_uname_output("Linux x86_64").unwrap();
    assert_eq!(platform.os, RemoteOs::Linux);
    assert_eq!(platform.arch, RemoteArch::X86_64);
}

#[test]
fn parse_uname_linux_aarch64() {
    let platform = parse_uname_output("Linux aarch64").unwrap();
    assert_eq!(platform.os, RemoteOs::Linux);
    assert_eq!(platform.arch, RemoteArch::Aarch64);
}

#[test]
fn parse_uname_darwin_arm64() {
    let platform = parse_uname_output("Darwin arm64").unwrap();
    assert_eq!(platform.os, RemoteOs::MacOs);
    assert_eq!(platform.arch, RemoteArch::Aarch64);
}

#[test]
fn parse_uname_darwin_x86_64() {
    let platform = parse_uname_output("Darwin x86_64").unwrap();
    assert_eq!(platform.os, RemoteOs::MacOs);
    assert_eq!(platform.arch, RemoteArch::X86_64);
}

#[test]
fn parse_uname_linux_armv8l() {
    let platform = parse_uname_output("Linux armv8l").unwrap();
    assert_eq!(platform.os, RemoteOs::Linux);
    assert_eq!(platform.arch, RemoteArch::Aarch64);
}

#[test]
fn parse_uname_skips_shell_initialization_output() {
    let output = "Last login: Mon Apr  7 10:00:00 2025\nWelcome to Ubuntu\nLinux x86_64";
    let platform = parse_uname_output(output).unwrap();
    assert_eq!(platform.os, RemoteOs::Linux);
    assert_eq!(platform.arch, RemoteArch::X86_64);
}

#[test]
fn parse_uname_trims_whitespace() {
    let platform = parse_uname_output("  Linux x86_64  \n").unwrap();
    assert_eq!(platform.os, RemoteOs::Linux);
    assert_eq!(platform.arch, RemoteArch::X86_64);
}

#[test]
fn parse_uname_unsupported_os() {
    let result = parse_uname_output("Windows x86_64");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unsupported OS"));
}

#[test]
fn parse_uname_unsupported_arch() {
    let result = parse_uname_output("Linux mips");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unsupported arch"));
}

#[test]
fn parse_uname_empty_output() {
    let result = parse_uname_output("");
    assert!(result.is_err());
}

#[test]
fn parse_uname_missing_arch() {
    let result = parse_uname_output("Linux");
    assert!(result.is_err());
}

#[test]
fn state_is_ready() {
    assert!(RemoteServerSetupState::Ready.is_ready());
    assert!(!RemoteServerSetupState::Checking.is_ready());
    assert!(!RemoteServerSetupState::Initializing.is_ready());
}

#[test]
fn state_is_failed() {
    assert!(RemoteServerSetupState::Failed {
        error: "test".into()
    }
    .is_failed());
    assert!(!RemoteServerSetupState::Ready.is_failed());
}

#[test]
fn state_is_terminal() {
    assert!(RemoteServerSetupState::Ready.is_terminal());
    assert!(RemoteServerSetupState::Failed {
        error: "test".into()
    }
    .is_terminal());
    assert!(RemoteServerSetupState::Unsupported {
        reason: UnsupportedReason::NonGlibc {
            name: "musl".into()
        }
    }
    .is_terminal());
    assert!(!RemoteServerSetupState::Checking.is_terminal());
    assert!(!RemoteServerSetupState::Installing {
        progress_percent: None,
    }
    .is_terminal());
    assert!(!RemoteServerSetupState::Updating.is_terminal());
    assert!(!RemoteServerSetupState::Initializing.is_terminal());
}

#[test]
fn parse_preinstall_supported_glibc() {
    let stdout = "required_glibc=2.31\n\
                  libc_family=glibc\n\
                  libc_version=2.35\n\
                  status=supported\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(result.status, PreinstallStatus::Supported);
    assert_eq!(result.libc, RemoteLibc::Glibc(GlibcVersion::new(2, 35)));
    assert!(result.is_supported());
}

#[test]
fn parse_preinstall_unsupported_glibc_too_old() {
    let stdout = "required_glibc=2.31\n\
                  libc_family=glibc\n\
                  libc_version=2.17\n\
                  status=unsupported\n\
                  reason=glibc_too_old\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(
        result.status,
        PreinstallStatus::Unsupported {
            reason: UnsupportedReason::GlibcTooOld {
                detected: GlibcVersion::new(2, 17),
                required: GlibcVersion::new(2, 31),
            }
        }
    );
    assert!(!result.is_supported());
}

#[test]
fn parse_preinstall_unsupported_non_glibc() {
    let stdout = "required_glibc=2.31\n\
                  libc_family=musl\n\
                  status=unsupported\n\
                  reason=non_glibc\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(
        result.status,
        PreinstallStatus::Unsupported {
            reason: UnsupportedReason::NonGlibc {
                name: "musl".to_string()
            }
        }
    );
    assert_eq!(
        result.libc,
        RemoteLibc::NonGlibc {
            name: "musl".to_string()
        }
    );
    assert!(!result.is_supported());
}

/// Regression: the install script's tilde-expansion line must work in
/// macOS's stock `/bin/bash` 3.2.57. In bash 3.2, `"$HOME"` inside the
/// replacement of `${var/pattern/replacement}` is treated as 6 literal
/// characters (the quotes are not stripped), which turned
/// `~/.warp/remote-server` into the *relative* path
/// `"/Users/<user>"/.warp/remote-server` and silently steered the
/// install into a directory tree literally named `"`. The launch step
/// then looked at the real `$HOME/.warp/remote-server/oz-...`, found
/// nothing, and reported "no such file or directory" → "Response
/// channel closed".
///
/// This test drives the *actual* production script (via
/// [`install_script`]) rather than a hand-copied snippet so a future
/// change that reintroduces the quoted form can't slip past by being
/// dissimilar enough to dodge the static-grep check below. We
/// truncate just before `mkdir -p` so no filesystem side effects leak
/// out of the test, and append a marker `printf` to capture the
/// resolved `install_dir`.
#[test]
fn install_script_tilde_expansion_works_in_bash_3_2() {
    use std::process::{Command, Stdio};

    let bash = if std::path::Path::new("/bin/bash").exists() {
        "/bin/bash"
    } else {
        "bash"
    };

    let script = install_script(None);
    let cutoff = script.find("mkdir -p \"$install_dir\"").expect(
        "install script no longer contains the `mkdir -p \"$install_dir\"` \
         checkpoint this test relies on; update the test alongside the \
         script change",
    );
    let probe = format!(
        "{prefix}\nprintf '%s' \"$install_dir\"\nexit 0\n",
        prefix = &script[..cutoff],
    );

    let fake_home = "/Users/test";
    let output = Command::new(bash)
        .arg("-c")
        .arg(&probe)
        .env("HOME", fake_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn bash");

    assert!(
        output.status.success(),
        "bash exited with {:?}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    let install_dir = String::from_utf8_lossy(&output.stdout);
    assert!(
        !install_dir.contains('"'),
        "install_dir contains literal quote characters \
         (bash 3.2 quote-literal regression): {install_dir:?}",
    );

    // Cross-check against the production layout: tilde must resolve
    // to HOME, so the result equals `remote_server_dir()` with the
    // leading `~` replaced.
    let expected = remote_server_dir().replacen('~', fake_home, 1);
    assert_eq!(install_dir, expected);
}

/// Regression: guards against re-introducing any quoted form of the
/// tilde substitution by scanning the script source itself.
/// Complements [`install_script_tilde_expansion_works_in_bash_3_2`] —
/// the live bash test catches behavioural regressions, this static
/// check catches them earlier and rules out variants the live test
/// might miss (`"${HOME}"`, `'$HOME'`, etc.).
///
/// The rule: every `${var/#\~/...}` occurrence in the script must
/// have its replacement start with `$` (i.e. `$HOME` or `${HOME}`
/// directly). Anything else is a quoted form that bash 3.2 will
/// preserve as literal characters.
#[test]
fn install_script_does_not_quote_home_in_tilde_substitution() {
    let template = INSTALL_SCRIPT_TEMPLATE;
    let needle = r"/#\~/";
    let mut hits = 0;
    let mut search_from = 0;
    while let Some(off) = template[search_from..].find(needle) {
        let abs = search_from + off;
        let after = &template[abs + needle.len()..];
        let next = after.chars().next();
        let context_end = abs + needle.len() + after.len().min(16);
        let context = &template[abs..context_end];
        assert_eq!(
            next,
            Some('$'),
            "install_remote_server.sh has tilde substitution `{context}` \
             at byte {abs}; the replacement must start with `$` (i.e. \
             `$HOME` or `${{HOME}}`) — not a quote, single quote, or \
             anything else — because bash 3.2 (macOS /bin/bash) keeps \
             the surrounding characters literal in the replacement of \
             `${{var/pattern/replacement}}`",
        );
        hits += 1;
        search_from = abs + needle.len();
    }
    assert!(
        hits >= 1,
        "no `/#\\~/...` substitutions found in install_remote_server.sh \
         — has the script changed shape such that this test is no \
         longer guarding what it claims to?",
    );
}

#[test]
fn parse_preinstall_missing_status_falls_open() {
    // Garbled / partial script output — missing status field. Confirms
    // the fail-open invariant: anything we can't positively classify as
    // unsupported degrades to Unknown and is treated as supported, so a
    // flaky probe doesn't block the install.
    let stdout = "libc_family=glibc\nlibc_version=2.35\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(result.status, PreinstallStatus::Unknown);
    assert!(result.is_supported());
}
