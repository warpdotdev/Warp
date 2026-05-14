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
    match result {
        Err(crate::transport::Error::UnsupportedOs { os }) => {
            assert_eq!(os, "Windows");
        }
        other => panic!("expected UnsupportedOs, got {other:?}"),
    }
}

#[test]
fn parse_uname_unsupported_arch() {
    let result = parse_uname_output("Linux mips");
    match result {
        Err(crate::transport::Error::UnsupportedArch { arch }) => {
            assert_eq!(arch, "mips");
        }
        other => panic!("expected UnsupportedArch, got {other:?}"),
    }
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
fn remote_server_identity_data_dir_uses_encoded_identity_directory() {
    let data_dir = remote_server_daemon_data_dir("user@example.com/ssh host");
    assert_eq!(
        data_dir,
        format!(
            "{}/user%40example%2Ecom%2Fssh%20host/data",
            remote_server_dir()
        )
    );
}

#[test]
fn remote_server_identity_data_dir_handles_empty_identity_key() {
    let data_dir = remote_server_daemon_data_dir("");
    assert_eq!(data_dir, format!("{}/empty/data", remote_server_dir()));
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

/// Regression: the install script's tilde-expansion logic must work
/// across the bash versions we actually invoke at install time
/// (`run_ssh_script` pipes the script into `bash -s` on the remote).
/// Two interpreter quirks have to be avoided simultaneously:
///
///   1. bash 3.2 (macOS `/bin/bash`) keeps inner double-quotes around
///      the replacement of `${var/pattern/replacement}` literal, so
///      `"$HOME"` ends up as 6 literal characters and the install
///      lands under a directory tree literally named `"`.
///   2. bash 5.2+ with `patsub_replacement` (default-on) treats `&`
///      in the replacement as the matched pattern, so a `$HOME`
///      containing `&` resolves to a `~`-substituted path.
///
/// Both bugs surface as the install binary landing somewhere Warp's
/// launch step doesn't look, producing a misleading "Response channel
/// closed before receiving a reply".
///
/// This test drives the *actual* production script (via
/// [`install_script`]) rather than a hand-copied snippet, and runs it
/// against several `HOME` values to exercise the patsub-`&` trap as
/// well as the quote-literal trap. We truncate just before `mkdir -p`
/// so no filesystem side effects leak out of the test, and append a
/// marker `printf` to capture the resolved `install_dir`.
///
/// Gated to Unix because the test invokes `/bin/bash` (or `bash` from
/// PATH) directly. The bug only matters on Unix remotes anyway —
/// Warp's remote-server SSH wrapper doesn't target Windows hosts.
#[cfg(unix)]
#[test]
fn install_script_tilde_expansion_resolves_correctly() {
    use command::blocking::Command;
    use std::process::Stdio;

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

    // Run the probe against a matrix of HOME values. The first is an
    // ordinary path; the second contains `&`, which exercises bash
    // 5.2's patsub_replacement (where it would otherwise expand to
    // the matched `~`).
    let cases = [
        ("/Users/test", "ordinary HOME"),
        (
            "/Users/A&B",
            "HOME with `&` (bash 5.2 patsub_replacement trap)",
        ),
    ];

    for (fake_home, label) in cases {
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
            "[{label}] bash exited with {:?}: stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );

        let install_dir = String::from_utf8_lossy(&output.stdout);
        assert!(
            !install_dir.contains('"'),
            "[{label}] install_dir contains literal quote characters \
             (bash 3.2 quote-literal regression): {install_dir:?}",
        );

        // Cross-check against the production layout: tilde must
        // resolve to HOME, so the result equals `remote_server_dir()`
        // with the leading `~` replaced.
        let expected = remote_server_dir().replacen('~', fake_home, 1);
        assert_eq!(
            install_dir, expected,
            "[{label}] install_dir resolved incorrectly",
        );
    }
}

/// Regression: guards against re-introducing the
/// `${var/pattern/replacement}` tilde-substitution form, which has two
/// known interpreter bugs (see
/// [`install_script_tilde_expansion_resolves_correctly`] for details).
/// Complements the live bash test — the live test catches behavioural
/// regressions, this static check fails fast and explains *why* in
/// the assertion message so a future contributor doesn't have to
/// re-discover the constraints from a CI failure.
#[test]
fn install_script_avoids_pattern_substitution_for_tilde_expansion() {
    let template = INSTALL_SCRIPT_TEMPLATE;
    assert!(
        !template.contains(r"/#\~/"),
        "install_remote_server.sh uses `${{var/#\\~/...}}` for tilde \
         expansion. This form has two known interpreter bugs that \
         silently mis-resolve the install path:\n\
         \n\
           1. bash 3.2 (macOS /bin/bash) keeps inner double-quotes \
              around the replacement literal, so `\"$HOME\"` ends up \
              as 6 literal characters including the quotes.\n\
           2. bash 5.2+ enables `patsub_replacement` by default, which \
              makes `&` in the replacement expand to the matched \
              pattern, so a `$HOME` containing `&` resolves wrong.\n\
         \n\
         Use `case`/`${{var#\\~}}` instead — see install_remote_server.sh \
         for the pattern.",
    );
}

#[test]
fn parse_uname_linux_amd64() {
    let platform = parse_uname_output("Linux amd64").unwrap();
    assert_eq!(platform.os, RemoteOs::Linux);
    assert_eq!(platform.arch, RemoteArch::X86_64);
}

#[test]
fn parse_uname_unsupported_armv7l() {
    let result = parse_uname_output("Linux armv7l");
    match result {
        Err(crate::transport::Error::UnsupportedArch { arch }) => {
            assert_eq!(arch, "armv7l");
        }
        other => panic!("expected UnsupportedArch, got {other:?}"),
    }
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

#[test]
fn parse_uname_unsupported_cygwin_os() {
    let result = parse_uname_output("CYGWIN_NT-10.0-22621 x86_64");
    match result {
        Err(crate::transport::Error::UnsupportedOs { os }) => {
            assert_eq!(os, "CYGWIN_NT-10.0-22621");
        }
        other => panic!("expected UnsupportedOs, got {other:?}"),
    }
}

#[test]
fn parse_uname_unsupported_armv6l() {
    let result = parse_uname_output("Linux armv6l");
    match result {
        Err(crate::transport::Error::UnsupportedArch { arch }) => {
            assert_eq!(arch, "armv6l");
        }
        other => panic!("expected UnsupportedArch, got {other:?}"),
    }
}

#[test]
fn unsupported_reason_from_transport_error_maps_os_and_arch() {
    assert_eq!(
        unsupported_reason_from_transport_error(&crate::transport::Error::UnsupportedOs {
            os: "CYGWIN_NT-10.0-22621".into(),
        }),
        Some(UnsupportedReason::UnsupportedOs {
            os: "CYGWIN_NT-10.0-22621".into(),
        })
    );
    assert_eq!(
        unsupported_reason_from_transport_error(&crate::transport::Error::UnsupportedArch {
            arch: "armv7l".into(),
        }),
        Some(UnsupportedReason::UnsupportedArch {
            arch: "armv7l".into(),
        })
    );
}

#[cfg(unix)]
#[test]
fn install_script_platform_mapping_handles_supported_and_unsupported_targets() {
    use command::blocking::Command;
    use std::process::Stdio;

    let bash = if std::path::Path::new("/bin/bash").exists() {
        "/bin/bash"
    } else {
        "bash"
    };

    let script = install_script(None);
    let cutoff = script.find("install_dir=").expect(
        "install script no longer contains the install_dir checkpoint this test relies on; update the test alongside the script change",
    );
    let prefix = &script[..cutoff];

    struct Case<'a> {
        name: &'a str,
        os: &'a str,
        arch: &'a str,
        expected_status: i32,
        expected_stdout: &'a str,
        expected_stderr: &'a str,
    }

    let cases = [
        Case {
            name: "armv8l is treated as aarch64",
            os: "Linux",
            arch: "armv8l",
            expected_status: 0,
            expected_stdout: "linux/aarch64",
            expected_stderr: "",
        },
        Case {
            name: "armv6l remains unsupported",
            os: "Linux",
            arch: "armv6l",
            expected_status: 2,
            expected_stdout: "",
            expected_stderr: "unsupported arch: armv6l",
        },
        Case {
            name: "cygwin remains unsupported",
            os: "CYGWIN_NT-10.0-22621",
            arch: "x86_64",
            expected_status: 2,
            expected_stdout: "",
            expected_stderr: "unsupported OS: CYGWIN_NT-10.0-22621",
        },
    ];

    for case in cases {
        let probe = format!(
            r#"uname() {{
  case "$1" in
    -m) printf '%s\n' '{arch}' ;;
    -s) printf '%s\n' '{os}' ;;
    *) return 1 ;;
  esac
}}
{prefix}
printf '%s/%s' "$os_name" "$arch_name"
"#,
            arch = case.arch,
            os = case.os,
            prefix = prefix,
        );

        let output = Command::new(bash)
            .arg("-c")
            .arg(&probe)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("failed to spawn bash");

        assert_eq!(
            output.status.code(),
            Some(case.expected_status),
            "{}: unexpected status; stderr={}",
            case.name,
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            case.expected_stdout,
            "{}: unexpected stdout",
            case.name,
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            case.expected_stderr,
            "{}: unexpected stderr",
            case.name,
        );
    }
}
