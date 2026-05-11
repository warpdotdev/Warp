use std::path::PathBuf;

use super::super::proto;
use crate::code_review::diff_state::{FileStatusInfo, GitFileStatus};

// ── FileStatusInfo path validation (TryFrom) ────────────────────────

#[test]
fn file_status_info_valid_relative_path() {
    let proto_info = proto::FileStatusInfo {
        path: "src/main.rs".into(),
        status: Some(proto::GitFileStatus {
            status: Some(proto::git_file_status::Status::NewFile(
                proto::GitFileStatusNew {},
            )),
        }),
    };

    let info = FileStatusInfo::try_from(&proto_info).unwrap();
    assert_eq!(info.path, PathBuf::from("src/main.rs"));
    assert_eq!(info.status, GitFileStatus::New);
}

#[test]
fn file_status_info_missing_status_defaults_to_modified() {
    let proto_info = proto::FileStatusInfo {
        path: "file.rs".into(),
        status: None,
    };

    let info = FileStatusInfo::try_from(&proto_info).unwrap();
    assert_eq!(info.status, GitFileStatus::Modified);
}

#[test]
fn file_status_info_rejects_absolute_path() {
    let proto_info = proto::FileStatusInfo {
        path: "/etc/passwd".into(),
        status: None,
    };

    assert!(FileStatusInfo::try_from(&proto_info).is_err());
}

#[test]
fn file_status_info_rejects_path_traversal() {
    let proto_info = proto::FileStatusInfo {
        path: "../outside/secret.txt".into(),
        status: None,
    };

    assert!(FileStatusInfo::try_from(&proto_info).is_err());
}

#[test]
fn file_status_info_rejects_empty_path() {
    let proto_info = proto::FileStatusInfo {
        path: "".into(),
        status: None,
    };

    assert!(FileStatusInfo::try_from(&proto_info).is_err());
}
