#[cfg(not(target_family = "wasm"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_family = "wasm"))]
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
#[cfg(not(target_family = "wasm"))]
use async_stream::try_stream;
#[cfg(not(target_family = "wasm"))]
use base64::{engine::general_purpose::STANDARD, Engine as _};
#[cfg(not(target_family = "wasm"))]
use bytes::Bytes;
#[cfg(not(target_family = "wasm"))]
use crc::{Crc, CRC_32_ISCSI};
#[cfg(not(target_family = "wasm"))]
use futures_lite::io::AsyncReadExt as _;
use thiserror::Error;

#[cfg(not(target_family = "wasm"))]
use super::ai::FileArtifactUploadTargetInfo;
use super::harness_support::UploadTarget;

/// Typed error for HTTP-backed operations so downstream classifiers (e.g. the agent-SDK
/// retry helper) can decide transient vs permanent failures without string-parsing the
/// anyhow Display.
///
/// Emitted as the source cause of an upload failure; callers typically also attach a
/// human-facing context message via `.context(...)` so `err.to_string()` remains useful.
#[derive(Debug, Error)]
#[error("HTTP request failed with status {status}: {body}")]
pub struct HttpStatusError {
    pub status: u16,
    pub body: String,
}

#[cfg(not(target_family = "wasm"))]
static CRC32C: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
const CONTENT_LENGTH_HEADER_NAME: &str = "content-length";
#[cfg(not(target_family = "wasm"))]
const FILE_UPLOAD_CHUNK_SIZE: usize = 64 * 1024;

struct NormalizedUploadTarget<'a> {
    url: &'a str,
    method: &'a str,
    headers: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&'a UploadTarget> for NormalizedUploadTarget<'a> {
    fn from(target: &'a UploadTarget) -> Self {
        Self {
            url: &target.url,
            method: &target.method,
            headers: target
                .headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect(),
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl<'a> From<&'a FileArtifactUploadTargetInfo> for NormalizedUploadTarget<'a> {
    fn from(target: &'a FileArtifactUploadTargetInfo) -> Self {
        Self {
            url: &target.url,
            method: &target.method,
            headers: target
                .headers
                .iter()
                .map(|header| (header.name.as_str(), header.value.as_str()))
                .collect(),
        }
    }
}

#[cfg(not(target_family = "wasm"))]
#[derive(Clone)]
struct SharedChecksumState(Arc<Mutex<Option<crc::Digest<'static, u32>>>>);

#[derive(Copy, Clone)]
struct UploadErrorContext {
    transport: &'static str,
    failure: &'static str,
}

#[cfg(not(target_family = "wasm"))]
impl SharedChecksumState {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Some(CRC32C.digest()))))
    }

    fn update(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        let mut digest = self.0.lock().expect("checksum state mutex poisoned");
        digest
            .as_mut()
            .expect("checksum already finalized")
            .update(bytes);
    }

    fn finalize(&self) -> Result<String> {
        let digest = self
            .0
            .lock()
            .map_err(|_| anyhow!("checksum state mutex poisoned"))?
            .take()
            .ok_or_else(|| anyhow!("checksum already finalized"))?;
        Ok(encode_crc32c_base64(digest.finalize()))
    }
}

fn build_upload_request<'a>(
    http_client: &'a http_client::Client,
    target: NormalizedUploadTarget<'_>,
    content_length: Option<u64>,
) -> Result<http_client::RequestBuilder<'a>> {
    let method = target.method.to_ascii_uppercase();
    let has_content_length = target
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(CONTENT_LENGTH_HEADER_NAME));

    let mut request = match method.as_str() {
        "GET" => http_client.get(target.url),
        "POST" => http_client.post(target.url),
        "PUT" => http_client.put(target.url),
        "DELETE" => http_client.delete(target.url),
        other => return Err(anyhow!("Unsupported HTTP method: {other}")),
    };

    for (name, value) in target.headers {
        request = request.header(name, value);
    }

    if let Some(content_length) = content_length.filter(|_| !has_content_length) {
        request = request.header(CONTENT_LENGTH_HEADER_NAME, content_length.to_string());
    }

    Ok(request)
}

async fn ensure_upload_succeeded(
    response: http_client::Response,
    error_context: UploadErrorContext,
) -> Result<()> {
    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let status_err = HttpStatusError {
        status: status.as_u16(),
        body: body.clone(),
    };
    Err(anyhow::Error::new(status_err).context(format!(
        "{} failed with status {status}: {body}",
        error_context.failure
    )))
}

async fn send_upload_request(
    http_client: &http_client::Client,
    target: NormalizedUploadTarget<'_>,
    body: impl Into<reqwest::Body>,
    content_length: Option<u64>,
    error_context: UploadErrorContext,
) -> Result<()> {
    let response = build_upload_request(http_client, target, content_length)?
        .body(body)
        .send()
        .await
        .context(error_context.transport)?;

    ensure_upload_succeeded(response, error_context).await
}

pub(crate) async fn upload_to_target(
    http_client: &http_client::Client,
    target: &UploadTarget,
    body: impl Into<reqwest::Body>,
) -> Result<()> {
    send_upload_request(
        http_client,
        target.into(),
        body,
        None,
        UploadErrorContext {
            transport: "Failed to upload to presigned URL",
            failure: "Upload",
        },
    )
    .await
}

#[cfg(not(target_family = "wasm"))]
fn encode_crc32c_base64(crc32c: u32) -> String {
    // Storage providers expect the checksum as base64 of the raw big-endian CRC32C bytes,
    // not the more human-readable hex string we typically log.
    STANDARD.encode(crc32c.to_be_bytes())
}

#[cfg(not(target_family = "wasm"))]
fn file_upload_stream(
    mut file: async_fs::File,
    path: PathBuf,
    checksum: SharedChecksumState,
) -> impl futures::Stream<Item = std::io::Result<Bytes>> + Send + 'static {
    try_stream! {
        loop {
            let mut chunk = vec![0; FILE_UPLOAD_CHUNK_SIZE];
            let bytes_read = file.read(&mut chunk).await.map_err(|err| {
                std::io::Error::other(format!(
                    "Failed to read artifact file '{}': {err}",
                    path.display()
                ))
            })?;

            if bytes_read == 0 {
                break;
            }

            chunk.truncate(bytes_read);
            checksum.update(&chunk);
            yield Bytes::from(chunk);
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) async fn upload_file_to_target(
    http_client: &http_client::Client,
    target: &FileArtifactUploadTargetInfo,
    path: &Path,
    file_size: u64,
) -> Result<String> {
    let file = async_fs::File::open(path)
        .await
        .with_context(|| format!("Failed to open artifact file '{}'", path.display()))?;
    let checksum = SharedChecksumState::new();
    let body = reqwest::Body::wrap_stream(file_upload_stream(
        file,
        path.to_path_buf(),
        checksum.clone(),
    ));

    send_upload_request(
        http_client,
        target.into(),
        body,
        Some(file_size),
        UploadErrorContext {
            transport: "Failed to upload artifact bytes",
            failure: "Artifact upload",
        },
    )
    .await?;

    checksum.finalize()
}

#[cfg(test)]
#[path = "presigned_upload_tests.rs"]
mod tests;
