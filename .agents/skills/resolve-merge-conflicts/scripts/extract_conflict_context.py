#!/usr/bin/env python3
"""Summarize and extract compact merge-conflict context from a Git repository."""

from __future__ import annotations

import argparse
import difflib
import json
import re
import subprocess
import sys
from pathlib import Path


START_RE = re.compile(r"^<<<<<<<(?: (.*))?$")
BASE_RE = re.compile(r"^\|\|\|\|\|\|\|(?: (.*))?$")
END_RE = re.compile(r"^>>>>>>>(?: (.*))?$")


def run_git(repo_root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo_root), *args],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip() or "unknown git error"
        raise RuntimeError(f"git {' '.join(args)} failed: {message}")
    return result.stdout


def find_repo_root(start: Path) -> Path:
    result = subprocess.run(
        ["git", "-C", str(start), "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip() or "not a git repository"
        raise RuntimeError(message)
    return Path(result.stdout.strip()).resolve()


def get_unmerged_entries(repo_root: Path) -> dict[str, dict[int, dict[str, str]]]:
    entries: dict[str, dict[int, dict[str, str]]] = {}
    output = run_git(repo_root, "ls-files", "-u", "-z")
    for record in output.split("\0"):
        if not record:
            continue
        metadata, path = record.split("\t", 1)
        mode, object_id, stage_text = metadata.split()
        file_entry = entries.setdefault(path, {})
        file_entry[int(stage_text)] = {"mode": mode, "object_id": object_id}
    return entries


def read_text_file(path: Path) -> list[str] | None:
    if not path.exists() or path.is_dir():
        return None
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    if "\x00" in text:
        return None
    return text.splitlines()


def read_stage_text(repo_root: Path, path: str, stage: int) -> list[str] | None:
    result = subprocess.run(
        ["git", "-C", str(repo_root), "show", f":{stage}:{path}"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    if "\x00" in result.stdout:
        return None
    return result.stdout.splitlines()


def truncate_lines(lines: list[str], max_lines: int) -> list[str]:
    if len(lines) <= max_lines:
        return lines
    omitted = len(lines) - max_lines
    return [*lines[:max_lines], f"... ({omitted} more lines omitted)"]


def build_diff(
    left_lines: list[str],
    right_lines: list[str],
    left_label: str,
    right_label: str,
    max_lines: int,
) -> list[str]:
    diff = list(
        difflib.unified_diff(
            left_lines,
            right_lines,
            fromfile=left_label,
            tofile=right_label,
            lineterm="",
        )
    )
    if not diff:
        diff = ["(no textual diff)"]
    return truncate_lines(diff, max_lines)


def classify_conflict(stages: list[int], marker_hunks: int) -> str:
    if marker_hunks:
        return "text"
    stage_set = set(stages)
    if stage_set == {2, 3}:
        return "add/add"
    if stage_set == {1, 2}:
        return "deleted-by-them"
    if stage_set == {1, 3}:
        return "deleted-by-us"
    if stage_set == {1, 2, 3}:
        return "index-only"
    return "unmerged"


def normalize_requested_path(repo_root: Path, raw_path: str) -> str:
    path = Path(raw_path)
    candidate = path.resolve() if path.is_absolute() else (repo_root / path).resolve()
    try:
        return str(candidate.relative_to(repo_root))
    except ValueError as error:
        raise RuntimeError(f"path is outside repository: {raw_path}") from error


def parse_conflict_hunks(lines: list[str], context: int) -> tuple[list[dict[str, object]], str | None]:
    hunks: list[dict[str, object]] = []
    index = 0
    while index < len(lines):
        start_match = START_RE.match(lines[index])
        if not start_match:
            index += 1
            continue

        start_index = index
        ours_label = start_match.group(1) or "ours"
        index += 1
        ours: list[str] = []
        base: list[str] = []
        theirs: list[str] = []
        base_label: str | None = None
        theirs_label = "theirs"

        while index < len(lines):
            base_match = BASE_RE.match(lines[index])
            if base_match:
                base_label = base_match.group(1) or "base"
                index += 1
                while index < len(lines) and lines[index] != "=======":
                    base.append(lines[index])
                    index += 1
                break
            if lines[index] == "=======":
                break
            ours.append(lines[index])
            index += 1

        if index >= len(lines) or lines[index] != "=======":
            return hunks, f"unterminated conflict starting at line {start_index + 1}"

        index += 1
        end_index = index
        while index < len(lines):
            end_match = END_RE.match(lines[index])
            if end_match:
                theirs_label = end_match.group(1) or "theirs"
                end_index = index
                index += 1
                break
            theirs.append(lines[index])
            index += 1
        else:
            return hunks, f"unterminated conflict starting at line {start_index + 1}"

        hunks.append(
            {
                "start_line": start_index + 1,
                "end_line": end_index + 1,
                "before_context": lines[max(0, start_index - context):start_index],
                "ours": ours,
                "ours_label": ours_label,
                "base": base or None,
                "base_label": base_label,
                "theirs": theirs,
                "theirs_label": theirs_label,
                "after_context": lines[index:index + context],
            }
        )

    return hunks, None


def build_summary_report(repo_root: Path, path: str, stage_entries: dict[int, dict[str, str]], context: int) -> dict[str, object]:
    worktree_lines = read_text_file(repo_root / path)
    hunks: list[dict[str, object]] = []
    parse_error = None
    if worktree_lines is not None:
        hunks, parse_error = parse_conflict_hunks(worktree_lines, context)
    stages = sorted(stage_entries)
    return {
        "path": path,
        "stages": stages,
        "conflict_type": classify_conflict(stages, len(hunks)),
        "marker_hunks": len(hunks),
        "parse_error": parse_error,
        "worktree_present": worktree_lines is not None,
        "hunks": hunks,
    }


def build_index_preview(repo_root: Path, report: dict[str, object], max_lines: int) -> dict[str, object]:
    path = str(report["path"])
    ours = read_stage_text(repo_root, path, 2)
    theirs = read_stage_text(repo_root, path, 3)
    base = read_stage_text(repo_root, path, 1)
    preview: dict[str, object] = {
        "ours": truncate_lines(ours, max_lines) if ours else None,
        "theirs": truncate_lines(theirs, max_lines) if theirs else None,
        "base": truncate_lines(base, max_lines) if base else None,
    }
    if ours and theirs:
        preview["ours_vs_theirs_diff"] = build_diff(ours, theirs, "ours", "theirs", max_lines)
    return preview


def section_lines(title: str, lines: list[str] | None) -> list[str]:
    if lines is None:
        return [f"{title}:", "  (not present)"]
    if not lines:
        return [f"{title}:", "  (empty)"]
    return [f"{title}:", *[f"  {line}" for line in lines]]


def render_summary_text(repo_root: Path, reports: list[dict[str, object]]) -> str:
    lines = [f"repo: {repo_root}", f"conflicted files: {len(reports)}"]
    for report in reports:
        stages = ",".join(str(stage) for stage in report["stages"])
        lines.append(
            f"- {report['path']} | type={report['conflict_type']} | stages={stages} | hunks={report['marker_hunks']}"
        )
        if report["parse_error"]:
            lines.append(f"  parse-error: {report['parse_error']}")
    lines.append("use --file <path> for compact hunk details or --all for every file")
    return "\n".join(lines)


def render_detail_text(
    repo_root: Path,
    report: dict[str, object],
    max_lines: int,
) -> str:
    lines = [
        f"== {report['path']} ==",
        f"type: {report['conflict_type']}",
        f"stages: {', '.join(str(stage) for stage in report['stages'])}",
    ]
    parse_error = report["parse_error"]
    if parse_error:
        lines.append(f"parse-error: {parse_error}")

    hunks = report["hunks"]
    if hunks:
        lines.append(f"hunks: {len(hunks)}")
        for index, hunk in enumerate(hunks, start=1):
            ours = list(hunk["ours"])
            theirs = list(hunk["theirs"])
            diff = build_diff(
                ours,
                theirs,
                str(hunk["ours_label"]),
                str(hunk["theirs_label"]),
                max_lines,
            )
            lines.extend(
                [
                    "",
                    f"[hunk {index}] current lines {hunk['start_line']}-{hunk['end_line']}",
                    *section_lines("before", truncate_lines(list(hunk["before_context"]), max_lines)),
                    *section_lines(
                        f"ours ({hunk['ours_label']})",
                        truncate_lines(ours, max_lines),
                    ),
                ]
            )
            if hunk["base"] is not None:
                lines.extend(
                    section_lines(
                        f"base ({hunk['base_label'] or 'base'})",
                        truncate_lines(list(hunk["base"]), max_lines),
                    )
                )
            lines.extend(
                [
                    *section_lines(
                        f"theirs ({hunk['theirs_label']})",
                        truncate_lines(theirs, max_lines),
                    ),
                    *section_lines("ours vs theirs diff", diff),
                    *section_lines("after", truncate_lines(list(hunk["after_context"]), max_lines)),
                ]
            )
        return "\n".join(lines)

    preview = build_index_preview(repo_root, report, max_lines)
    lines.append("hunks: 0")
    lines.append("index preview:")
    lines.extend(section_lines("ours", preview["ours"]))
    lines.extend(section_lines("base", preview["base"]))
    lines.extend(section_lines("theirs", preview["theirs"]))
    if "ours_vs_theirs_diff" in preview:
        lines.extend(section_lines("ours vs theirs diff", preview["ours_vs_theirs_diff"]))
    return "\n".join(lines)


def render_json(
    repo_root: Path,
    reports: list[dict[str, object]],
    include_details: bool,
    max_lines: int,
) -> str:
    files: list[dict[str, object]] = []
    for report in reports:
        file_entry: dict[str, object] = {
            "path": report["path"],
            "conflict_type": report["conflict_type"],
            "stages": report["stages"],
            "marker_hunks": report["marker_hunks"],
            "parse_error": report["parse_error"],
        }
        if include_details:
            if report["hunks"]:
                file_entry["hunks"] = [
                    {
                        "start_line": hunk["start_line"],
                        "end_line": hunk["end_line"],
                        "before_context": truncate_lines(list(hunk["before_context"]), max_lines),
                        "ours_label": hunk["ours_label"],
                        "ours": truncate_lines(list(hunk["ours"]), max_lines),
                        "base_label": hunk["base_label"],
                        "base": truncate_lines(list(hunk["base"]), max_lines) if hunk["base"] else None,
                        "theirs_label": hunk["theirs_label"],
                        "theirs": truncate_lines(list(hunk["theirs"]), max_lines),
                        "after_context": truncate_lines(list(hunk["after_context"]), max_lines),
                        "ours_vs_theirs_diff": build_diff(
                            list(hunk["ours"]),
                            list(hunk["theirs"]),
                            str(hunk["ours_label"]),
                            str(hunk["theirs_label"]),
                            max_lines,
                        ),
                    }
                    for hunk in report["hunks"]
                ]
            else:
                file_entry["index_preview"] = build_index_preview(repo_root, report, max_lines)
        files.append(file_entry)

    return json.dumps(
        {
            "repo_root": str(repo_root),
            "conflicted_files": files,
        },
        indent=2,
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Summarize and extract compact merge-conflict context."
    )
    parser.add_argument("--repo", default=".", help="Path inside the target repository.")
    parser.add_argument(
        "--file",
        action="append",
        default=[],
        help="Conflicted file to inspect in detail. Repeat to inspect multiple files.",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Print detailed output for every conflicted file.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit JSON instead of text.",
    )
    parser.add_argument(
        "--context",
        type=int,
        default=2,
        help="Lines of surrounding context to include around each conflict hunk.",
    )
    parser.add_argument(
        "--max-lines",
        type=int,
        default=40,
        help="Maximum lines to print for each section before truncating.",
    )
    args = parser.parse_args()

    if args.all and args.file:
        parser.error("--all cannot be combined with --file")
    if args.context < 0:
        parser.error("--context must be non-negative")
    if args.max_lines <= 0:
        parser.error("--max-lines must be positive")

    try:
        repo_root = find_repo_root(Path(args.repo).resolve())
        entries = get_unmerged_entries(repo_root)
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    reports = [
        build_summary_report(repo_root, path, entries[path], args.context)
        for path in sorted(entries)
    ]

    if not reports:
        message = json.dumps({"repo_root": str(repo_root), "conflicted_files": []}, indent=2) if args.json else f"repo: {repo_root}\nconflicted files: 0"
        print(message)
        return 0

    if args.all:
        selected_reports = reports
    elif args.file:
        try:
            requested_paths = {normalize_requested_path(repo_root, path) for path in args.file}
        except RuntimeError as error:
            print(f"error: {error}", file=sys.stderr)
            return 2
        known_paths = {str(report["path"]) for report in reports}
        missing = sorted(requested_paths - known_paths)
        if missing:
            for path in missing:
                print(f"error: conflicted file not found: {path}", file=sys.stderr)
            return 2
        selected_reports = [report for report in reports if report["path"] in requested_paths]
    else:
        selected_reports = []

    if args.json:
        print(render_json(repo_root, selected_reports or reports, bool(selected_reports), args.max_lines))
        return 0

    if not selected_reports:
        print(render_summary_text(repo_root, reports))
        return 0

    print("\n\n".join(render_detail_text(repo_root, report, args.max_lines) for report in selected_reports))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
