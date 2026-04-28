---
name: resolve-merge-conflicts
description: Resolve Git merge conflicts by extracting only unresolved paths, conflict hunks, and compact diffs instead of loading whole files into context. Use when a merge, rebase, cherry-pick, or stash pop stops on conflicts, when `git status` shows unmerged paths, or when files contain conflict markers.
---

# Resolve Merge Conflicts

## Overview

Resolve conflicts without opening full files unless the compact view is insufficient. Start with a summary, then inspect one conflicted file at a time.

## Workflow

1. Start with a summary.

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py
```

Use the summary to identify which files are unresolved, which index stages exist, and how many text hunks each file contains.

2. Drill into one file.

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py --file path/to/file
```

Prefer this over reading the whole file. The script prints only nearby context, the `ours` / `base` / `theirs` sections for each hunk, and a compact unified diff between `ours` and `theirs`.

3. Resolve the file.

- Take one side wholesale with `git checkout --ours -- path/to/file` or `git checkout --theirs -- path/to/file` when appropriate.
- Otherwise edit the file directly and remove the conflict markers.
- Read more of the file only if the compact output is not enough to decide the correct merge.

4. Re-check unresolved files.

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py
git diff --name-only --diff-filter=U
```

5. Validate the resolution.

- Ensure no unmerged paths remain.
- Ensure no `<<<<<<<`, `=======`, or `>>>>>>>` markers remain in the resolved files.
- Run targeted tests, builds, or linters for the touched area.
- Stage the resolved files.

## Commands

### Summary only

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py
```

### Detailed view for one file

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py --file path/to/file
```

### Detailed view for all conflicted files

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py --all
```

### JSON output

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py --file path/to/file --json
```

### Tune output size

```bash
python3 .agents/skills/resolve-merge-conflicts/scripts/extract_conflict_context.py \
  --file path/to/file \
  --context 3 \
  --max-lines 60
```

## Notes

- Use the script before opening conflicted files directly.
- Resolve one file at a time to keep context small.
- Expect marker-based text conflicts and index-only conflicts such as add/add or modify/delete. The script summarizes both, and it falls back to index-stage previews when the worktree file has no conflict markers.
