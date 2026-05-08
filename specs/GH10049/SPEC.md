# Spec: Unified file-read interface (GH-10049)

## Problem

The codebase uses ad-hoc `std::fs::read`, `read_to_string`, and
similar calls scattered across crates. There is no central place that
enforces:

- A maximum file size (so a stray gigabyte file can't OOM the editor).
- A binary-vs-text decision (so a binary blob can't get fed into a
  string-typed pipeline).
- A consistent error type for "file too big" / "file is binary" /
  "invalid UTF-8".

The existing approach has caused production OOMs and silent garbage
when binary content gets pulled into UI/AI surfaces.

## Goal

Add `crates/warp_files/src/safe_read.rs` (or a sibling crate) exposing
the `safe_read` module. Provide a small set of functions covering the
sync and async paths and migrate existing callers in waves. Add a
clippy lint restricting raw `std::fs::read*` and `tokio::fs::read*`
calls to the new helpers.

## Public API

V1 (sync only — ships first):

- `safe_read::read_to_string(path, opts) -> Result<String, SafeReadError>`
- `safe_read::read_bytes(path, opts) -> Result<Vec<u8>, SafeReadError>`

V1.5 (async helpers — ships before the lint is promoted to deny):

- `safe_read::async_read_to_string(path, opts) -> Result<String, SafeReadError>`
- `safe_read::async_read_bytes(path, opts) -> Result<Vec<u8>, SafeReadError>`

Both async helpers wrap the sync implementations on a Tokio
`spawn_blocking` boundary and apply the same size, UTF-8, and binary
checks. They are the compliant async path the lint will direct callers
to once promoted.

## Behavior contract

- B1. Text reads (`read_to_string` / `async_read_to_string`) reject
  files larger than `opts.max_bytes` (default: editor large-file
  threshold, ~8 MiB) with
  `SafeReadError::TooLarge { actual, max, path }`.
- B2. Text reads reject files whose first 8 KiB contain a NUL byte
  with `SafeReadError::BinaryContent { path }` to prevent silent
  corruption of `String`-typed pipelines. Callers that legitimately
  want raw bytes must use `read_bytes` / `async_read_bytes`.
- B3. Text reads validate UTF-8 on the full payload. Invalid UTF-8
  returns `SafeReadError::InvalidUtf8 { path, byte_offset }` where
  `byte_offset` points at the first invalid sequence. This error is
  non-retryable (re-reading the same bytes will fail the same way).
- B4. `BinaryContent` and `InvalidUtf8` are both non-retryable.
  Callers must switch to `read_bytes` to handle binary or
  non-UTF-8 content.
- B5. Byte reads (`read_bytes` / `async_read_bytes`) enforce
  `max_bytes` only — no NUL/UTF-8 check.
- B6. All four helpers stream-read for files larger than 1 MiB and
  short-circuit on the size check before allocating the full
  buffer.

## Error type

```rust
pub enum SafeReadError {
    TooLarge { actual: u64, max: u64, path: PathBuf },
    BinaryContent { path: PathBuf },
    InvalidUtf8 { path: PathBuf, byte_offset: usize },
    Io { path: PathBuf, source: std::io::Error },
}
```

`BinaryContent` and `InvalidUtf8` are non-retryable: callers must
either use `read_bytes` or surface the failure to the user.

## Lint enforcement

A `.clippy.toml` at the workspace root configures the
`disallowed-methods` lint:

```toml
disallowed-methods = [
    { path = "std::fs::read",                 reason = "use safe_read::read_bytes" },
    { path = "std::fs::read_to_string",       reason = "use safe_read::read_to_string" },
    { path = "tokio::fs::read",               reason = "use safe_read::async_read_bytes" },
    { path = "tokio::fs::read_to_string",     reason = "use safe_read::async_read_to_string" },
]
```

### Path allowlist

The lint is suppressed in the following path patterns. These
suppressions are documented in the implementation crate's lint
configuration (via `#![allow(clippy::disallowed_methods)]` at the
file or module head) so the allowlist is auditable from source:

- `crates/warp_files/src/safe_read.rs` — the implementation itself
  must call the underlying `std::fs` / `tokio::fs` primitives.
- `**/tests/**` — integration test directories.
- `**/*_test.rs` — unit test files following the in-tree convention.
- `**/fixtures/**` — test fixture helpers.
- Any module gated entirely by `#[cfg(test)]`.

Production code outside these patterns has no escape hatch other than
migrating to `safe_read`.

### Warn → deny promotion plan

- **V1**: lint level is `warn`. The four `safe_read` sync helpers
  ship. The five high-traffic call sites listed below are migrated
  in the same PR. Other call sites remain as warnings so contributors
  see them in CI output without a hard block.
- **V1.5**: ship the four async helpers. Begin migrating async call
  sites. Lint level remains `warn`.
- **V2**: once the open-warning count from `cargo clippy --all-targets`
  reaches zero on `master` for one release cycle, promote the lint
  level to `deny` in `.clippy.toml`. From this point forward, any new
  `std::fs::read*` or `tokio::fs::read*` call outside the allowlist
  fails CI.

The promotion is gated on the warning count rather than a fixed date
so the migration milestone is the trigger.

## High-traffic V1 migration targets

Identified via `git grep "fs::read"` in app/src and crates/:
1. `app/src/code/local_code_editor.rs` (file open path).
2. `crates/ai/src/skills/parse_skill.rs::parse_markdown_file`.
3. `app/src/changelog_model.rs` (changelog read).
4. `app/src/notebooks/file/...` (notebook restore).
5. `app/src/integration_testing/...` (test fixtures, opt-out via
   `#[cfg(test)]`).

## Test plan

- T1. `safe_read::read_to_string(small_text_file, default) -> Ok(content)`.
- T2. `safe_read::read_to_string(huge_file, default) -> TooLarge`.
- T3. `safe_read::read_to_string(binary_file, default) -> BinaryContent`.
- T4. `safe_read::read_bytes(binary_file, default) -> Ok(bytes)`.
- T5. `safe_read::read_to_string(invalid_utf8_file, default) -> InvalidUtf8`
  with `byte_offset` pointing at the first invalid sequence.
- T6. Stream-short-circuit: a 100 MiB file rejected without
  allocating 100 MiB (verify peak heap via a `dhat`-instrumented
  test or a smaller `max_bytes` ceiling).
- T7. Clippy lint fires on a fixture call to `std::fs::read`
  outside the implementation and outside the test allowlist.
- T8. Clippy lint does not fire on the same call inside a
  `**/tests/**` file.
- T9. Migration regression: each V1 migrated call site has a
  test that round-trips through the new helper.
- T10. (V1.5) Async parity: `async_read_to_string` and
  `async_read_bytes` produce identical errors and successful
  payloads as their sync counterparts on the same fixtures.

## Files touched

- `crates/warp_files/src/safe_read.rs` (new — sync helpers in V1,
  async helpers added in V1.5).
- `crates/warp_files/src/error.rs` (new — `SafeReadError` enum).
- `.clippy.toml` (new entries; level `warn` in V1, `deny` in V2).
- 5 call-site migrations listed above (V1).
- Per-callsite tests.

## Non-goals

- Configurable per-extension binary detection (`.svg` is XML but may
  have NULs in CDATA). Callers that legitimately need bytes use
  `read_bytes`.
- Migrating every call site in V1. Lint-as-`warn` lets late call
  sites surface gradually without blocking the V1 PR.
- Streaming/iterator APIs that yield content without ever buffering.
  Out of scope for this spec; `safe_read` is for whole-file reads.

Note: async helpers were originally listed as out of scope. They are
now in scope as V1.5 because the lint promotion to `deny` would
otherwise leave async callers with no compliant path.
