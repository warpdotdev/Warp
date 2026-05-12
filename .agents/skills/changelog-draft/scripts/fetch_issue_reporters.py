#!/usr/bin/env python3
"""Fetch the original reporters for GitHub issues linked to PRs in a release.

Uses `gh` CLI (must be authenticated) — stdlib only, no pip deps.

Usage:
    python3 fetch_issue_reporters.py --repo warpdotdev/warp --issues 1234,5678,9012

Outputs JSON to stdout mapping issue numbers to reporter info.
"""

import argparse
import json
import subprocess
import sys


def run(cmd: list[str], *, check: bool = True) -> str:
    result = subprocess.run(cmd, capture_output=True, text=True, check=check)
    return result.stdout.strip()


def fetch_issue_reporter(repo: str, issue_number: int) -> dict | None:
    """Fetch the reporter (author) of a GitHub issue via gh CLI."""
    raw = run(
        [
            "gh",
            "issue",
            "view",
            str(issue_number),
            "--repo",
            repo,
            "--json",
            "number,title,author,url",
        ],
        check=False,
    )
    if not raw:
        return None
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return None

    author = ""
    if isinstance(data.get("author"), dict):
        author = data["author"].get("login", "")
    elif isinstance(data.get("author"), str):
        author = data["author"]

    return {
        "issue_number": data.get("number", issue_number),
        "title": data.get("title", ""),
        "reporter": author,
        "url": data.get("url", ""),
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Fetch issue reporters for linked issues"
    )
    parser.add_argument("--repo", required=True, help="GitHub repo (owner/name)")
    parser.add_argument(
        "--issues",
        required=True,
        help="Comma-separated issue numbers",
    )
    args = parser.parse_args()

    issue_numbers = [
        int(n.strip()) for n in args.issues.split(",") if n.strip().isdigit()
    ]

    reporters: list[dict] = []
    for num in issue_numbers:
        info = fetch_issue_reporter(args.repo, num)
        if info:
            reporters.append(info)

    json.dump({"issue_reporters": reporters}, sys.stdout, indent=2)
    print()  # trailing newline


if __name__ == "__main__":
    main()
