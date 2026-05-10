//! Programmatic input channel for Warp Remote Control.
//!
//! `rc_input` exposes a local-socket / named-pipe listener that accepts
//! JSON-formatted input messages and forwards them on a
//! [`tokio::sync::mpsc::Sender`]. The intended consumer is Warp's shared-session
//! input dispatcher; external bridges (Telegram, Slack, scripts) connect on the
//! same machine and write messages newline-delimited.
//!
//! The crate is deliberately decoupled from Warp internals so it can be unit
//! tested in isolation. Wiring into [`Sharer`](Sharer-link) is the responsibility
//! of `app/src/terminal/view/shared_session`.
//!
//! # Wire format
//!
//! One JSON object per line:
//!
//! ```json
//! {"kind":"text","value":"hello\n","client_id":"opaque","ts":"2026-05-10T00:00:00Z"}
//! ```
//!
//! See [`protocol::InputMsg`] for the full schema.
//!
//! [Sharer-link]: https://github.com/warpdotdev/warp/blob/main/app/src/terminal/view/shared_session/sharer/mod.rs

pub mod auth;
pub mod error;
pub mod protocol;
pub mod server;

pub use error::{Error, Result};
pub use protocol::{InputMsg, MessageKind};
pub use server::Server;
