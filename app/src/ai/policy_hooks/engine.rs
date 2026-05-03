use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use command::{r#async::Command, Stdio};
use futures_lite::io::AsyncWriteExt;
use reqwest::header::CONTENT_TYPE;
use warpui::r#async::FutureExt as _;

use super::{
    audit::write_audit_record,
    config::{AgentPolicyHook, AgentPolicyHookConfig, AgentPolicyHookTransport},
    decision::{
        compose_policy_decisions, AgentPolicyDecisionKind, AgentPolicyEffectiveDecision,
        AgentPolicyHookErrorKind, AgentPolicyHookEvaluation, AgentPolicyHookResponse,
        WarpPermissionSnapshot,
    },
    event::{AgentPolicyEvent, AGENT_POLICY_SCHEMA_VERSION},
    redaction::truncate_for_policy,
};

const MAX_HOOK_STDOUT_BYTES: usize = 64 * 1024;

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
            Ok(response) => AgentPolicyHookEvaluation::from_response(hook.name.clone(), response),
            Err(failure) => AgentPolicyHookEvaluation::unavailable(
                hook.name.clone(),
                self.config.hook_unavailable_decision(hook).decision_kind(),
                failure.kind,
                failure.detail,
            ),
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
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(working_directory) = working_directory {
            command.current_dir(working_directory);
        }

        for (key, value) in env {
            command.env(key, value.as_str());
        }

        let mut child = command.spawn().map_err(|source| AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::SpawnFailed,
            detail: format!("failed to spawn policy hook: {source}"),
        })?;

        let event_bytes = serialize_event(event).map_err(|source| AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::MalformedResponse,
            detail: format!("failed to serialize policy event: {source}"),
        })?;

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

        let timeout = Duration::from_millis(self.config.hook_timeout_ms(hook));
        let output = child
            .output()
            .with_timeout(timeout)
            .await
            .map_err(|_| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::Timeout,
                detail: format!("policy hook timed out after {timeout:?}"),
            })?
            .map_err(|source| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::SpawnFailed,
                detail: format!("failed to wait for policy hook: {source}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::NonZeroExit,
                detail: format!(
                    "policy hook exited with {}; stderr={}",
                    output.status,
                    truncate_for_policy(stderr.trim())
                ),
            });
        }

        if output.stdout.len() > MAX_HOOK_STDOUT_BYTES {
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::MalformedResponse,
                detail: format!("policy hook stdout exceeded {MAX_HOOK_STDOUT_BYTES} bytes"),
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

        let event_bytes = serialize_event(event).map_err(|source| AgentPolicyHookFailure {
            kind: AgentPolicyHookErrorKind::MalformedResponse,
            detail: format!("failed to serialize policy event: {source}"),
        })?;

        let client = reqwest::Client::new();
        let mut request = client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header("x-warp-agent-policy-event-id", event.event_id.to_string())
            .body(event_bytes);
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let timeout = Duration::from_millis(self.config.hook_timeout_ms(hook));
        let response = request
            .send()
            .with_timeout(timeout)
            .await
            .map_err(|_| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::Timeout,
                detail: format!("policy hook timed out after {timeout:?}"),
            })?
            .map_err(|source| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::HttpRequestFailed,
                detail: format!("failed to call HTTP policy hook: {source}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::HttpStatus,
                detail: format!("HTTP policy hook returned status {status}"),
            });
        }

        let response_bytes = response
            .bytes()
            .with_timeout(timeout)
            .await
            .map_err(|_| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::Timeout,
                detail: format!("policy hook response timed out after {timeout:?}"),
            })?
            .map_err(|source| AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::HttpRequestFailed,
                detail: format!("failed to read HTTP policy hook response: {source}"),
            })?;

        if response_bytes.len() > MAX_HOOK_STDOUT_BYTES {
            return Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::MalformedResponse,
                detail: format!("policy hook response exceeded {MAX_HOOK_STDOUT_BYTES} bytes"),
            });
        }

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

fn serialize_event(event: &AgentPolicyEvent) -> Result<Vec<u8>> {
    serde_json::to_vec(event).context("serialize policy event")
}

fn parse_hook_response(stdout: &[u8]) -> Result<AgentPolicyHookResponse> {
    let response: AgentPolicyHookResponse =
        serde_json::from_slice(stdout).context("parse JSON response")?;

    if response.schema_version != AGENT_POLICY_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported schema_version {:?}",
            response.schema_version
        ));
    }

    if response.decision == AgentPolicyDecisionKind::Unknown {
        return Err(anyhow!("unknown policy hook decision"));
    }

    Ok(response)
}
