//! Local-AI bypass plumbing. Reads `.env`/process env to route agent runs to a
//! local provider (Ollama / Claude CLI / Codex CLI) and to skip Warp's online
//! auth gate. All state is read once at startup and cached.

use std::sync::OnceLock;

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
