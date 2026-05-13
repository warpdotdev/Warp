use warp_cli::{
    artifact::{ArtifactCommand, DownloadArtifactArgs, GetArtifactArgs, UploadArtifactArgs},
    CliCommand,
};

use super::command_requires_auth;

#[test]
fn login_does_not_require_auth() {
    assert!(!command_requires_auth(&CliCommand::Login));
}

#[test]
fn artifact_download_does_not_require_auth() {
    assert!(!command_requires_auth(&CliCommand::Artifact(
        ArtifactCommand::Download(DownloadArtifactArgs {
            artifact_uid: "artifact-123".to_string(),
            out: None,
        },)
    )));
}

#[test]
fn artifact_get_does_not_require_auth() {
    assert!(!command_requires_auth(&CliCommand::Artifact(
        ArtifactCommand::Get(GetArtifactArgs {
            artifact_uid: "artifact-123".to_string(),
        },)
    )));
}

#[test]
fn artifact_upload_does_not_require_auth() {
    assert!(!command_requires_auth(&CliCommand::Artifact(
        ArtifactCommand::Upload(UploadArtifactArgs {
            path: "artifact.txt".into(),
            run_id: Some("run-123".to_string()),
            conversation_id: None,
            description: None,
        },)
    )));
}
