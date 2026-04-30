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
fn state_is_unsupported() {
    assert!(RemoteServerSetupState::Unsupported {
        reason: UnsupportedReason::GlibcTooOld {
            detected: GlibcVersion::new(2, 17),
            required: GlibcVersion::new(2, 31),
        }
    }
    .is_unsupported());
    assert!(!RemoteServerSetupState::Ready.is_unsupported());
    assert!(!RemoteServerSetupState::Failed {
        error: "oops".into()
    }
    .is_unsupported());
}

#[test]
fn state_unsupported_is_not_in_progress() {
    assert!(!RemoteServerSetupState::Unsupported {
        reason: UnsupportedReason::NonGlibc {
            name: "musl".into()
        }
    }
    .is_in_progress());
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

#[test]
fn parse_preinstall_unknown_busybox() {
    // No getconf, no parseable ldd output — the script emits status=unknown.
    let stdout = "required_glibc=2.31\n\
                  libc_family=unknown\n\
                  status=unknown\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(result.status, PreinstallStatus::Unknown);
    assert_eq!(result.libc, RemoteLibc::Unknown);
    // Fail open: Unknown is reported as supported.
    assert!(result.is_supported());
}

#[test]
fn parse_preinstall_missing_status_falls_open() {
    // Garbled / partial script output — missing status field.
    let stdout = "libc_family=glibc\nlibc_version=2.35\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(result.status, PreinstallStatus::Unknown);
    assert!(result.is_supported());
}

#[test]
fn parse_preinstall_unknown_keys_are_ignored() {
    // Forward-compatibility: future keys are tolerated without breaking
    // the parser.
    let stdout = "status=supported\n\
                  libc_family=glibc\n\
                  libc_version=2.31\n\
                  required_glibc=2.31\n\
                  future_key=future_value\n\
                  another_one=42\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(result.status, PreinstallStatus::Supported);
}

#[test]
fn parse_preinstall_empty_input_is_unknown() {
    let result = PreinstallCheckResult::parse("");
    assert_eq!(result.status, PreinstallStatus::Unknown);
    assert_eq!(result.libc, RemoteLibc::Unknown);
    assert!(result.is_supported());
}

#[test]
fn parse_preinstall_glibc_with_patch_version() {
    let stdout = "required_glibc=2.31\n\
                  libc_family=glibc\n\
                  libc_version=2.35.1\n\
                  status=supported\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(result.libc, RemoteLibc::Glibc(GlibcVersion::new(2, 35)));
    assert_eq!(result.status, PreinstallStatus::Supported);
}

#[test]
fn parse_preinstall_unsupported_glibc_with_unparseable_version_is_unknown() {
    // Defensive: if the script labelled `glibc_too_old` but the version
    // value can't be parsed, we degrade to Unknown rather than panic or
    // surface a malformed reason.
    let stdout = "status=unsupported\n\
                  reason=glibc_too_old\n\
                  libc_family=glibc\n\
                  libc_version=garbage\n\
                  required_glibc=2.31\n";
    let result = PreinstallCheckResult::parse(stdout);
    assert_eq!(result.status, PreinstallStatus::Unknown);
}

#[test]
fn is_supported_truth_table() {
    // Supported.
    let supported = PreinstallCheckResult {
        status: PreinstallStatus::Supported,
        libc: RemoteLibc::Glibc(GlibcVersion::new(2, 35)),
        raw: String::new(),
    };
    assert!(supported.is_supported());

    // Glibc too old.
    let too_old = PreinstallCheckResult {
        status: PreinstallStatus::Unsupported {
            reason: UnsupportedReason::GlibcTooOld {
                detected: GlibcVersion::new(2, 17),
                required: GlibcVersion::new(2, 31),
            },
        },
        libc: RemoteLibc::Glibc(GlibcVersion::new(2, 17)),
        raw: String::new(),
    };
    assert!(!too_old.is_supported());

    // Non-glibc.
    let musl = PreinstallCheckResult {
        status: PreinstallStatus::Unsupported {
            reason: UnsupportedReason::NonGlibc {
                name: "musl".to_string(),
            },
        },
        libc: RemoteLibc::NonGlibc {
            name: "musl".to_string(),
        },
        raw: String::new(),
    };
    assert!(!musl.is_supported());

    // Unknown — fail open.
    let unknown = PreinstallCheckResult {
        status: PreinstallStatus::Unknown,
        libc: RemoteLibc::Unknown,
        raw: String::new(),
    };
    assert!(unknown.is_supported());
}
