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
//! | ID string                                   | Invocation                                                  |
//! |---------------------------------------------|-------------------------------------------------------------|
//! | `local:claude:sonnet`                        | `claude -p --model sonnet ...`                              |
//! | `local:claude:opus`                          | `claude -p --model opus ...`                                |
//! | `local:claude:haiku`                         | `claude -p --model haiku ...`                               |
//! | `local:codex:gpt-5.5:low`                   | `codex exec -m gpt-5.5 -c reasoning_effort=low ...`         |
//! | `local:codex:gpt-5.5:medium`                | `codex exec -m gpt-5.5 -c reasoning_effort=medium ...`      |
//! | `local:codex:gpt-5.5:high`                  | `codex exec -m gpt-5.5 -c reasoning_effort=high ...`        |
//! | `local:ollama:qwen2.5-coder:7b`              | HTTP POST `$OLLAMA_HOST/api/chat` model=qwen2.5-coder:7b    |
//! | `local:ollama:llama3.3:70b`                  | HTTP POST `$OLLAMA_HOST/api/chat` model=llama3.3:70b        |
//! | `local:ollama:custom`                        | HTTP POST `$OLLAMA_HOST/api/chat` model=`$OLLAMA_MODEL`     |
//!
//! Ollama uses HTTP, not a CLI subprocess.  The `POST /api/chat` endpoint with
//! `"stream": true` returns newline-delimited JSON; each line is a chunk like
//! `{"message":{"content":"..."},"done":false}`.  The final line has `"done":true`.
//!
//! The `WARP_LOCAL_AI` env var overrides which provider is used (for
//! backwards compatibility) but does NOT restrict which models appear in the
//! menu.  The menu selection is the primary control when `WARP_BYPASS_AUTH=1`
//! is set without `WARP_LOCAL_AI`.
//!
//! # Ollama model discovery
//!
//! At startup (via `LLMPreferences::new`) a background task fetches
//! `GET $OLLAMA_HOST/api/tags` and caches the installed model list in a
//! process-wide `RwLock`.  `local_model_list()` reads from that cache so the
//! model selector shows real models rather than static placeholders.  If Ollama
//! is offline or the fetch fails, the selector falls back to the two hard-coded
//! placeholder entries.  The cache is refreshed on each menu open, with a
//! minimum 30-second gap between network calls.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

// Re-export only what callers need; the full model definitions live here.
use crate::ai::llms::{
    AvailableLLMs, LLMId, LLMInfo, LLMProvider, LLMSpec, LLMUsageMetadata, ModelsByFeature,
};

/// Minimum interval between Ollama `/api/tags` fetches.
const OLLAMA_CACHE_TTL: Duration = Duration::from_secs(30);

/// Process-wide cache of installed Ollama model names.
///
/// `None` means no successful fetch has completed yet (initial state or all
/// fetches have failed).  `Some(vec)` holds the last known list.
struct OllamaModelCache {
    models: Option<Vec<String>>,
    last_fetch: Option<Instant>,
}

static OLLAMA_MODELS: OnceLock<RwLock<OllamaModelCache>> = OnceLock::new();

fn ollama_model_cache() -> &'static RwLock<OllamaModelCache> {
    OLLAMA_MODELS.get_or_init(|| {
        RwLock::new(OllamaModelCache {
            models: None,
            last_fetch: None,
        })
    })
}

/// Returns the cached Ollama model names, or `None` if the cache is empty.
pub fn cached_ollama_models() -> Option<Vec<String>> {
    ollama_model_cache()
        .read()
        .ok()
        .and_then(|g| g.models.clone())
}

/// Returns `true` if the cache is old enough that a fresh fetch should be attempted.
pub fn ollama_cache_needs_refresh() -> bool {
    ollama_model_cache()
        .read()
        .ok()
        .map(|g| {
            g.last_fetch
                .map(|t| t.elapsed() >= OLLAMA_CACHE_TTL)
                .unwrap_or(true)
        })
        .unwrap_or(true)
}

/// Fetch installed model names from the Ollama `/api/tags` endpoint.
///
/// Returns a list of model name strings (e.g. `["qwen3:32b", "llama3.3:70b"]`),
/// or an error if Ollama is unreachable, returns an unexpected HTTP status, or
/// returns unparseable JSON.
pub async fn fetch_ollama_models() -> Result<Vec<String>, String> {
    let host = ollama_host();
    let url = format!("{host}/api/tags");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(3000))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GET {url} failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "GET {url} returned HTTP {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse JSON from {url}: {e}"))?;

    let models = body
        .get("models")
        .and_then(|m| m.as_array())
        .ok_or_else(|| format!("unexpected JSON shape from {url}: missing 'models' array"))?;

    let names: Vec<String> = models
        .iter()
        .filter_map(|m| m.get("name")?.as_str().map(str::to_string))
        .collect();

    Ok(names)
}

/// Store a successfully-fetched list of models in the process-wide cache.
pub fn update_ollama_model_cache(models: Vec<String>) {
    if let Ok(mut guard) = ollama_model_cache().write() {
        guard.models = Some(models);
        guard.last_fetch = Some(Instant::now());
    }
}

// ---------------------------------------------------------------------------
// LocalAiMode -- legacy env-var override
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
// Ollama env-var helpers
// ---------------------------------------------------------------------------

/// Default Ollama base URL used when `OLLAMA_HOST` is not set.
pub const OLLAMA_DEFAULT_HOST: &str = "http://localhost:11434";

/// Returns the Ollama base URL, reading `OLLAMA_HOST` from the environment.
pub fn ollama_host() -> String {
    std::env::var("OLLAMA_HOST")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| OLLAMA_DEFAULT_HOST.to_string())
}

/// Returns the model name to use for the "custom" Ollama entry, reading
/// `OLLAMA_MODEL` from the environment (defaults to `"llama3.2"`).
pub fn ollama_custom_model() -> String {
    std::env::var("OLLAMA_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "llama3.2".to_string())
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
    /// HTTP POST to `$OLLAMA_HOST/api/chat` with the given model name.
    /// When `model_name` is `"custom"`, the actual model is read from `$OLLAMA_MODEL`.
    Ollama { model_name: String },
}

impl LocalModelSpec {
    /// Parse a local model ID string (e.g. `"local:claude:claude-sonnet-4-7"`).
    /// Returns `None` for unrecognised or non-local IDs.
    pub fn parse(id: &str) -> Option<Self> {
        let rest = id.strip_prefix(LOCAL_MODEL_PREFIX)?;
        // Ollama model names contain colons (e.g. "qwen2.5-coder:7b"), so we
        // only split off the provider prefix and keep the rest as the model name.
        let colon = rest.find(':')?;
        let provider = &rest[..colon];
        let after_provider = &rest[colon + 1..];

        match provider {
            "claude" => {
                // Normalise legacy versioned IDs (stored before this change) to
                // the canonical aliases so the CLI call succeeds regardless of
                // which claude version is installed.
                let model_name = match after_provider {
                    "claude-sonnet-4-7" | "claude-sonnet-4-6" | "claude-sonnet-4-5" => "sonnet",
                    "claude-opus-4-7" | "claude-opus-4-6" | "claude-opus-4-5" => "opus",
                    "claude-haiku-4-5" | "claude-haiku-4-6" => "haiku",
                    other => other,
                };
                Some(LocalModelSpec::Claude {
                    model_name: model_name.to_string(),
                })
            }
            "codex" => {
                // Codex IDs: `codex:<model>` or `codex:<model>:<effort>`
                // Model names don't contain colons, so splitting on ':' is safe here.
                let mut parts = after_provider.splitn(2, ':');
                let model_name = parts.next()?.to_string();
                let reasoning_effort = parts.next().map(str::to_string);
                Some(LocalModelSpec::Codex {
                    model_name,
                    reasoning_effort,
                })
            }
            "ollama" => Some(LocalModelSpec::Ollama {
                model_name: after_provider.to_string(),
            }),
            _ => None,
        }
    }

    /// Resolve the effective Ollama model name, expanding `"custom"` to
    /// the `OLLAMA_MODEL` env var value.
    pub fn ollama_resolved_model(model_name: &str) -> String {
        if model_name == "custom" {
            ollama_custom_model()
        } else {
            model_name.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Local model list
// ---------------------------------------------------------------------------

fn make_claude_model(display_name: &str, model_name: &str, spec: Option<LLMSpec>) -> LLMInfo {
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

/// Build an Ollama model entry.
///
/// `ollama_model` is the Ollama model tag (e.g. `"qwen2.5-coder:7b"`).
/// Pass `"custom"` to produce the entry that reads `$OLLAMA_MODEL` at
/// request time; the display name will include the current env value.
fn make_ollama_model(display_name: &str, ollama_model: &str, spec: Option<LLMSpec>) -> LLMInfo {
    LLMInfo {
        display_name: display_name.to_string(),
        base_model_name: display_name.to_string(),
        // The Ollama model tag may contain a colon (e.g. "qwen2.5-coder:7b").
        // The ID encodes it directly; the parser handles it by keeping
        // everything after the second colon as the model name.
        id: format!("local:ollama:{ollama_model}").into(),
        reasoning_level: None,
        usage_metadata: LLMUsageMetadata {
            request_multiplier: 1,
            credit_multiplier: None,
        },
        description: Some(ollama_host()),
        disable_reason: None,
        vision_supported: false,
        spec,
        provider: LLMProvider::Unknown,
        host_configs: HashMap::new(),
        discount_percentage: None,
    }
}

/// Build the `ModelsByFeature` for the local-model menu.
///
/// The default selection is Claude Sonnet (latest alias, always resolves to the
/// installed version) for users who already have `claude` CLI authenticated.
///
/// The `WARP_LOCAL_AI` env var does NOT restrict which models appear; it only
/// provides a backwards-compatible default provider choice.  Specifically:
/// - If `WARP_LOCAL_AI=codex`, the default selection becomes GPT-5.5 (medium).
/// - If `WARP_LOCAL_AI=ollama`, the default selection becomes Ollama (custom).
/// - Otherwise, Claude Sonnet 4.7 is the default.
pub fn local_model_list() -> ModelsByFeature {
    let claude_sonnet = make_claude_model(
        "Claude / Sonnet (latest)",
        "sonnet",
        Some(LLMSpec { cost: 0.5, quality: 0.85, speed: 0.8 }),
    );
    let claude_opus = make_claude_model(
        "Claude / Opus (latest)",
        "opus",
        Some(LLMSpec { cost: 0.9, quality: 1.0, speed: 0.5 }),
    );
    let claude_haiku = make_claude_model(
        "Claude / Haiku (latest)",
        "haiku",
        Some(LLMSpec { cost: 0.2, quality: 0.6, speed: 1.0 }),
    );
    let gpt55_low = make_codex_model(
        "Codex / GPT-5.5 (low)",
        "gpt-5.5",
        Some("low"),
        Some(LLMSpec { cost: 0.3, quality: 0.65, speed: 0.9 }),
    );
    let gpt55_medium = make_codex_model(
        "Codex / GPT-5.5 (medium)",
        "gpt-5.5",
        Some("medium"),
        Some(LLMSpec { cost: 0.55, quality: 0.80, speed: 0.7 }),
    );
    let gpt55_high = make_codex_model(
        "Codex / GPT-5.5 (high)",
        "gpt-5.5",
        Some("high"),
        Some(LLMSpec { cost: 0.85, quality: 0.95, speed: 0.45 }),
    );
    // Ollama entries: built from the live cache when available, falling back
    // to a pair of popular static entries so the menu is never empty.
    //
    // The "custom (env)" entry always appears last so power-users who set
    // OLLAMA_MODEL can always reach their model regardless of cache state.
    let discovered: Vec<LLMInfo> = if let Some(names) = cached_ollama_models() {
        names
            .into_iter()
            .map(|name| {
                make_ollama_model(
                    &format!("Ollama / {name}"),
                    &name,
                    Some(LLMSpec { cost: 0.0, quality: 0.5, speed: 0.75 }),
                )
            })
            .collect()
    } else {
        // Fallback while the background fetch is in flight (or Ollama is offline).
        vec![
            make_ollama_model(
                "Ollama / qwen2.5-coder:7b",
                "qwen2.5-coder:7b",
                Some(LLMSpec { cost: 0.0, quality: 0.7, speed: 0.85 }),
            ),
            make_ollama_model(
                "Ollama / llama3.3:70b",
                "llama3.3:70b",
                Some(LLMSpec { cost: 0.0, quality: 0.8, speed: 0.5 }),
            ),
        ]
    };

    // "custom" entry: display name shows current OLLAMA_MODEL value so the
    // user can see which model will be used without opening the env file.
    let custom_model = ollama_custom_model();
    let ollama_custom = make_ollama_model(
        &format!("Ollama / {custom_model} (env)"),
        "custom",
        Some(LLMSpec { cost: 0.0, quality: 0.5, speed: 0.75 }),
    );

    let mut all_choices = vec![
        claude_sonnet.clone(),
        claude_opus,
        claude_haiku,
        gpt55_low.clone(),
        gpt55_medium.clone(),
        gpt55_high,
    ];
    all_choices.extend(discovered);
    all_choices.push(ollama_custom.clone());

    // The default ID depends on the legacy WARP_LOCAL_AI env var so that
    // existing configurations keep working without re-selecting a model.
    let default_id: LLMId = match current() {
        LocalAiMode::Codex => gpt55_medium.id.clone(),
        LocalAiMode::Ollama => ollama_custom.id.clone(),
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

// ---------------------------------------------------------------------------
// jCodeMunch MCP integration
// ---------------------------------------------------------------------------

/// Returns `true` when jCodeMunch auto-registration is suppressed.
///
/// Set `JCODEMUNCH_DISABLED=1` in `.env` to opt out.
pub fn jcodemunch_disabled() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        std::env::var("JCODEMUNCH_DISABLED")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
    })
}

/// Path to the Warp home-level MCP config file (channel-aware).
///
/// On macOS with the OSS channel this is `~/.warp-oss/.mcp.json`.
/// Returns `None` if the home directory cannot be determined.
fn warp_home_mcp_config_path() -> Option<std::path::PathBuf> {
    warp_core::paths::warp_home_mcp_config_file_path()
}

/// Ensures a jCodeMunch entry exists in the Warp home MCP config when bypass
/// is active.
///
/// The function is idempotent: if an entry named `"jcodemunch"` already exists
/// it is not modified.  If `jcodemunch-mcp` is not on `$PATH` (and the user has
/// not installed it via `pip install jcodemunch-mcp`) a single info-level
/// message is logged and the function returns normally - Warp continues without
/// the codebase index.
///
/// Set `JCODEMUNCH_DISABLED=1` to suppress this entirely.
pub fn ensure_jcodemunch_mcp_entry() {
    if !auth_bypass_enabled() || jcodemunch_disabled() {
        return;
    }

    // Check that jcodemunch-mcp is on PATH.  We look for the binary that
    // `pip install jcodemunch-mcp` drops (name: `jcodemunch-mcp`) as well as
    // `uvx jcodemunch-mcp` (which is runtime-resolved and always available if
    // uvx is installed).  We prefer the direct binary to avoid a uvx overhead.
    let has_binary = std::process::Command::new("which")
        .arg("jcodemunch-mcp")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_uvx = !has_binary
        && std::process::Command::new("which")
            .arg("uvx")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if !has_binary && !has_uvx {
        log::info!(
            "jCodeMunch: jcodemunch-mcp not found on PATH and uvx not available - \
             local codebase index unavailable. \
             Install with: pip install jcodemunch-mcp   OR   brew install uv"
        );
        return;
    }

    let Some(mcp_config_path) = warp_home_mcp_config_path() else {
        return;
    };

    // Read or initialise the config file.
    let existing = if mcp_config_path.exists() {
        std::fs::read_to_string(&mcp_config_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Parse as a JSON object; fall back to an empty object on parse error.
    let mut config: serde_json::Value = if existing.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&existing).unwrap_or(serde_json::json!({}))
    };

    // If "jcodemunch" is already registered, nothing to do.
    if config
        .get("mcpServers")
        .and_then(|s| s.get("jcodemunch"))
        .is_some()
    {
        return;
    }

    // Build the server entry.
    let entry = if has_binary {
        serde_json::json!({
            "command": "jcodemunch-mcp",
            "args": []
        })
    } else {
        // uvx path: no install required, runs via uv tool runner.
        serde_json::json!({
            "command": "uvx",
            "args": ["jcodemunch-mcp"]
        })
    };

    config
        .as_object_mut()
        .and_then(|obj| {
            obj.entry("mcpServers")
                .or_insert(serde_json::json!({}))
                .as_object_mut()
                .map(|servers| servers.insert("jcodemunch".to_owned(), entry))
        });

    // Ensure the parent directory exists.
    if let Some(parent) = mcp_config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match serde_json::to_string_pretty(&config) {
        Ok(serialized) => {
            if let Err(e) = std::fs::write(&mcp_config_path, serialized) {
                log::warn!("jCodeMunch: failed to write MCP config: {e}");
            } else {
                let invocation = if has_binary { "jcodemunch-mcp" } else { "uvx jcodemunch-mcp" };
                log::info!(
                    "jCodeMunch: registered '{invocation}' in {} - \
                     local codebase index available via MCP",
                    mcp_config_path.display()
                );
            }
        }
        Err(e) => log::warn!("jCodeMunch: failed to serialize MCP config: {e}"),
    }
}
