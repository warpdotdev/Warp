use chrono::Duration;
use virtual_fs::{Stub, VirtualFS};

use super::*;

#[test]
fn test_clean_up_snapshot_files() {
    VirtualFS::test("test_clean_up_snapshot_files", |dirs, mut sandbox| {
        // Create snapshot directory with test files
        sandbox.mkdir(REPO_SNAPSHOT_SUBDIR_NAME);

        // Create a valid snapshot file
        let test_path = PathBuf::from("/test/path");
        let mut hasher = DefaultHasher::new();
        test_path.hash(&mut hasher);
        let valid_snapshot_name = format!("snapshot_{}", hasher.finish());

        // Create entries in the virtual filesystem
        let mut snapshot_dir_relative_path = PathBuf::new();
        snapshot_dir_relative_path.push(REPO_SNAPSHOT_SUBDIR_NAME);
        sandbox.with_files(vec![
            // Valid snapshot file that matches metadata
            Stub::FileWithContent(
                snapshot_dir_relative_path
                    .join(&valid_snapshot_name)
                    .to_string_lossy()
                    .as_ref(),
                "valid content",
            ),
            // Expired snapshot file
            Stub::FileWithContent(
                snapshot_dir_relative_path
                    .join("snapshot_expired")
                    .to_string_lossy()
                    .as_ref(),
                "expired content",
            ),
            // Non-snapshot file that should be ignored
            Stub::FileWithContent(
                snapshot_dir_relative_path
                    .join("regular_file.txt")
                    .to_string_lossy()
                    .as_ref(),
                "regular content",
            ),
        ]);
        // Subdirectory with the 'snapshot_' prefix that should be ignored
        let invalid_subdir_path = snapshot_dir_relative_path.join("snapshot_prefixed_directory");
        sandbox.mkdir(invalid_subdir_path.to_string_lossy().to_string().as_str());

        let snapshot_dir_absolute_path = dirs.tests().join(REPO_SNAPSHOT_SUBDIR_NAME);

        let valid_snapshot_file = snapshot_dir_absolute_path.join(&valid_snapshot_name);
        let expired_file = snapshot_dir_absolute_path.join("snapshot_expired");
        let regular_file = snapshot_dir_absolute_path.join("regular_file.txt");
        let prefixed_subdir = snapshot_dir_absolute_path.join("snapshot_prefixed_directory");

        assert!(valid_snapshot_file.is_file());
        assert!(expired_file.is_file());
        assert!(regular_file.is_file());
        assert!(prefixed_subdir.is_dir());

        // Set the expired file to be older than shelf life
        let old_time = std::time::SystemTime::now()
            - REPO_SNAPSHOT_SHELF_LIFE_DURATION
            - Duration::days(1).to_std().unwrap();
        filetime::set_file_mtime(
            &expired_file,
            filetime::FileTime::from_system_time(old_time),
        )
        .unwrap();

        // Create test metadata that only includes the valid file
        let metadata = vec![WorkspaceMetadata {
            path: test_path,
            navigated_ts: None,
            modified_ts: None,
            queried_ts: None,
        }];

        // Run cleanup
        clean_up_snapshot_files(&snapshot_dir_absolute_path, &metadata);

        // Valid snapshot should still exist
        assert!(valid_snapshot_file.exists());

        // Expired snapshot should be deleted
        assert!(!expired_file.exists());

        // Regular file should remain untouched
        assert!(regular_file.exists());

        // Subdirectory with 'snapshot_' prefix should remain untouched
        assert!(prefixed_subdir.exists());
    });
}

#[test]
fn test_clean_up_snapshot_files_no_snapshot_dir() {
    VirtualFS::test("test_clean_up_snapshot_files_no_dir", |dirs, _sandbox| {
        // Test with empty metadata when snapshot directory doesn't exist
        let snapshot_metadata = vec![];
        let snapshot_directory = dirs.tests().join(REPO_SNAPSHOT_SUBDIR_NAME);
        clean_up_snapshot_files(&snapshot_directory, &snapshot_metadata);
        assert!(snapshot_metadata.is_empty());
    });
}
