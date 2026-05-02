# WARP.md Project-Overlay Schema

`WARP.md` is a Markdown file that injects project-specific context into every
Helm agent prompt. The Helm prompt composer reads it as the third layer of a
three-layer stack:

```
Base prompt  →  Role overlay  →  Project overlay (WARP.md)
```

When the AI system in the Warp client scans a repository it also collects every
`WARP.md` (and its fallback `AGENTS.md`) found within three directory levels of
the project root. Each file is applied when an agent is working on files inside
that directory.

---

## File naming and priority

| Filename    | Priority | Notes                                           |
|-------------|----------|-------------------------------------------------|
| `WARP.md`   | 1 (high) | Preferred; matched case-insensitively           |
| `AGENTS.md` | 2 (low)  | Fallback when no `WARP.md` exists in a dir      |

At most one of the two is active per directory. If both exist, `WARP.md` wins.

---

## Placement

```
<repo-root>/
├── WARP.md                         ← root overlay (widest scope)
├── crates/
│   └── some_crate/
│       └── src/
│           └── some_module/
│               └── WARP.md         ← scoped to this subtree (depth ≤ 3)
└── app/
    └── some_feature/
        └── WARP.md                 ← scoped to this subtree (depth ≤ 3)
```

- The scan descends at most **3 levels** from the project root.
- A sub-directory overlay is *active* only when the agent is editing a file
  that lives under that directory.
- Symlinks to WARP.md files are followed; symlinks to directories are not.

---

## Universal rules (all WARP.md files)

These constraints apply to every overlay regardless of depth.

### Structure

| Rule | Description |
|------|-------------|
| **H1 present** | The file must open with exactly one `#`-level heading. |
| **H1 is first content line** | No other text may appear before the H1. Blank lines before H1 are acceptable. |
| **H1 is unique** | Exactly one H1 heading is allowed. Additional `#` headings are an error. |
| **At least one H2** | The file must contain at least one `##`-level section heading. |
| **No duplicate H2 names** | Section names are compared case-insensitively; duplicates are an error. |
| **Code fences closed** | Every opening triple-backtick (` ``` `) must have a matching closing ` ``` `. An unclosed fence causes the rest of the file to be treated as a code block by the model. |
| **Non-empty** | A zero-byte or whitespace-only file is invalid. |

### Size guidance

| Threshold | Severity | Rationale |
|-----------|----------|-----------|
| ≤ 10 000 bytes | OK | Comfortable project-overlay budget |
| 10 001 – 20 000 bytes | Warning | Token cost; consider splitting |
| > 20 000 bytes | Error | Exceeds safe prompt injection budget |

### Heading depth

H4 (`####`) and deeper headings are a **warning**. Current model context windows
parse them inconsistently when the file is injected mid-prompt. Use H3 (`###`)
as the maximum nesting level.

---

## Root overlay rules (`<repo-root>/WARP.md`)

The root overlay is the primary project context document. Additional constraints
apply beyond the universal rules above.

### Required H1 title

```markdown
# WARP.md
```

The H1 must be the literal string `# WARP.md`. Any other title is an error.

### Required sections

Both of the following H2 sections must be present (case-insensitive match):

| Section | Purpose |
|---------|---------|
| `## Development Commands` | Build, test, lint, and run commands that agents use to verify their changes. |
| `## Architecture Overview` | High-level description of the codebase structure and key design patterns. |

### Development Commands subsections

The `## Development Commands` section should contain at least one H3 subsection
documenting commands for a specific workflow phase (e.g., `### Build and Run`,
`### Testing`, `### Linting and Formatting`). A section with no H3 children is
a warning.

### Recommended (non-required) sections

These sections appear in the reference root overlay and are strongly recommended:

- `## Architecture Overview` → `### Key Components`
- `## Architecture Overview` → `### Development Guidelines`
- Subsection covering coding-style preferences
- Subsection covering testing conventions

---

## Sub-directory overlay rules

Sub-directory overlays are scoped to a specific component, module, or
example. The requirements are lighter.

| Rule | Notes |
|------|-------|
| H1 may be any descriptive title | Does not need to be `# WARP.md` |
| At least one H2 section | Universal rule applies |
| Content scoped to that directory | Don't repeat root-level guidance |

### Good examples of sub-directory overlay content

- Debugging guides for the component
- Invariants the component enforces that are non-obvious to a reader
- Known pitfalls (e.g., deadlock hazards, layout constraints)
- Component-specific visual testing protocol

---

## Content guidelines

These are authoring best practices enforced as warnings by the validator.

1. **Write for an AI reader that has never seen this repo.** Be explicit; don't
   rely on external documentation linked by URL (agents may not fetch it).
2. **Command examples must be runnable as-is.** Agents copy commands verbatim.
   Placeholder tokens like `<project>` break automation.
3. **One topic per H2.** Dense multi-topic sections degrade instruction
   following. If a section exceeds ~80 lines, split it.
4. **No duplicate content with other layers.** The base prompt already covers
   the AI Gateway routing and AGPL licensing rules. Do not restate them.

---

## Section ordering convention (root overlay)

While section order is not validated, this order is conventional and should be
followed in new or refactored root overlays:

1. `## Development Commands`
2. `## Architecture Overview`
3. Additional topic sections (e.g., `## Feature Flags`, `## Database`)

---

## Complete BNF-style grammar (informative)

```
warp-md         ::= blank* h1 blank* section+
h1              ::= "# " title newline
section         ::= h2-heading blank* section-body
h2-heading      ::= "## " heading-text newline
section-body    ::= (h3-section | paragraph | code-block | blank)*
h3-section      ::= "### " heading-text newline (paragraph | code-block | blank)*
paragraph       ::= (non-heading-line newline)+
code-block      ::= "```" lang? newline code-line* "```" newline
blank           ::= newline
```

All other CommonMark constructs (lists, blockquotes, tables, inline code,
emphasis) are permitted anywhere body content is allowed.

---

## Quick reference: error and warning codes

These are the codes emitted by `script/validate-warp-md`.

| Code | Severity | Condition |
|------|----------|-----------|
| E001 | Error    | File is empty or contains only whitespace |
| E002 | Error    | File is not valid UTF-8 |
| E003 | Error    | No H1 heading found |
| E004 | Error    | H1 is not the first content line |
| E005 | Error    | Multiple H1 headings |
| E006 | Error    | No H2 sections |
| E007 | Error    | Duplicate H2 section name (case-insensitive) |
| E008 | Error    | Unclosed code fence |
| E009 | Error    | (root) H1 is not `# WARP.md` |
| E010 | Error    | (root) Missing `## Development Commands` section |
| E011 | Error    | (root) Missing `## Architecture Overview` section |
| W001 | Warning  | File size exceeds 10 000 bytes |
| W002 | Warning  | File size exceeds 20 000 bytes (promoted to error by `--strict`) |
| W003 | Warning  | H4+ heading levels present |
| W004 | Warning  | Empty section (heading with no body before next heading) |
| W005 | Warning  | (root) `## Development Commands` has no H3 subsections |
