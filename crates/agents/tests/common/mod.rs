//! Shared helpers for the `agents` integration tests.
//!
//! Right now this is just a process-global mutex used to serialize tests
//! that mutate `PATH`. `cargo test` runs every `#[test]` in the same process,
//! so any concurrent `env::set_var("PATH", …)` racing against another test
//! that calls `which::which` is undefined behaviour. Tests touching the
//! environment must hold this lock for the duration of the mutation and the
//! `which::which` call.

use std::sync::Mutex;

/// Lock held while mutating process-global `PATH` in a test.
pub static ENV_LOCK: Mutex<()> = Mutex::new(());
