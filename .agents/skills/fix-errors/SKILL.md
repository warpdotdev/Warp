---
name: fix-errors
description: Fix compilation errors, linting issues, and test failures in the warp Rust codebase. Covers presubmit checks, WASM-specific errors, and running specific tests. Use when the user hits build errors, clippy or fmt failures, test failures, or needs to run or interpret presubmit before a PR.
---

# fix-errors

Fix compilation errors, linting issues, and test failures in the warp Rust codebase.

## Overview

This skill helps resolve common issues encountered during development, including:
- Compilation errors (unused imports, type mismatches, etc.)
- Linting failures (clippy warnings)
- Formatting violations
- WASM-specific errors
- Test failures

Before opening or updating a pull request, all presubmit checks must pass.

## Presubmit Checks

Run all presubmit checks at once:

```bash
./script/presubmit
```

This runs formatting, linting, and all tests. If it passes, you're ready to open a PR.

### Individual Checks

Run checks separately when debugging specific issues:

**Rust formatting:**
```bash
cargo fmt -- --check
```

**Clippy (full workspace):**
```bash
cargo clippy --workspace --exclude warp_completer --all-targets --all-features --tests -- -D warnings
cargo clippy -p warp_completer --all-targets --tests -- -D warnings
```

**WASM Clippy:**
```bash
cargo clippy --target wasm32-unknown-unknown --profile release-wasm-debug_assertions --no-deps
```

**Objective-C/C/C++ formatting:**
```bash
./script/run-clang-format.py -r --extensions 'c,h,cpp,m' ./crates/warpui/src/ ./app/src/
```

**All tests:**
```bash
cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2
cargo nextest run -p warp_completer --features v2
```

**Doc tests:**
```bash
cargo test --doc
```

## Running Specific Tests

**Single package:**
```bash
cargo nextest run -p <package_name>
```

**Filter by test name:**
```bash
cargo nextest run -E 'test(<substring>)'
```

**Specific package with filter:**
```bash
cargo nextest run -p <package_name> -E 'test(<substring>)'
```

**With output (no capture):**
```bash
cargo nextest run -p <package> --nocapture
```

## Common Error Types

### Unused Imports
Remove unused `use` statements identified by the compiler.

### Unused Constants
Remove constants that are defined but never used.

### Unknown Imports
Add the correct `use` statement for undefined types. Search the codebase to find the correct module path.

### Type Mismatches
Update function calls to pass arguments of the correct type. Common fixes:
- Use `.as_str()` instead of `.clone()` when a `&str` is expected
- Use `&value` when a reference is needed
- Use `.to_string()` when `String` is expected but `&str` is provided

### Struct Field Changes
When a struct adds/removes fields, update all places where it's constructed or destructured:
- Struct initialization
- Pattern matching (`match`, `if let`)
- Destructuring assignments

### Function Signature Changes
When a function adds a new parameter, update all call sites to provide the new argument:
- For `bool` params: pass `true` or `false` based on context
- For `Option<T>` params: pass `None` as default or `Some(value)` if needed

### Enum Variant Changes
When adding a new enum variant, update exhaustive `match` statements:
- Add a new match arm with appropriate handling
- Mirror the implementation pattern of similar variants

### Incorrect Trait Implementation
Fix trait implementations that return the wrong type or don't satisfy trait bounds.

### WASM-Specific Errors

WASM builds (`wasm32-unknown-unknown` target) don't support filesystem operations. Code that uses filesystem APIs must be gated behind the `local_fs` feature flag.

**Common WASM errors:**
- Dead code warnings for code only used in non-WASM builds
- Unused code that's only relevant when `local_fs` is available
- Tests that require filesystem access

**Fixes:**

**Gate tests behind `local_fs`:**
```rust
#[test]
#[cfg(feature = "local_fs")]
fn test_find_git_repo_with_worktree() {
    // Test that uses filesystem operations
}
```

**Conditionally allow dead code for types only used when `local_fs` is enabled:**
```rust
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
#[derive(Clone, EnumDiscriminants, Serialize)]
pub enum ExampleType {
    // Variants only used when local_fs is enabled
    Variant1,
    Variant2,
    Variant3,
}
```

WASM errors are discovered by running:

```bash
cargo clippy --target wasm32-unknown-unknown --profile release-wasm-debug_assertions --no-deps
```

## Best Practices

**Before fixing:**
- Read the full error message to understand the root cause
- Check if multiple errors are related (fixing one may resolve others)
- For trait/type errors, verify you understand the expected vs actual types
- For WASM errors, check if code needs to be gated behind `local_fs`

**When fixing:**
- Fix one error type at a time when there are multiple issues
- Run `cargo check` frequently to verify fixes
- For WASM errors, run WASM clippy to verify the fix
- For complex changes, run relevant tests after fixing

**After fixing:**
- Always run `cargo fmt` and `cargo clippy` before pushing
- Run the full presubmit script before opening or updating a PR. Use the `create-pr` skill for more detailed instructions
- Verify tests pass in the areas you modified
