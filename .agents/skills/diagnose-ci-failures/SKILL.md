---
name: diagnose-ci-failures
description: Diagnose CI failures for a PR using the GitHub CLI, extract error logs, and generate a plan to fix them. Use when the user asks to check CI status, pull CI issues, triage test failures, or investigate PR build failures.
---

# diagnose-ci-failures

Programmatically diagnose CI failures for a PR and generate a plan to fix them.

## Overview

This skill provides a deterministic workflow to check CI status for a PR, extract failure logs, analyze errors, and create a plan (not code changes) to resolve issues. The output is always a plan document that can be reviewed before execution.

## Workflow

### 1. Verify PR exists for current branch

Get the current branch and check if a PR exists:

```bash
# Get current branch
git branch --show-current

# Check for PR
gh --no-pager pr view <branch-name> --json number,title,url,state
```

If no PR exists, inform the user and offer to create one using the `create-pr` skill.

### 2. Check CI status

Fetch the status of all CI checks:

```bash
gh pr view <branch-name> --json statusCheckRollup
```

Parse the output to identify:
- Completed checks vs. in-progress checks
- Successful checks
- Failed checks with their names and details URLs

If CI is still running, inform the user which checks have already failed or passed, highlight the checks that are still running, and suggest waiting for completion before diagnosis.

### 3. Extract failure logs

For each failed check, pull the logs using the run ID from the status check:

```bash
gh run view <run-id> --log-failed
```

Focus on extracting:
- Error messages and their locations (file paths, line numbers)
- Compilation errors (unused imports, type mismatches, etc.)
- Linting/clippy errors with specific lint names
- Test failure messages and stack traces
- Build failures and their root causes

### 4. Categorize errors

Group errors by type:
- **Formatting issues**: `cargo fmt` failures
- **Linting issues**: `cargo clippy` warnings/errors
- **Compilation errors**: Type errors, missing imports, signature mismatches
- **Test failures**: Failing tests with their names and failure reasons
- **Platform-specific issues**: WASM, Linux, macOS, Windows-specific failures

### 5. Generate fix plan

Create a plan document (using `create_plan` tool) with:
- **Problem Statement**: Summary of failing checks
- **Current State**: What errors were found and where
- **Proposed Changes**: Specific fixes needed for each error category
- **Validation Steps**: Commands to verify fixes (fmt, clippy, tests, presubmit)

The plan should reference the `fix-errors` skill for detailed guidance on resolving specific error types.

## Important Notes

- **Always create a plan first**: Never make code changes directly. Generate a plan for user review
- **Check test status in CI**: Even if tests fail locally, verify they passed in CI before flagging as issues
- **Unrelated test failures**: If tests passed in CI but fail locally, they may be environment-specific or flaky
- **Multiple error types**: Fix one category at a time (e.g., all clippy errors before tests)
- **Cross-reference fix-errors skill**: For detailed error resolution strategies, use the `fix-errors` skill

## Common CI Check Names

- `Formatting + Clippy (MacOS)`
- `Formatting + Clippy (Linux)`
- `Run MacOS tests`
- `Run Linux tests`
- `Run Windows tests`
- `Check CI results` (summary check)
- `WASM build`

## Example Commands

**Get PR status with details:**
```bash
gh --no-pager pr view --json number,title,state,statusCheckRollup
```

**Get logs from specific failed run:**
```bash
gh run view 12345678 --log-failed
```

**Check for specific error in logs:**
```bash
gh run view 12345678 --log-failed 2>&1 | grep -A 5 "error:"
```
