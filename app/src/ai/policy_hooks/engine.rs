use std::{collections::BTreeMap, io, process::ExitStatus, time::Duration};

use anyhow::{anyhow, Context, Result};
use command::{r#async::Command, Stdio};
use futures::StreamExt as _;
use futures_lite::{
    future,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
};
use reqwest::header::CONTENT_TYPE;
use warpui::r#async::FutureExt as _;

use super::{
    audit::write_audit_record,
    config::{
        AgentPolicyHook, AgentPolicyHookConfig, AgentPolicyHookSecretValue,
        AgentPolicyHookTransport,
    },
    decision::{
        compose_policy_decisions, AgentPolicyDecisionKind, AgentPolicyEffectiveDecision,
        AgentPolicyHookErrorKind, AgentPolicyHookEvaluation, AgentPolicyHookResponse,
        WarpPermissionSnapshot,
    },
    event::{AgentPolicyEvent, AGENT_POLICY_SCHEMA_VERSION},
    redaction::redact_sensitive_text_for_policy,
};

const MAX_HOOK_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_HOOK_EVENT_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct AgentPolicyHookEngine {
    config: AgentPolicyHookConfig,
}

impl AgentPolicyHookEngine {
    pub(crate) fn new(config: AgentPolicyHookConfig) -> Self {
        Self { config }
    }

    pub(crate) async fn preflight(
        &self,
        mut event: AgentPolicyEvent,
        warp_permission: WarpPermissionSnapshot,
    ) -> AgentPolicyEffectiveDecision {
        event.warp_permission = warp_permission.clone();

        if !self.config.is_active() {
            return compose_policy_decisions(warp_permission, Vec::new(), false);
        }

        if let Err(err) = self.config.validate() {
            let decision = compose_policy_decisions(
                warp_permission,
                vec![AgentPolicyHookEvaluation::unavailable(
                    "agent_policy_hooks",
                    self.config.on_unavailable.decision_kind(),
                    AgentPolicyHookErrorKind::InvalidConfiguration,
                    format!("agent policy hook configuration is invalid: {err}"),
                )],
                false,
            );
            audit_decision(&event, &decision);
            return decision;
        }

        let mut hook_results = Vec::new();
        for hook in &self.config.before_action {
            let result = self.evaluate_hook(hook, &event).await;
            let denied = result.decision == AgentPolicyDecisionKind::Deny;
            hook_results.push(result);

            if denied {
                break;
            }
        }

        let decision = compose_policy_decisions(
            warp_permission,
            hook_results,
            self.config.allow_autoapproval_for_all_hooks(),
        );
        audit_decision(&event, &decision);
        decision
    }

    async fn evaluate_hook(
        &self,
        hook: &AgentPolicyHook,
        event: &AgentPolicyEvent,
    ) -> AgentPolicyHookEvaluation {
        let response = match &hook.transport {
            AgentPolicyHookTransport::Stdio { .. } => self.run_stdio_hook(hook, event).await,
            AgentPolicyHookTransport::Http { .. } => self.run_http_hook(hook, event).await,
        };

        match response {
            Ok(response) => AgentPolicyHookEvaluation::from_response(
                hook.name.clone(),
                redact_hook_response_configured_secrets(response, hook),
            ),
            Err(failure) => {
                let failure = redact_hook_failure_configured_secrets(failure, hook);
                AgentPolicyHookEvaluation::unavailable(
                    hook.name.clone(),
                    self.config.hook_unavailable_decision(hook).decision_kind(),
                    failure.kind,
                    failure.detail,
                )
            }
        }
    }

    async fn run_stdio_hook(
        &self,
        hook: &AgentPolicyHook,
        event: &AgentPolicyEvent,
    ) -> Result<AgentPolicyHookResponse, AgentPolicyHookFailure> {
        let AgentPolicyHookTransport::Stdio {
            command,
            args,
            env,
            working_directory,
        } = &hook.transport
        else {
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::UnsupportedTransport,
                detail: "hook transport is not stdio".to_string(),
            });
        };

        let mut command = Command::new(command);
        command
            .args(args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(working_directory) = working_directory {
            command.current_dir(working_directory);
        }

        for (key, value) in env {
            command.env(key, resolve_hook_secret_value(value)?);
        }

        let event_bytes = serialize_event(event)?;

        let mut child = command.spawn().map_err(|source| AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::SpawnFailed,
            detail: format!("failed to spawn policy hook: {source}"),
        })?;

        let timeout = Duration::from_millis(self.config.hook_timeout_ms(hook));
        let output = match async {
            let Some(mut stdin) = child.stdin.take() else {
                return Err(AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::StdinWriteFailed,
                    detail: "policy hook stdin was not available".to_string(),
                });
            };

            stdin
                .write_all(&event_bytes)
                .await
                .map_err(|source| AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::StdinWriteFailed,
                    detail: format!("failed to write policy event to hook stdin: {source}"),
                })?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|source| AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::StdinWriteFailed,
                    detail: format!("failed to terminate policy event on hook stdin: {source}"),
                })?;
            drop(stdin);

            let stdout = child.stdout.take().ok_or_else(|| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::SpawnFailed,
                detail: "policy hook stdout was not available".to_string(),
            })?;
            let stderr = child.stderr.take().ok_or_else(|| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::SpawnFailed,
                detail: "policy hook stderr was not available".to_string(),
            })?;

            let (stdout, stderr) = future::try_zip(
                read_capped_output(stdout, "stdout"),
                read_capped_output(stderr, "stderr"),
            )
            .await?;
            let status = child
                .status()
                .await
                .map_err(|source| AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::SpawnFailed,
                    detail: format!("failed to wait for policy hook: {source}"),
                })?;

            Ok::<_, AgentPolicyHookFailure>(HookProcessOutput {
                status,
                stdout,
                stderr,
            })
        }
        .with_timeout(timeout)
        .await
        {
            Err(_) => {
                let _ = child.kill();
                return Err(AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::Timeout,
                    detail: format!("policy hook timed out after {timeout:?}"),
                });
            }
            Ok(Err(failure)) => {
                let _ = child.kill();
                return Err(failure);
            }
            Ok(Ok(output)) => output,
        };

        if !output.status.success() {
            let stderr = redact_hook_stderr(&output.stderr, env);
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::NonZeroExit,
                detail: format!(
                    "policy hook exited with {}; stderr={}",
                    output.status, stderr,
                ),
            });
        }

        let response =
            parse_hook_response(&output.stdout).map_err(|source| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::MalformedResponse,
                detail: format!("policy hook returned malformed response: {source:#}"),
            })?;

        Ok(response)
    }

    async fn run_http_hook(
        &self,
        hook: &AgentPolicyHook,
        event: &AgentPolicyEvent,
    ) -> Result<AgentPolicyHookResponse, AgentPolicyHookFailure> {
        let AgentPolicyHookTransport::Http { url, headers } = &hook.transport else {
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::UnsupportedTransport,
                detail: "hook transport is not HTTP".to_string(),
            });
        };

        let event_bytes = serialize_event(event)?;

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|source| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::HttpRequestFailed,
                detail: format!(
                    "failed to build HTTP policy hook client: {}",
                    source.without_url()
                ),
            })?;
        let mut request = client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header("x-warp-agent-policy-event-id", event.event_id.to_string())
            .body(event_bytes);
        for (key, value) in headers {
            request = request.header(key.as_str(), resolve_hook_secret_value(value)?);
        }

        let timeout = Duration::from_millis(self.config.hook_timeout_ms(hook));
        let response_bytes = match async {
            let response = request
                .send()
                .await
                .map_err(|source| AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::HttpRequestFailed,
                    detail: format!("failed to call HTTP policy hook: {}", source.without_url()),
                })?;

            let status = response.status();
            if !status.is_success() {
                return Err(AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::HttpStatus,
                    detail: format!("HTTP policy hook returned status {status}"),
                });
            }

            read_capped_http_response(response).await
        }
        .with_timeout(timeout)
        .await
        {
            Err(_) => {
                return Err(AgentPolicyHookFailure {
                    kind: AgentPolicyHookErrorKind::Timeout,
                    detail: format!("policy hook timed out after {timeout:?}"),
                });
            }
            Ok(result) => result?,
        };

        parse_hook_response(&response_bytes).map_err(|source| AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::MalformedResponse,
            detail: format!("policy hook returned malformed response: {source:#}"),
        })
    }
}

fn audit_decision(event: &AgentPolicyEvent, decision: &AgentPolicyEffectiveDecision) {
    if let Err(err) = write_audit_record(event, decision) {
        log::warn!("Failed to write agent policy hook audit record: {err:#}");
    }
}

#[derive(Debug, Clone)]
struct AgentPolicyHookFailure {
    kind: AgentPolicyHookErrorKind,
    detail: String,
}

#[derive(Debug)]
struct HookProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn read_capped_output<R>(
    mut reader: R,
    stream_name: &'static str,
) -> Result<Vec<u8>, AgentPolicyHookFailure>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut chunk = [0_u8; 8192];

    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|source| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::SpawnFailed,
                detail: format!("failed to read policy hook {stream_name}: {source}"),
            })?;
        if read == 0 {
            break;
        }

        if output.len().saturating_add(read) > MAX_HOOK_OUTPUT_BYTES {
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::MalformedResponse,
                detail: format!("policy hook {stream_name} exceeded {MAX_HOOK_OUTPUT_BYTES} bytes"),
            });
        }

        output.extend_from_slice(&chunk[..read]);
    }

    Ok(output)
}

async fn read_capped_http_response(
    response: reqwest::Response,
) -> Result<Vec<u8>, AgentPolicyHookFailure> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_HOOK_OUTPUT_BYTES as u64)
    {
        return Err(AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::MalformedResponse,
            detail: format!("policy hook response exceeded {MAX_HOOK_OUTPUT_BYTES} bytes"),
        });
    }

    let mut output = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::HttpRequestFailed,
            detail: format!(
                "failed to read HTTP policy hook response: {}",
                source.without_url()
            ),
        })?;

        if output.len().saturating_add(chunk.len()) > MAX_HOOK_OUTPUT_BYTES {
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::MalformedResponse,
                detail: format!("policy hook response exceeded {MAX_HOOK_OUTPUT_BYTES} bytes"),
            });
        }

        output.extend_from_slice(&chunk);
    }

    Ok(output)
}

fn redact_hook_stderr(stderr: &[u8], env: &BTreeMap<String, AgentPolicyHookSecretValue>) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let redacted = redact_configured_secret_values(stderr.trim(), env.values());
    redact_sensitive_text_for_policy(&redacted)
}

fn redact_hook_response_configured_secrets(
    response: AgentPolicyHookResponse,
    hook: &AgentPolicyHook,
) -> AgentPolicyHookResponse {
    match &hook.transport {
        AgentPolicyHookTransport::Stdio { env, .. } => {
            redact_hook_response_secret_values(response, env.values())
        }
        AgentPolicyHookTransport::Http { headers, .. } => {
            redact_hook_response_secret_values(response, headers.values())
        }
    }
}

fn redact_hook_failure_configured_secrets(
    failure: AgentPolicyHookFailure,
    hook: &AgentPolicyHook,
) -> AgentPolicyHookFailure {
    let detail = match &hook.transport {
        AgentPolicyHookTransport::Stdio { env, .. } => {
            redact_configured_secret_values(&failure.detail, env.values())
        }
        AgentPolicyHookTransport::Http { headers, .. } => {
            redact_configured_secret_values(&failure.detail, headers.values())
        }
    };

    AgentPolicyHookFailure { detail, ..failure }
}

fn redact_hook_response_secret_values<'a>(
    response: AgentPolicyHookResponse,
    secrets: impl IntoIterator<Item = &'a AgentPolicyHookSecretValue> + Clone,
) -> AgentPolicyHookResponse {
    AgentPolicyHookResponse {
        schema_version: response.schema_version,
        decision: response.decision,
        reason: response
            .reason
            .map(|reason| redact_configured_secret_values(&reason, secrets.clone())),
        external_audit_id: response
            .external_audit_id
            .map(|audit_id| redact_configured_secret_values(&audit_id, secrets)),
    }
}

fn redact_configured_secret_values<'a>(
    value: &str,
    secrets: impl IntoIterator<Item = &'a AgentPolicyHookSecretValue>,
) -> String {
    let mut redacted = value.to_string();
    for value in secrets {
        let Ok(secret) = value.resolved_value() else {
            continue;
        };
        if !secret.is_empty() {
            redacted = redacted.replace(&secret, "<redacted>");
        }
        if let Some((scheme, credential)) = secret.split_once(' ') {
            if (scheme.eq_ignore_ascii_case("bearer") || scheme.eq_ignore_ascii_case("basic"))
                && credential.len() >= 4
            {
                redacted = redacted.replace(credential, "<redacted>");
            }
        }
    }
    redacted
}

fn resolve_hook_secret_value(
    value: &AgentPolicyHookSecretValue,
) -> Result<String, AgentPolicyHookFailure> {
    value
        .resolved_value()
        .map_err(|env| AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::InvalidConfiguration,
            detail: format!("policy hook secret environment variable {env:?} is not set"),
        })
}

struct CappedEventWriter {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl CappedEventWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            exceeded: false,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl io::Write for CappedEventWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(buf.len()) > MAX_HOOK_EVENT_BYTES {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("policy event exceeded {MAX_HOOK_EVENT_BYTES} bytes"),
            ));
        }

        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialize_event(event: &AgentPolicyEvent) -> Result<Vec<u8>, AgentPolicyHookFailure> {
    let mut writer = CappedEventWriter::new();
    if let Err(source) = serde_json::to_writer(&mut writer, event).context("serialize policy event")
    {
        let kind = if writer.exceeded {
            AgentPolicyHookErrorKind::PayloadTooLarge
        } else {
            AgentPolicyHookErrorKind::MalformedResponse
        };
        return Err(AgentPolicyHookFailure {
            kind,
            detail: format!("failed to serialize policy event: {source}"),
        });
    }

    Ok(writer.into_inner())
}

fn parse_hook_response(stdout: &[u8]) -> Result<AgentPolicyHookResponse> {
    let response: AgentPolicyHookResponse =
        serde_json::from_slice(stdout).context("parse JSON response")?;

    if response.schema_version != AGENT_POLICY_SCHEMA_VERSION {
        return Err(anyhow!("unsupported schema_version"));
    }

    if response.decision == AgentPolicyDecisionKind::Unknown {
        return Err(anyhow!("unknown policy hook decision"));
    }

    Ok(response)
}
