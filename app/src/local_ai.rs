//! Local-AI bypass plumbing. Reads `.env`/process env to route agent runs to a
//! local provider (Ollama / Claude CLI / Codex CLI) and to skip Warp's online
//! auth gate. All state is read once at startup and cached.
//!
//! # Local model IDs
//!
//! When local-AI is active the model selector shows a custom list of models
//! instead of Warp's server-fetched list.  Each entry uses a structured ID
//! that encodes the provider and its parameters:
//!
//! | ID string                                   | CLI invoked                                   |
//! |---------------------------------------------|-----------------------------------------------|
//! | `local:claude:claude-sonnet-4-7`             | `claude -p --model claude-sonnet-4-7 ...`     |
//! | `local:claude:claude-opus-4-7`               | `claude -p --model claude-opus-4-7 ...`       |
//! | `local:claude:claude-haiku-4-5`              | `claude -p --model claude-haiku-4-5 ...`      |
//! | `local:codex:gpt-5.5:low`                   | `codex exec -m gpt-5.5 -c reasoning_effort=low ...`  |
//! | `local:codex:gpt-5.5:medium`                | `codex exec -m gpt-5.5 -c reasoning_effort=medium ...` |
//! | `local:codex:gpt-5.5:high`                  | `codex exec -m gpt-5.5 -c reasoning_effort=high ...`  |
//!
//! The `WARP_LOCAL_AI` env var overrides which provider is used (for
//! backwards compatibility) but does NOT restrict which models appear in the
//! menu.  The menu selection is the primary control when `WARP_BYPASS_AUTH=1`
//! is set without `WARP_LOCAL_AI`.

use std::collections::HashMap;
use std::sync::OnceLock;

// Re-export only what callers need; the full model definitions live here.
use crate::ai::llms::{
    AvailableLLMs, LLMId, LLMInfo, LLMProvider, LLMSpec, LLMUsageMetadata, ModelsByFeature,
};

// ---------------------------------------------------------------------------
// LocalAiMode – legacy env-var override
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalAiMode {
    Off,
    Ollama,
    Claude,
    Codex,
}

impl LocalAiMode {
    fn from_env() -> Self {
        match std::env::var("WARP_LOCAL_AI")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("ollama") => LocalAiMode::Ollama,
            Some("claude") | Some("claude-code") => LocalAiMode::Claude,
            Some("codex") => LocalAiMode::Codex,
            _ => LocalAiMode::Off,
        }
    }
}

pub fn current() -> LocalAiMode {
    static CACHE: OnceLock<LocalAiMode> = OnceLock::new();
    *CACHE.get_or_init(LocalAiMode::from_env)
}

pub fn is_active() -> bool {
    current() != LocalAiMode::Off
}

/// Returns true if the full auth bypass (no server, no credentials) is active.
/// This is also true when `WARP_BYPASS_AUTH=1` without `WARP_LOCAL_AI`, so
/// the model selector and harness both activate the local path.
pub fn auth_bypass_enabled() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        std::env::var("WARP_BYPASS_AUTH")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
            // Local-AI mode implies auth bypass: the server-mediated path requires auth.
            || LocalAiMode::from_env() != LocalAiMode::Off
    })
}

/// Returns true when local-model routing should be used.  This is true when
/// either `WARP_LOCAL_AI` is set OR `WARP_BYPASS_AUTH=1` is set (in bypass
/// mode the server is unavailable, so we always want local models).
pub fn local_model_routing_active() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| auth_bypass_enabled())
}

// ---------------------------------------------------------------------------
// Local model ID helpers
// ---------------------------------------------------------------------------

/// Well-known local model ID prefix.
pub const LOCAL_MODEL_PREFIX: &str = "local:";

/// Returns true if `id` is a local-model ID managed by this module.
#[allow(dead_code)]
pub fn is_local_model_id(id: &str) -> bool {
    id.starts_with(LOCAL_MODEL_PREFIX)
}

/// Parsed representation of a local model ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalModelSpec {
    /// `claude -p --model <model_name>`
    Claude { model_name: String },
    /// `codex exec -m <model_name> -c reasoning_effort=<effort>`
    Codex {
        model_name: String,
        reasoning_effort: Option<String>,
    },
}

impl LocalModelSpec {
    /// Parse a local model ID string (e.g. `"local:claude:claude-sonnet-4-7"`).
    /// Returns `None` for unrecognised or non-local IDs.
    pub fn parse(id: &str) -> Option<Self> {
        let rest = id.strip_prefix(LOCAL_MODEL_PREFIX)?;
        let mut parts = rest.splitn(3, ':');
        let provider = parts.next()?;
        let model_name = parts.next()?.to_string();

        match provider {
            "claude" => Some(LocalModelSpec::Claude { model_name }),
            "codex" => {
                let reasoning_effort = parts.next().map(str::to_string);
                Some(LocalModelSpec::Codex {
                    model_name,
                    reasoning_effort,
                })
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Local model list
// ---------------------------------------------------------------------------

fn make_claude_model(
    display_name: &str,
    model_name: &str,
    spec: Option<LLMSpec>,
) -> LLMInfo {
    LLMInfo {
        display_name: display_name.to_string(),
        base_model_name: display_name.to_string(),
        id: format!("local:claude:{model_name}").into(),
        reasoning_level: None,
        usage_metadata: LLMUsageMetadata {
            request_multiplier: 1,
            credit_multiplier: None,
        },
        description: None,
        disable_reason: None,
        vision_supported: true,
        spec,
        provider: LLMProvider::Anthropic,
        host_configs: HashMap::new(),
        discount_percentage: None,
    }
}

fn make_codex_model(
    display_name: &str,
    model_name: &str,
    reasoning_effort: Option<&str>,
    spec: Option<LLMSpec>,
) -> LLMInfo {
    let id = match reasoning_effort {
        Some(effort) => format!("local:codex:{model_name}:{effort}"),
        None => format!("local:codex:{model_name}"),
    };
    LLMInfo {
        display_name: display_name.to_string(),
        base_model_name: display_name.to_string(),
        id: id.into(),
        reasoning_level: reasoning_effort.map(str::to_string),
        usage_metadata: LLMUsageMetadata {
            request_multiplier: 1,
            credit_multiplier: None,
        },
        description: None,
        disable_reason: None,
        vision_supported: false,
        spec,
        provider: LLMProvider::OpenAI,
        host_configs: HashMap::new(),
        discount_percentage: None,
    }
}

/// Build the `ModelsByFeature` for the local-model menu.
///
/// The default selection is Claude Sonnet 4.7 (good balance of quality and speed
/// for users who already have `claude` CLI authenticated).
///
/// The `WARP_LOCAL_AI` env var does NOT restrict which models appear; it only
/// provides a backwards-compatible default provider choice.  Specifically:
/// - If `WARP_LOCAL_AI=codex`, the default selection becomes GPT-5.5 (medium).
/// - Otherwise, Claude Sonnet 4.7 is the default.
pub fn local_model_list() -> ModelsByFeature {
    let claude_sonnet = make_claude_model(
        "Claude Sonnet 4.7",
        "claude-sonnet-4-7",
        Some(LLMSpec { cost: 0.5, quality: 0.85, speed: 0.8 }),
    );
    let claude_opus = make_claude_model(
        "Claude Opus 4.7",
        "claude-opus-4-7",
        Some(LLMSpec { cost: 0.9, quality: 1.0, speed: 0.5 }),
    );
    let claude_haiku = make_claude_model(
        "Claude Haiku 4.5",
        "claude-haiku-4-5",
        Some(LLMSpec { cost: 0.2, quality: 0.6, speed: 1.0 }),
    );
    let gpt55_low = make_codex_model(
        "GPT-5.5 (low)",
        "gpt-5.5",
        Some("low"),
        Some(LLMSpec { cost: 0.3, quality: 0.65, speed: 0.9 }),
    );
    let gpt55_medium = make_codex_model(
        "GPT-5.5 (medium)",
        "gpt-5.5",
        Some("medium"),
        Some(LLMSpec { cost: 0.55, quality: 0.80, speed: 0.7 }),
    );
    let gpt55_high = make_codex_model(
        "GPT-5.5 (high)",
        "gpt-5.5",
        Some("high"),
        Some(LLMSpec { cost: 0.85, quality: 0.95, speed: 0.45 }),
    );

    let all_choices = vec![
        claude_sonnet.clone(),
        claude_opus,
        claude_haiku,
        gpt55_low.clone(),
        gpt55_medium.clone(),
        gpt55_high,
    ];

    // The default ID depends on the legacy WARP_LOCAL_AI env var so that
    // existing configurations keep working without re-selecting a model.
    let default_id: LLMId = match current() {
        LocalAiMode::Codex => gpt55_medium.id.clone(),
        _ => claude_sonnet.id.clone(),
    };

    let available = AvailableLLMs::new(default_id, all_choices, None)
        .expect("local model list is non-empty and default is present");

    ModelsByFeature {
        agent_mode: available.clone(),
        coding: available.clone(),
        cli_agent: Some(available.clone()),
        computer_use: None,
    }
}
