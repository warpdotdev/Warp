use std::fs;
use std::io::Write as _;

use flate2::write::GzEncoder;
use flate2::Compression;

use super::{extract_tarball_locally, find_extracted_binary};

#[test]
fn finds_unversioned_binary_in_flat_dir() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("oz");
    fs::write(&bin, b"#!/bin/sh\n").unwrap();

    let found = find_extracted_binary(dir.path()).unwrap();
    assert_eq!(found, bin);
}

#[test]
fn finds_versioned_binary_in_flat_dir() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("oz-v0.2026.01.01");
    fs::write(&bin, b"binary").unwrap();

    let found = find_extracted_binary(dir.path()).unwrap();
    assert_eq!(found, bin);
}

#[test]
fn finds_binary_in_nested_dir() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("payload").join("subdir");
    fs::create_dir_all(&nested).unwrap();
    let bin = nested.join("oz-dev");
    fs::write(&bin, b"binary").unwrap();

    let found = find_extracted_binary(dir.path()).unwrap();
    assert_eq!(found, bin);
}

#[test]
fn excludes_tarball_archives() {
    let dir = tempfile::tempdir().unwrap();
    // Archive files that share the `oz` prefix must never be selected as the
    // binary, even when they sort before it.
    fs::write(dir.path().join("oz.tar.gz"), b"archive").unwrap();
    fs::write(dir.path().join("oz.tar"), b"archive").unwrap();
    let bin = dir.path().join("oz-preview");
    fs::write(&bin, b"binary").unwrap();

    let found = find_extracted_binary(dir.path()).unwrap();
    assert_eq!(found, bin);
}

#[test]
fn ignores_unrelated_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("README"), b"docs").unwrap();
    fs::write(dir.path().join("install.sh"), b"shell").unwrap();
    let bin = dir.path().join("oz");
    fs::write(&bin, b"binary").unwrap();

    let found = find_extracted_binary(dir.path()).unwrap();
    assert_eq!(found, bin);
}

#[test]
fn errors_when_no_binary_present() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("oz.tar.gz"), b"archive").unwrap();
    fs::write(dir.path().join("README"), b"docs").unwrap();

    let err = find_extracted_binary(dir.path()).unwrap_err();
    assert!(
        err.to_string().contains("No remote-server binary found"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn extracts_synthetic_tarball() {
    let staging = tempfile::tempdir().unwrap();
    let tarball_path = staging.path().join("oz.tar.gz");

    // Build a minimal gzipped tar archive containing one `oz` binary file.
    let tarball_file = fs::File::create(&tarball_path).unwrap();
    let gz = GzEncoder::new(tarball_file, Compression::default());
    let mut builder = tar::Builder::new(gz);
    let payload = b"#!/bin/sh\necho oz\n";
    let mut header = tar::Header::new_gnu();
    header.set_path("oz").unwrap();
    header.set_size(payload.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    builder.append(&header, &payload[..]).unwrap();
    let gz = builder.into_inner().unwrap();
    let mut finished = gz.finish().unwrap();
    finished.flush().unwrap();
    drop(finished);

    let extraction = extract_tarball_locally(&tarball_path).await.unwrap();
    let binary = find_extracted_binary(extraction.dir.path()).unwrap();
    let contents = fs::read(&binary).unwrap();
    assert_eq!(contents, payload);
}
