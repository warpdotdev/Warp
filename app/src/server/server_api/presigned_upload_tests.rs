use std::collections::HashMap;
use std::fs;

use futures::executor::block_on;
use mockito::Server;
use tempfile::tempdir;

use super::*;
use crate::server::server_api::ai::{FileArtifactUploadHeaderInfo, FileArtifactUploadTargetInfo};

#[test]
fn encode_crc32c_base64_matches_spec_example() {
    assert_eq!(encode_crc32c_base64(0x1234_5678), "EjRWeA==");
}

#[test]
fn shared_checksum_state_finalize_returns_error_when_called_twice() {
    let checksum = SharedChecksumState::new();
    checksum.update(b"artifact payload");

    let finalized = checksum.finalize().unwrap();
    let err = checksum.finalize().unwrap_err();

    assert_eq!(
        finalized,
        encode_crc32c_base64(CRC32C.checksum(b"artifact payload"))
    );
    assert!(err.to_string().contains("checksum already finalized"));
}

#[test]
fn upload_to_target_replays_headers_for_byte_uploads() {
    block_on(async {
        let mut server = Server::new();
        let mock = server
            .mock("POST", "/upload")
            .match_header("x-test-header", "expected-header")
            .match_body("serialized body")
            .with_status(200)
            .create();

        let client = http_client::Client::new_for_test();
        let target = UploadTarget {
            url: format!("{}/upload", server.url()),
            method: "POST".to_string(),
            headers: HashMap::from([("x-test-header".to_string(), "expected-header".to_string())]),
        };

        upload_to_target(&client, &target, "serialized body".to_string())
            .await
            .unwrap();

        mock.assert();
    });
}

#[test]
fn upload_file_to_target_replays_headers_sets_content_length_and_returns_checksum() {
    block_on(async {
        let tempdir = tempdir().unwrap();
        let path = tempdir.path().join("artifact.bin");
        let body = b"artifact payload";
        let content_length = body.len().to_string();
        fs::write(&path, body).unwrap();

        let mut server = Server::new();
        let mock = server
            .mock("POST", "/upload")
            .match_header("x-test-header", "expected-header")
            .match_header("content-length", content_length.as_str())
            .match_body(body.to_vec())
            .with_status(200)
            .create();

        let client = http_client::Client::new_for_test();
        let target = FileArtifactUploadTargetInfo {
            url: format!("{}/upload", server.url()),
            method: "POST".to_string(),
            headers: vec![FileArtifactUploadHeaderInfo {
                name: "x-test-header".to_string(),
                value: "expected-header".to_string(),
            }],
        };

        let checksum = upload_file_to_target(&client, &target, &path, body.len() as u64)
            .await
            .unwrap();

        mock.assert();
        assert_eq!(checksum, encode_crc32c_base64(CRC32C.checksum(body)));
    });
}

#[test]
fn upload_file_to_target_returns_status_and_body_for_failed_uploads() {
    block_on(async {
        let tempdir = tempdir().unwrap();
        let path = tempdir.path().join("artifact.bin");
        fs::write(&path, b"artifact payload").unwrap();

        let mut server = Server::new();
        let mock = server
            .mock("PUT", "/upload")
            .with_status(403)
            .with_body("denied")
            .create();

        let client = http_client::Client::new_for_test();
        let target = FileArtifactUploadTargetInfo {
            url: format!("{}/upload", server.url()),
            method: "PUT".to_string(),
            headers: Vec::new(),
        };

        let err =
            upload_file_to_target(&client, &target, &path, fs::metadata(&path).unwrap().len())
                .await
                .unwrap_err();

        mock.assert();
        assert!(err
            .to_string()
            .contains("Artifact upload failed with status 403 Forbidden: denied"));
    });
}
