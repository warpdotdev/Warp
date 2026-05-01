//! Concrete `orchestrator::Agent` implementations.
//!
//! Each module wraps one external CLI/runtime: [`ClaudeCodeAgent`] for the
//! `claude` CLI, `CodexAgent` for `codex`, `OllamaAgent` for local models,
//! etc. Adding a new backend is a one-line `pub mod` declaration here plus a
//! sibling module file.

#![deny(missing_docs)]

pub mod claude_code;
pub mod codex;
pub mod foundation_models;
pub mod ollama;
pub mod remote;

pub use claude_code::{ClaudeCodeAgent, ClaudeModel};
pub use codex::{CodexAgent, ReasoningEffort, ServiceTier};
pub use foundation_models::FoundationModelsAgent;
pub use ollama::OllamaAgent;
pub use remote::RemoteAgent;
