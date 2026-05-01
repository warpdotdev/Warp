// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Mutex;

use doppler::{detect, DopplerError};

// Mutating PATH is process-global. Serialize the two tests so they don't
// race with each other.
static PATH_LOCK: Mutex<()> = Mutex::new(());

/// Helper: temporarily replace `PATH` for the duration of a closure.
fn with_path<F: FnOnce()>(new_path: &std::ffi::OsStr, f: F) {
    let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old = std::env::var_os("PATH");
    std::env::set_var("PATH", new_path);
    f();
    match old {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
}

#[test]
fn detect_returns_not_installed_with_empty_path() {
    with_path(std::ffi::OsStr::new(""), || {
        let result = detect();
        match result {
            Err(DopplerError::NotInstalled { install_hint }) => {
                assert!(!install_hint.is_empty(), "install hint must not be empty");
            }
            other => panic!("expected NotInstalled, got {other:?}"),
        }
    });
}

#[test]
fn detect_finds_binary_in_tempdir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin_dir = tmp.path();

    // Put a fake `doppler` binary in the tempdir.
    #[cfg(unix)]
    let bin_path = {
        use std::os::unix::fs::PermissionsExt;
        let path = bin_dir.join("doppler");
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    };
    #[cfg(windows)]
    let bin_path = {
        let path = bin_dir.join("doppler.exe");
        std::fs::write(&path, b"MZ").unwrap();
        path
    };

    with_path(bin_dir.as_os_str(), || {
        let found = detect().expect("detect should succeed");
        // Canonicalise both sides because PATH lookup may resolve symlinks
        // (e.g. `/var` -> `/private/var` on macOS).
        let found_canon = std::fs::canonicalize(&found).unwrap_or(found);
        let expected_canon = std::fs::canonicalize(&bin_path).unwrap_or(bin_path);
        assert_eq!(found_canon, expected_canon);
    });
}
