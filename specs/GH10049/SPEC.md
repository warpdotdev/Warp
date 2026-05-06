# Spec: Unified file-read interface (GH-10049)

## Problem

The codebase uses ad-hoc `std::fs::read`, `read_to_string`, and
similar calls scattered across crates. There is no central place that
enforces:

- A maximum file size (so a stray gigabyte file can't OOM the editor).
- A binary-vs-text decision (so a binary blob can't get fed into a
  string-typed pipeline).
- A consistent error type for "file too big" / "file is binary".

The existing approach has caused production OOMs and silent garbage
when binary content gets pulled into UI/AI surfaces.

## Goal

Add `crates/warp_files/src/read.rs` (or a sibling crate) with a single
`read_text_file_safe(path, opts) -> Result<String, ReadError>` and
`read_binary_file_safe(path, opts) -> Result<Vec<u8>, ReadError>`
helper. Migrate existing callers in waves. Add a clippy lint
restricting raw `std::fs::read*` calls to the new helpers.

## Behavior contract

- B1. `read_text_file_safe` rejects files larger than
  `opts.max_bytes` (default: editor large-file threshold,
  ~8 MiB) with `ReadError::TooLarge { actual, max }`.
- B2. `read_text_file_safe` rejects files whose first 8 KiB
  contain a NUL byte with `ReadError::Binary`. Configurable via
  `opts.allow_binary`.
- B3. `read_binary_file_safe` enforces `max_bytes` only — the
  binary check is skipped.
- B4. Both helpers stream-read for files larger than 1 MiB and
  short-circuit on the size check before allocating the full
  buffer.
- B5. A new clippy `disallowed_methods` config in
  `.clippy.toml` lists `std::fs::read`, `std::fs::read_to_string`,
  and `tokio::fs::read{,_to_string}` as denied except in:
  - `crates/warp_files/src/read.rs` (the implementation itself).
  - Test files (`#[cfg(test)]`).
- B6. Migration is staged: V1 ships the helpers + the lint
  configured as `warn`. V2 (after one release of stabilization)
  promotes to `deny`. V1 also migrates the 5 highest-traffic
  call sites.

## High-traffic V1 migration targets

Identified via `git grep "fs::read"` in app/src and crates/:
1. `app/src/code/local_code_editor.rs` (file open path).
2. `crates/ai/src/skills/parse_skill.rs::parse_markdown_file`.
3. `app/src/changelog_model.rs` (changelog read).
4. `app/src/notebooks/file/...` (notebook restore).
5. `app/src/integration_testing/...` (test fixtures, opt-out via
   `#[cfg(test)]`).

## Test plan

- T1. `read_text_file_safe(small_text_file, default) -> Ok(content)`.
- T2. `read_text_file_safe(huge_file, default) -> TooLarge`.
- T3. `read_text_file_safe(binary_file, default) -> Binary`.
- T4. `read_text_file_safe(binary_file, allow_binary=true) -> Ok(...)`.
- T5. Stream-short-circuit: a 100 MiB file rejected without
  allocating 100 MiB (verify peak heap via a `dhat`-instrumented
  test or a smaller `max_bytes` ceiling).
- T6. Clippy lint fires on a fixture call to `std::fs::read`
  outside the implementation.
- T7. Migration regression: each V1 migrated call site has a
  test that round-trips through the new helper.

## Files touched

- `crates/warp_files/src/read.rs` (new).
- `crates/warp_files/src/error.rs` (new — `ReadError` enum).
- `.clippy.toml` (new entries).
- 5 call-site migrations listed above.
- Per-callsite tests.

## Out of scope (V1)

- Async/await variants (`read_text_file_safe_async`). The five V1
  migration targets are sync; async wrapper is a follow-up.
- Configurable per-extension binary detection (`.svg` is XML but
  may have NULs in CDATA). Use `allow_binary: true` for now.
- Migrating every call site. Lint-as-warn lets late call sites
  surface gradually without blocking the V1 PR.
