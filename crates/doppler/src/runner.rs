// SPDX-License-Identifier: AGPL-3.0-only
//
// Thin abstraction over `tokio::process::Command` so tests can substitute
// the runner without spawning the real `doppler` binary.

use std::io;
use std::process::Output;

/// A pluggable command runner. The default implementation
/// ([`TokioCommandRunner`]) shells out to the local `doppler` binary; tests
/// substitute a mock to avoid actually invoking the CLI.
#[async_trait::async_trait]
pub trait CommandRunner: Send + Sync {
    /// Run `doppler` with the given args and return its [`Output`].
    async fn run(&self, args: &[&str]) -> io::Result<Output>;
}

/// Default [`CommandRunner`] that spawns the real `doppler` binary via
/// `tokio::process::Command`.
pub struct TokioCommandRunner;

#[async_trait::async_trait]
impl CommandRunner for TokioCommandRunner {
    async fn run(&self, args: &[&str]) -> io::Result<Output> {
        tokio::process::Command::new("doppler")
            .args(args)
            .output()
            .await
    }
}
