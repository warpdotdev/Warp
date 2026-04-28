---
name: create-pr
description: Create a pull request in the warp repository for the current branch. Use when the user mentions opening a PR, creating a pull request, submitting changes for review, or preparing code for merge.
---

# create-pr

## Overview

This guide covers best practices for creating pull requests in the warp repository, including merging master, running presubmit checks, linking Linear tasks, ensuring appropriate test coverage, and structuring your PR for effective review.

## Related Skills

- `fix-errors` - Fix presubmit failures (formatting, linting, tests) before opening PR
- `rust-unit-tests` - Write unit tests for your changes, if applicable (see "Testing Requirements" below)
- `warp-integration-test` - Add or update integration coverage for user-visible flows, regressions, and P0 use cases
- `add-feature-flag` - Gate changes behind feature flags

## Pre-PR Checklist

### 1. Merge master into your feature branch

**Always merge master into your feature branch before starting the review process.**

```bash
git fetch origin
git merge origin/master
```

Resolve any merge conflicts locally before opening the PR.

### 2. Run presubmit checks for code changes

If the PR includes code changes, run the relevant presubmit checks before opening or updating it:

```bash
./script/presubmit
```

`./script/presubmit` runs:
- `cargo fmt` - Code formatting
- `cargo clippy` - Linting with all warnings as errors
- All tests (unit, doc, and integration)
If the PR is documentation-only (for example, skills, markdown, or other non-code content), you do not need to run `cargo fmt` or `cargo clippy` just to open or update the PR.

If presubmit fails for a code-changing PR, use the `fix-errors` skill to resolve issues.

**You must run `cargo fmt` and `cargo clippy` before:**
- Opening a new PR that includes code changes
- Pushing new commits that include code changes to an existing PR branch
- Any reviewed branch update that changes code

### 3. Review your changes

Before creating a PR, review what changes you're about to submit:

```bash
# View commits in your branch (comparing against base branch)
git --no-pager log <base-branch>..HEAD --oneline

# View file statistics for changes
git --no-pager diff <base-branch>...HEAD --stat

# View full diff
git --no-pager diff <base-branch>...HEAD
```

This helps you:
- Verify all intended changes are included
- Catch unintended changes before review
- Write an accurate PR description
- Ensure you're comparing against the correct base branch
- **Tests:** Include tests when required—bug fixes (regression test), algorithmic code (unit tests), UI components (layout test), P0 use cases (integration test). See Testing Requirements below.

### 4. Link to Linear task

When possible, PRs should be associated with a Linear task. Use the Linear MCP tool (if available) to find corresponding issues.

**Branch naming convention:**
Remote branches should be prefixed with your name (e.g., `zheng/feature`, `alice/fix-bug`).

**How to link PRs to Linear:**
Include the issue ID in the PR title (e.g., `[WARP-1234] Add new feature`). Do this **before** creating the PR for automatic linking.

### 5. Open the PR

Use the PR template at `.github/pull_request_template.md` when opening PRs.

Add changelog entries when appropriate using the format at the bottom of the PR template. Some examples:
- Feature: "Global search in files across your current directories. Use CMD-F/CTRL-SHIFT-F to open."
- Improvement: "Added horizontal autoscrolling when jumping to line/column."
- Bug fix: "Fixed session viewer input being cleared when agent runs commands.

**CLI workflow:**

- **Check if PR exists** for current branch:
  ```bash
  gh pr view --json number,url
  ```
  Exit code 0 if PR exists, 1 if not.

- **Create a new PR:**
  ```bash
  # With title and body
  gh pr create --title "Title" --body "Description" --draft

  # Auto-fill from commits
  gh pr create --fill --draft

  # Use PR template file
  gh pr create --body-file .github/pull_request_template.md --title "Title" --draft
  ```
  Key flags: `--draft` / `-d`, `--fill` / `-f`, `--body-file` / `-F`, `--web` / `-w`

- **Update an existing PR:**
  ```bash
  gh pr edit --title "New title" --body "New body"
  gh pr edit --add-reviewer username --add-label bug
  ```

- **Mark PR ready for review:**
  ```bash
  gh pr ready
  ```

### 6. Include co-author attribution

When committing changes or creating a PR, include attribution at the end of every commit message or PR description:

```
Co-Authored-By: Warp <agent@warp.dev>
```

## Testing Requirements

### Bug fixes require regression tests

**All bug fixes should be accompanied by a regression test.** This helps prevent re-breaking something that was already broken once.

The test should:
- Reproduce the original bug (would fail before the fix)
- Pass after the fix is applied
- Be clearly named to indicate what bug it's preventing

### Algorithmic code requires unit tests

Code with non-trivial logic should have unit tests to validate functionality:

**Examples of what needs unit tests:**
- Custom data structures (e.g., `SumTree`)
- Search-related APIs that should return expected results for a given query
- Core layout code in the UI framework
- Any algorithmic or computational logic

**Not required for:**
- Sufficiently-simple functions
- Trivial getters/setters

See the `rust-unit-tests` skill for guidance on writing unit tests.

### UI components need layout validation tests

**All UI components (implementations of `View`) should have a simple unit test** to validate that they can be laid out without a panic.

This provides high-level coverage over rendering "safety" (though not "correctness"):

```rust
#[test]
fn test_component_can_layout() {
    use warpui::App;
    use warp::test_util::{terminal::initialize_app_for_terminal_view, add_window_with_terminal};
    
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let term = add_window_with_terminal(&mut app, None);
        
        // Render the component - should not panic
        term.update(&mut app, |view, ctx| {
            // Create and layout your component
        });
    })
}
```

### Ask before skipping integration coverage

If the PR changes a user-visible flow, fixes an end-to-end regression, or otherwise looks like it would benefit from integration coverage, use the `ask_user_question` tool before creating or updating the PR to ask whether the user wants an integration test added as part of the work.

Prefer a direct choice such as:

- `Yes, add an integration test before creating the PR`
- `No, continue without an integration test`

If the user chooses to add one, use the `warp-integration-test` skill.

### P0 use cases require integration tests

**All "P0 use cases" require an integration test** that covers the behavior/flow in question.

**A "P0 use case" is defined as:** Any behavior of the application that, if broken, warrants an out-of-band release.

Integration tests should:
- Exercise the full user-facing flow
- Validate end-to-end functionality
- Be placed in the `integration/` directory

Use the `warp-integration-test` skill for implementation details, test registration steps, and validation workflow.

## PR Description Guidelines

Your PR summary under the "Description" section should include:

1. **What** - What changes are being made
2. **Why** - Why these changes are necessary (link to Linear task if applicable)
3. **How** - Brief explanation of the approach taken

## After Opening the PR

1. **Monitor CI checks** - Ensure all automated checks pass
2. **Respond to review comments** - Address feedback promptly
3. **Keep the PR up to date** - Merge master if conflicts arise
4. **Re-run relevant validation** - After making changes based on review feedback. For code changes, re-run `cargo fmt`/`cargo clippy` (and other relevant checks); for documentation-only changes, this is not required.

## Best Practices

- **Keep PRs focused** - One logical change per PR when possible
- **Write clear commit messages** - Explain what and why, not just what
- **Self-review first** - Review your own diff before requesting review
- **Update tests** - Ensure test coverage reflects your changes
- **Document breaking changes** - Call out any API changes or breaking modifications
- **Use feature flags** - Gate risky changes behind feature flags when appropriate (see the `add-feature-flag` skill)
