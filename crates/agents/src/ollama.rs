//! [`Agent`] implementation backed by the local [Ollama](https://ollama.com)
//! CLI.
//!
//! The agent shells out to a locally installed `ollama` binary and pipes the
//! task prompt to `ollama run <model>` over stdin, streaming the model's
//! stdout as [`AgentEvent::OutputChunk`] events. It also probes the model's
//! capabilities at construction time via `ollama show <model>` so the router
//! can size context windows and gate tool-using tasks.
//!
//! No direct provider SDK calls happen here — `ollama` itself is the local
//! inference driver and we are merely a process supervisor.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use chrono::Utc;
use orchestrator::{
    Agent, AgentError, AgentEvent, AgentEventStream, AgentId, Capabilities, Health, Role, Task,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

/// Default Ollama HTTP endpoint (used for diagnostics; the agent itself talks
/// to `ollama` over the CLI rather than the HTTP API).
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";

/// Fallback context window when `ollama show` does not expose one for the
/// requested model. Conservative — most current Ollama models ship with at
/// least an 8K window.
pub const DEFAULT_CONTEXT_TOKENS: u32 = 8_192;

/// An [`Agent`] that drives a local `ollama` CLI binary.
///
/// Construction probes `ollama show <model>` for tool-use support and the
/// model's context window length. If the probe fails (model not pulled,
/// `ollama` daemon down, JSON schema drift) the agent still constructs but
/// falls back to [`DEFAULT_CONTEXT_TOKENS`] and `supports_tools = false`.
pub struct OllamaAgent {
    id: AgentId,
    binary: PathBuf,
    model: String,
    base_url: String,
    capabilities: Capabilities,
    health: Arc<Mutex<Health>>,
}

impl OllamaAgent {
    /// Construct a new [`OllamaAgent`] for `model` (e.g. `"qwen2.5-coder:32b"`).
    ///
    /// Resolves the `ollama` binary on `PATH` via [`which::which`]; returns
    /// [`AgentError::Other`] with an install hint if it isn't found. After
    /// resolving the binary, runs `ollama show <model>` to detect the model's
    /// tool-use support and context window length. Probe failures are
    /// non-fatal: the constructor returns successfully with safe defaults.
    pub fn new(id: AgentId, model: String) -> Result<Self, AgentError> {
        let binary = which::which("ollama").map_err(|_| {
            AgentError::Other(
                "ollama CLI not on PATH; install from https://ollama.com/download".to_string(),
            )
        })?;
        let probe = probe_model(&binary, &model);
        let capabilities = build_capabilities(probe);
        let health = Arc::new(Mutex::new(Health {
            healthy: true,
            last_check: Utc::now(),
            error_rate: 0.0,
        }));
        Ok(Self {
            id,
            binary,
            model,
            base_url: DEFAULT_OLLAMA_BASE_URL.to_string(),
            capabilities,
            health,
        })
    }

    /// Path to the resolved `ollama` binary. Exposed for diagnostics / tests.
    pub fn binary_path(&self) -> &PathBuf {
        &self.binary
    }

    /// Model name this agent is configured to drive.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// HTTP base URL associated with this agent (informational only — the
    /// agent itself talks to `ollama` over the CLI).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Result of probing `ollama show` for a model.
#[derive(Debug, Clone, Default)]
struct ModelProbe {
    /// Detected context window in tokens, if `ollama show` reported one.
    context_tokens: Option<u32>,
    /// Whether the model card advertises tool-use support.
    supports_tools: bool,
    /// Whether the model card advertises vision / multimodal support.
    supports_vision: bool,
}

/// Run `ollama show <model>` and parse what we can. Failures are swallowed —
/// the caller receives a default [`ModelProbe`] so the agent still constructs.
///
/// `ollama show` output is human-formatted text; we scan for the patterns we
/// care about rather than relying on a stable JSON schema (the `--json` /
/// `--modelfile` flags are not consistent across Ollama releases). The
/// detector is intentionally conservative: anything we can't confidently
/// parse degrades to "not supported".
fn probe_model(binary: &std::path::Path, model: &str) -> ModelProbe {
    let output = match std::process::Command::new(binary)
        .arg("show")
        .arg(model)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            tracing::warn!(
                model,
                status = ?o.status,
                "ollama show exited non-zero; falling back to safe capability defaults"
            );
            return ModelProbe::default();
        }
        Err(err) => {
            tracing::warn!(
                model,
                ?err,
                "failed to invoke ollama show; falling back to safe capability defaults"
            );
            return ModelProbe::default();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lower = stdout.to_lowercase();

    // Tool-use detection. `ollama show` lists model capabilities (e.g.
    // `Capabilities`/`tools`/`vision`) for newer model cards. Older cards
    // omit the section entirely; we treat that as "no tools".
    let supports_tools = lower.contains("tools");
    let supports_vision = lower.contains("vision");

    // Context-window detection. Look for either `context length` (newer
    // formatted output) or `num_ctx` (parameter dump from older versions).
    let context_tokens = extract_context_length(&stdout);

    ModelProbe {
        context_tokens,
        supports_tools,
        supports_vision,
    }
}

/// Scan `ollama show` output for the model's context length. Tries a couple
/// of textual patterns to stay robust across Ollama CLI versions.
fn extract_context_length(text: &str) -> Option<u32> {
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.contains("context length") || lower.contains("num_ctx") {
            // Pull the first integer-like token out of the line.
            for token in line.split(|c: char| !c.is_ascii_digit()) {
                if token.is_empty() {
                    continue;
                }
                if let Ok(n) = token.parse::<u32>() {
                    if n >= 1024 {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}

fn build_capabilities(probe: ModelProbe) -> Capabilities {
    use std::collections::HashSet;
    let roles: HashSet<Role> = [
        Role::Worker,
        Role::BulkRefactor,
        Role::Summarize,
        Role::Inline,
    ]
    .into_iter()
    .collect();
    Capabilities {
        roles,
        max_context_tokens: probe.context_tokens.unwrap_or(DEFAULT_CONTEXT_TOKENS),
        supports_tools: probe.supports_tools,
        supports_vision: probe.supports_vision,
    }
}

#[async_trait]
impl Agent for OllamaAgent {
    fn id(&self) -> AgentId {
        self.id.clone()
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn health(&self) -> Health {
        if let Ok(guard) = self.health.try_lock() {
            guard.clone()
        } else {
            Health {
                healthy: true,
                last_check: Utc::now(),
                error_rate: 0.0,
            }
        }
    }

    async fn execute(&self, task: Task) -> Result<AgentEventStream, AgentError> {
        let task_id = task.id;

        // Hard gate on roles Ollama is not appropriate for.
        if matches!(task.role, Role::Planner | Role::Reviewer) {
            let stream = stream! {
                yield AgentEvent::Started { task_id };
                yield AgentEvent::Failed {
                    task_id,
                    error: "Ollama is not configured for planning/review roles".to_string(),
                };
            };
            return Ok(Box::pin(stream));
        }

        // Soft gate on tool use: warn but proceed, per spec.
        let role_implies_tools = matches!(task.role, Role::ToolRouter);
        if role_implies_tools && !self.capabilities.supports_tools {
            tracing::warn!(
                model = %self.model,
                "task implies tool use but ollama model does not advertise tool support; \
                 proceeding without tools (output may not handle tool requests gracefully)"
            );
        }

        let mut cmd = Command::new(&self.binary);
        cmd.arg("run").arg(&self.model);

        if !task.context.cwd.as_os_str().is_empty() {
            cmd.current_dir(&task.context.cwd);
        }
        for (k, v) in &task.context.env {
            cmd.env(k, v);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|err| {
            AgentError::Other(format!(
                "failed to spawn `{}`: {err}",
                self.binary.display()
            ))
        })?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Other("ollama child missing stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Other("ollama child missing stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::Other("ollama child missing stderr".to_string()))?;

        let prompt = task.prompt.clone();
        let health = Arc::clone(&self.health);

        let stream = stream! {
            yield AgentEvent::Started { task_id };

            // Pipe prompt to the model in a background task so we can read
            // stdout concurrently without deadlocking on a full pipe.
            let stdin_handle = tokio::spawn(async move {
                let _ = stdin.write_all(prompt.as_bytes()).await;
                let _ = stdin.write_all(b"\n").await;
                let _ = stdin.shutdown().await;
            });

            let stderr_handle = tokio::spawn(async move {
                let mut buf = String::new();
                let mut reader = BufReader::new(stderr);
                let _ = reader.read_to_string(&mut buf).await;
                buf
            });

            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        // `ollama run` prints model output as plain text;
                        // re-attach the trailing newline so consumers can
                        // reassemble the response losslessly.
                        let mut text = line;
                        text.push('\n');
                        yield AgentEvent::OutputChunk { text };
                    }
                    Ok(None) => break,
                    Err(err) => {
                        tracing::warn!(?err, "ollama stdout read error");
                        break;
                    }
                }
            }

            let _ = stdin_handle.await;
            let stderr_text = stderr_handle.await.unwrap_or_default();
            match child.wait().await {
                Ok(status) if status.success() => {
                    yield AgentEvent::Completed { task_id, summary: None };
                }
                Ok(status) => {
                    let mut h = health.lock().await;
                    h.healthy = false;
                    h.last_check = Utc::now();
                    h.error_rate = (h.error_rate + 1.0).min(1.0);
                    drop(h);
                    let error = if stderr_text.trim().is_empty() {
                        format!("ollama exited with status {status}")
                    } else {
                        format!("ollama exited with status {status}: {}", stderr_text.trim())
                    };
                    yield AgentEvent::Failed { task_id, error };
                }
                Err(err) => {
                    let mut h = health.lock().await;
                    h.healthy = false;
                    h.last_check = Utc::now();
                    h.error_rate = (h.error_rate + 1.0).min(1.0);
                    drop(h);
                    yield AgentEvent::Failed {
                        task_id,
                        error: format!("waiting on ollama failed: {err}"),
                    };
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_context_length_handles_context_length_line() {
        let txt = "Model\n  context length    32768\n  embedding length 5120\n";
        assert_eq!(extract_context_length(txt), Some(32_768));
    }

    #[test]
    fn extract_context_length_handles_num_ctx_parameter() {
        let txt = "Parameters\n  num_ctx                        16384\n";
        assert_eq!(extract_context_length(txt), Some(16_384));
    }

    #[test]
    fn extract_context_length_returns_none_when_absent() {
        assert_eq!(extract_context_length("nothing of interest here"), None);
    }

    #[test]
    fn build_capabilities_uses_default_when_probe_empty() {
        let caps = build_capabilities(ModelProbe::default());
        assert_eq!(caps.max_context_tokens, DEFAULT_CONTEXT_TOKENS);
        assert!(!caps.supports_tools);
        assert!(!caps.supports_vision);
        assert!(caps.roles.contains(&Role::Worker));
        assert!(caps.roles.contains(&Role::BulkRefactor));
        assert!(caps.roles.contains(&Role::Summarize));
        assert!(caps.roles.contains(&Role::Inline));
        assert!(!caps.roles.contains(&Role::Planner));
        assert!(!caps.roles.contains(&Role::Reviewer));
    }

    #[test]
    fn build_capabilities_respects_probe_overrides() {
        let caps = build_capabilities(ModelProbe {
            context_tokens: Some(131_072),
            supports_tools: true,
            supports_vision: true,
        });
        assert_eq!(caps.max_context_tokens, 131_072);
        assert!(caps.supports_tools);
        assert!(caps.supports_vision);
    }
}
