#!/usr/bin/env python3
"""Fetch PRs merged in a release range and extract explicit CHANGELOG markers.

Uses `gh` CLI (must be authenticated) and `git` — stdlib only, no pip deps.

Usage:
    python3 fetch_prs.py --repo warpdotdev/warp --base-ref <prev_tag> --head-ref <release_tag>

Outputs JSON to stdout.
"""

import argparse
import json
import re
import subprocess
import sys

# Matches lines like: CHANGELOG-NEW-FEATURE: Added dark mode
MARKER_RE = re.compile(
    r"^CHANGELOG-(NEW-FEATURE|IMPROVEMENT|BUG-FIX|IMAGE|OZ|NONE)\s*:?\s*(.*)$",
    re.MULTILINE,
)

# Matches issue-closing keywords: Fixes #123, Closes #456, Resolves #789
LINKED_ISSUE_RE = re.compile(
    r"(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)",
    re.IGNORECASE,
)


def run(cmd: list[str], *, check: bool = True) -> str:
    result = subprocess.run(cmd, capture_output=True, text=True, check=check)
    return result.stdout.strip()


def get_commits(base_ref: str, head_ref: str) -> list[str]:
    """Return SHAs of first-parent commits between base and head."""
    log = run(
        [
            "git",
            "log",
            "--first-parent",
            "--format=%H",
            f"{base_ref}..{head_ref}",
        ]
    )
    if not log:
        return []
    return log.splitlines()


def extract_pr_number(sha: str) -> int | None:
    """Extract PR number from a squash-merge commit subject line.

    Expects the GitHub squash format: 'feat: something (#1234)'.
    """
    msg = run(["git", "log", "-1", "--format=%s", sha])
    m = re.search(r"#(\d+)", msg)
    if m:
        return int(m.group(1))
    return None


def fetch_pr_data(repo: str, pr_number: int) -> dict | None:
    """Fetch PR metadata and changed file paths via gh CLI."""
    fields = "number,title,author,body,labels,mergedAt,files"
    raw = run(
        ["gh", "pr", "view", str(pr_number), "--repo", repo, "--json", fields],
        check=False,
    )
    if not raw:
        return None
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return None


def extract_linked_issues(body: str) -> list[int]:
    """Extract issue numbers from closing keywords in a PR body."""
    if not body:
        return []
    return sorted(set(int(m.group(1)) for m in LINKED_ISSUE_RE.finditer(body)))


def extract_markers(body: str) -> list[dict]:
    """Extract CHANGELOG-* markers from a PR body."""
    if not body:
        return []
    entries = []
    has_opt_out = False
    for m in MARKER_RE.finditer(body):
        category = m.group(1)
        text = m.group(2).strip()
        # CHANGELOG-NONE is an explicit opt-out — skip all other markers
        if category == "NONE":
            has_opt_out = True
            continue
        # Skip template placeholders
        if text.startswith("{{") or text.startswith("{text") or not text:
            continue
        entries.append({"category": category, "text": text})
    # If the PR explicitly opted out, return a special marker
    if has_opt_out:
        return [{"category": "NONE", "text": ""}]
    return entries


def main() -> None:
    parser = argparse.ArgumentParser(description="Fetch PRs in a release range")
    parser.add_argument("--repo", required=True, help="GitHub repo (owner/name)")
    parser.add_argument("--base-ref", required=True, help="Previous release tag")
    parser.add_argument("--head-ref", required=True, help="Current release tag")
    args = parser.parse_args()

    commit_shas = get_commits(args.base_ref, args.head_ref)

    seen_prs: set[int] = set()
    prs: list[dict] = []

    for sha in commit_shas:
        pr_num = extract_pr_number(sha)
        if pr_num is None or pr_num in seen_prs:
            continue
        seen_prs.add(pr_num)

        data = fetch_pr_data(args.repo, pr_num)
        if data is None:
            continue

        author_login = ""
        if isinstance(data.get("author"), dict):
            author_login = data["author"].get("login", "")
        elif isinstance(data.get("author"), str):
            author_login = data["author"]

        label_names = []
        for lbl in data.get("labels", []) or []:
            if isinstance(lbl, dict):
                label_names.append(lbl.get("name", ""))
            else:
                label_names.append(str(lbl))

        body = data.get("body", "") or ""
        explicit_entries = extract_markers(body)
        linked_issues = extract_linked_issues(body)

        file_paths = []
        for f in data.get("files", []) or []:
            if isinstance(f, dict):
                file_paths.append(f.get("path", ""))

        prs.append(
            {
                "number": data.get("number", pr_num),
                "title": data.get("title", ""),
                "author": author_login,
                "body": body,
                "labels": label_names,
                "merged_at": data.get("mergedAt", ""),
                "explicit_entries": explicit_entries,
                "linked_issues": linked_issues,
                "changed_files": file_paths,
            }
        )

    output = {
        "range": {"base": args.base_ref, "head": args.head_ref},
        "prs": prs,
    }
    json.dump(output, sys.stdout, indent=2)
    print()  # trailing newline


if __name__ == "__main__":
    main()
