use std::path::Path;

use anyhow::anyhow;
use warp_core::report_error;
use warp_multi_agent_api as api;

use crate::notebooks::NotebookId;

pub mod buttons;
pub use buttons::{ArtifactButtonsRow, ArtifactButtonsRowEvent};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "artifact_type", content = "data")]
pub enum Artifact {
    #[serde(rename = "PLAN")]
    Plan {
        document_uid: String,
        /// None until the plan is synced to Warp Drive.
        notebook_uid: Option<NotebookId>,
        title: Option<String>,
    },
    #[serde(rename = "PULL_REQUEST")]
    PullRequest {
        url: String,
        branch: String,
        #[serde(skip_serializing)] // We derive this field from the url on deserialize
        repo: Option<String>,
        #[serde(skip_serializing)] // We derive this field from the url on deserialize
        number: Option<u32>,
    },
    #[serde(rename = "SCREENSHOT")]
    Screenshot {
        artifact_uid: String,
        mime_type: String,
        description: Option<String>,
    },
    #[serde(rename = "FILE")]
    File {
        artifact_uid: String,
        filepath: String,
        filename: String,
        mime_type: String,
        description: Option<String>,
        size_bytes: Option<i32>,
    },
}

#[derive(serde::Deserialize)]
#[serde(tag = "artifact_type", content = "data")]
enum ArtifactHelper {
    #[serde(rename = "PLAN")]
    Plan {
        document_uid: String,
        notebook_uid: Option<NotebookId>,
        title: Option<String>,
    },
    #[serde(rename = "PULL_REQUEST")]
    PullRequest { url: String, branch: String },
    #[serde(rename = "SCREENSHOT")]
    Screenshot {
        artifact_uid: String,
        mime_type: String,
        description: Option<String>,
    },
    #[serde(rename = "FILE")]
    File {
        artifact_uid: String,
        filepath: String,
        filename: String,
        mime_type: String,
        description: Option<String>,
        size_bytes: Option<i32>,
    },
}

impl<'de> serde::Deserialize<'de> for Artifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = ArtifactHelper::deserialize(deserializer)?;
        Ok(match helper {
            ArtifactHelper::Plan {
                document_uid,
                notebook_uid,
                title,
            } => Artifact::Plan {
                document_uid,
                notebook_uid,
                title,
            },
            ArtifactHelper::PullRequest { url, branch } => {
                let (repo, number) = parse_github_pr_url(&url).unzip();
                Artifact::PullRequest {
                    url,
                    branch,
                    repo,
                    number,
                }
            }
            ArtifactHelper::Screenshot {
                artifact_uid,
                mime_type,
                description,
            } => Artifact::Screenshot {
                artifact_uid,
                mime_type,
                description,
            },
            ArtifactHelper::File {
                artifact_uid,
                filepath,
                filename,
                mime_type,
                description,
                size_bytes,
            } => Artifact::File {
                artifact_uid,
                filepath,
                filename,
                mime_type,
                description,
                size_bytes,
            },
        })
    }
}

impl From<api::message::artifact_event::PullRequestArtifact> for Artifact {
    fn from(pr: api::message::artifact_event::PullRequestArtifact) -> Self {
        let (repo, number) = parse_github_pr_url(&pr.url).unzip();
        Artifact::PullRequest {
            url: pr.url,
            branch: pr.branch,
            repo,
            number,
        }
    }
}

impl From<api::message::artifact_event::ScreenshotArtifact> for Artifact {
    fn from(screenshot: api::message::artifact_event::ScreenshotArtifact) -> Self {
        Artifact::Screenshot {
            artifact_uid: screenshot.artifact_uid,
            mime_type: screenshot.mime_type,
            description: if screenshot.description.is_empty() {
                None
            } else {
                Some(screenshot.description)
            },
        }
    }
}

impl From<api::message::artifact_event::FileArtifact> for Artifact {
    fn from(file: api::message::artifact_event::FileArtifact) -> Self {
        Artifact::File {
            artifact_uid: file.artifact_uid,
            filepath: file.filepath.clone(),
            filename: Path::new(&file.filepath)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .filter(|file_name| !file_name.trim().is_empty())
                .unwrap_or("File")
                .to_string(),
            mime_type: file.mime_type,
            description: if file.description.is_empty() {
                None
            } else {
                Some(file.description)
            },
            size_bytes: i32::try_from(file.size_bytes).ok(),
        }
    }
}

impl From<api::message::artifact_event::PlanArtifact> for Artifact {
    fn from(plan: api::message::artifact_event::PlanArtifact) -> Self {
        Artifact::Plan {
            document_uid: plan.document_id,
            notebook_uid: if plan.notebook_uid.is_empty() {
                None
            } else {
                Some(NotebookId::from(plan.notebook_uid))
            },
            title: if plan.title.is_empty() {
                None
            } else {
                Some(plan.title)
            },
        }
    }
}

/// Parse GitHub PR URL to extract repo and number.
/// Expected format: https://github.com/{owner}/{repo}/pull/{number}
pub fn parse_github_pr_url(url: &str) -> Option<(String, u32)> {
    if !url.contains("github.com") {
        return None;
    }
    let segments: Vec<&str> = url.split('/').collect();
    segments.windows(3).find_map(|w| {
        if w[1] != "pull" {
            return None;
        }
        Some((w[0].to_string(), w[2].parse().ok()?))
    })
}

/// Deserialize artifacts, skipping any that fail to parse.
/// This ensures task loading doesn't fail entirely if an artifact has an unknown format.
pub fn deserialize_artifacts<'de, D>(deserializer: D) -> Result<Vec<Artifact>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values: Vec<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| match serde_json::from_value::<Artifact>(value) {
            Ok(artifact) => Some(artifact),
            Err(e) => {
                report_error!(anyhow!("Failed to deserialize artifact, skipping: {}", e));
                None
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
