use super::super::proto;
use crate::code_review::diff_state::{FileStatusInfo, GitFileStatus};
use warp_util::standardized_path::StandardizedPath;

// ── FileStatusInfo path validation (TryFrom) ────────────────────

#[test]
fn file_status_info_valid_absolute_path() {
    let proto_info = proto::FileStatusInfo {
        path: "/repo/src/main.rs".into(),
        status: Some(proto::GitFileStatus {
            status: Some(proto::git_file_status::Status::NewFile(
                proto::GitFileStatusNew {},
            )),
        }),
    };

    let info = FileStatusInfo::try_from(&proto_info).unwrap();
    assert_eq!(
        info.path,
        StandardizedPath::try_new("/repo/src/main.rs").unwrap()
    );
    assert_eq!(info.status, GitFileStatus::New);
}

#[test]
fn file_status_info_missing_status_is_error() {
    let proto_info = proto::FileStatusInfo {
        path: "/repo/file.rs".into(),
        status: None,
    };

    assert!(FileStatusInfo::try_from(&proto_info).is_err());
}

#[test]
fn file_status_info_missing_status_variant_is_error() {
    let proto_info = proto::FileStatusInfo {
        path: "/repo/file.rs".into(),
        status: Some(proto::GitFileStatus { status: None }),
    };

    assert!(FileStatusInfo::try_from(&proto_info).is_err());
}

#[test]
fn file_status_info_validates_renamed_old_path() {
    let proto_info = proto::FileStatusInfo {
        path: "/repo/new_name.rs".into(),
        status: Some(proto::GitFileStatus {
            status: Some(proto::git_file_status::Status::Renamed(
                proto::GitFileStatusRenamed {
                    old_path: "relative/old.rs".into(),
                },
            )),
        }),
    };

    // old_path is relative — should fail validation.
    assert!(FileStatusInfo::try_from(&proto_info).is_err());
}
