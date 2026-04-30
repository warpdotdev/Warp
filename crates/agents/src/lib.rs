//! Concrete `orchestrator::Agent` implementations.
//!
//! Each module wraps one external CLI/runtime: [`ClaudeCodeAgent`] for the
//! `claude` CLI, `CodexAgent` for `codex`, `OllamaAgent` for local models,
//! etc. Adding a new backend is a one-line `pub mod` declaration here plus a
//! sibling module file.

#![deny(missing_docs)]

pub mod claude_code;
// pub mod codex;   // TODO PDX-45
// pub mod ollama;  // TODO PDX-46

pub use claude_code::{ClaudeCodeAgent, ClaudeModel};
