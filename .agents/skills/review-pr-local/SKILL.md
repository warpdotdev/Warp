---
name: review-pr-local
specializes: review-pr
description: Repo-specific review guidance for warp-external. Only the categories declared overridable by the core review-pr skill may be specialized here.
---

# Repo-specific review guidance for `warp-external`

This file is a companion to the core `review-pr` skill. It does not
redefine the review output schema, severity labels, safety rules, or
evidence rules. It only specializes the override categories the core
skill marks as overridable.

## Repo-specific style and recurring review patterns

- Do not suggest adding test cases that only vary constructor inputs or struct fields when an existing test already covers the meaningful behavior. Only suggest new tests when they exercise a distinct code path or edge case.
- When a PR is clearly a V0 or initial implementation, frame robustness suggestions such as timeouts, retries, and lifecycle management as optional future work rather than blocking concerns, unless they risk correctness, security, data loss, or a persistent UI hang.
- For Rust changes, apply the repository conventions from `WARP.md`: avoid unnecessary type annotations, prefer imports over long path qualifiers, name context parameters `ctx` and place them last, remove unused parameters instead of prefixing them with `_`, and prefer inline format arguments in macros.
- Avoid wildcard `_` match arms when an enum can reasonably be matched exhaustively; exhaustive matches are preferred so future variants are surfaced during review.
- For new or changed feature flags, prefer high-level runtime checks with `FeatureFlag::YourFlag.is_enabled()` over `#[cfg(...)]` unless the code cannot compile without a compile-time gate.
- Flag nested or redundant `TerminalModel` locking when the call stack may already hold the model lock. Prefer passing locked references down the stack and keeping lock scopes short.
- In WarpUI code, flag inline `MouseStateHandle::default()` usage during render or event handling. Mouse state handles should be created during construction and then cloned/referenced where needed.
- For user-facing UI changes, mention missing validation only when it is tied to a concrete risk or when the PR changes behavior that should be verified visually.

## User-facing strings

- Flag interpolated text that would read unnaturally at runtime or combine sentence fragments with the wrong casing.
- Link text should be descriptive rather than bare URLs or generic "click here" labels.
- Verify that product terminology is consistent across related UI, comments, workflow messages, and errors in the same PR.

## Graceful degradation and observability

- When optional dynamic data such as URLs, session links, workflow links, issue numbers, or metadata may be absent, prefer omitting the element or showing a short fallback over rendering empty or broken output.
- Do not suggest removing session links, workflow URLs, or diagnostic context from error paths. Those links are important for debugging failed automation and user reports.
- Prefer generic, user-safe error text in user-visible surfaces, but keep enough structured logging or diagnostic context for maintainers to investigate failures.
