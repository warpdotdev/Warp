use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use command::{r#async::Command, Stdio};
use futures_lite::io::AsyncWriteExt;
use warpui::r#async::FutureExt as _;

use super::{
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
            return compose_policy_decisions(
                warp_permission,
                vec![AgentPolicyHookEvaluation::unavailable(
                    "agent_policy_hooks",
                    self.config.on_unavailable.decision_kind(),
                    AgentPolicyHookErrorKind::InvalidConfiguration,
                    format!("agent policy hook configuration is invalid: {err}"),
                )],
                false,
            );
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

        compose_policy_decisions(
            warp_permission,
            hook_results,
            self.config.allow_autoapproval_for_all_hooks(),
        )
    }

    async fn evaluate_hook(
        &self,
        hook: &AgentPolicyHook,
        event: &AgentPolicyEvent,
    ) -> AgentPolicyHookEvaluation {
        let response = match &hook.transport {
            AgentPolicyHookTransport::Stdio { .. } => self.run_stdio_hook(hook, event).await,
            AgentPolicyHookTransport::Http { .. } => Err(AgentPolicyHookFailure {
                kind: AgentPolicyHookErrorKind::UnsupportedTransport,
                detail: "HTTP policy hooks are not implemented in the local engine yet".to_string(),
            }),
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
