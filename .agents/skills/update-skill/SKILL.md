---
name: update-skill
description: Create or update skills by generating, editing, or refining SKILL.md files in this repository. Use when authoring new skills or revising the structure, frontmatter, or guidance for existing ones.
---

# update-skill

This guide provides instructions for creating or updating skills in this repository. It covers the required structure, frontmatter, and best practices for skills.

## Quick Start

Every skill is a directory containing a `SKILL.md` file with YAML frontmatter and markdown body:

```markdown
---
name: pdf-processing
description: Extract text and tables from PDF files, fill forms, merge documents.
---

# PDF Processing

## When to use this skill
Use this skill when the user needs to work with PDF files...

## How to extract text
1. Use pdfplumber for text extraction...

## How to fill forms
...
```

## Requirements

### Frontmatter (Required)

Every SKILL.md must start with YAML frontmatter containing:

- **name**: Kebab-case identifier (lowercase letters, numbers, hyphens only)
  - Example: `add-feature-flag`, `rust-unit-tests`, `update-skill`
- **description**: Specific description of what the skill does and when to use it
  - Must be non-empty
  - Should include key terms for skill discovery
  - Begin with an action verb to clearly state what the skill accomplishes (e.g., "Adds feature flags..." instead of "Helps with features..."), and immediately follow with a specific use case or context (e.g., "Use when working with feature flags")
  - Write in third person (e.g., "Adds feature flags..." not "I can help you add...")

### Writing Effective Descriptions

The description field is critical for skill discovery. Include both **what** the skill does and **when** to use it. Some good examples:

- `git-commit`: "Generate descriptive commit messages by analyzing git diffs. Use when the user asks for help writing commit messages or reviewing staged changes."
- `pdf-processing`: "Extract text and tables from PDF files, fill forms, merge documents. Use when working with PDF files or when the user mentions PDFs, forms, or document extraction."

Avoid vague descriptions like "Helps with code" or "Does development tasks". For more context, see "Description Best Practices" in [references/best-practices.md](references/best-practices.md).

### Skill Structure

Typical sections in Warp skills:

1. **Title and brief summary** – Clear title and a concise overview of the skill's purpose and primary use cases. Link to sections, reference files or related skills if useful
2. **Overview** - Context about the skill's purpose (optional but common), extends the summary with more details and context
3. **Main content** - Steps, usage instructions, or workflow guidance
4. **Best Practices** - Guidelines and recommendations (optional)
5. **Examples / Reference PRs** - Links to real examples (optional)

Keep the structure flexible based on the skill's needs. Simple skills can omit the optional sections.

### Validation

Optionally, use the [skills-ref](https://github.com/agentskills/agentskills/tree/main/skills-ref) reference library to validate your skills:

```bash
skills-ref validate ./my-skill
```

This checks that your SKILL.md frontmatter is valid and follows all naming conventions. If not installed, use the WebSearch tool to get context around this package.

### Main Content Best Practices

- For guidance on what qualifies as good main content, see "Conciseness Principles" in [references/best-practices.md](references/best-practices.md)
- When formatting code examples, see "Code Example Formatting" in [references/best-practices.md](references/best-practices.md).

### File Organization

- **Simple skills** (<=200 lines): Keep everything in SKILL.md
- **Complex skills** (>200 lines): Split detailed content into `references/` subdirectory
  - Reference files from SKILL.md with clear links
  - Example: "See [references/best-practices.md](references/best-practices.md) for detailed guidance"

## When to Split Content

Create `references/` subdirectory when:

- SKILL.md approaches 200+ lines
- Skill covers multiple domains or workflows that can be loaded independently
- Detailed reference material would clutter the main instructions

Keep only essential workflow and procedural instructions in SKILL.md. Move detailed reference material, schemas, and extensive examples to `references/` files.

## Examples from Existing Skills

For reference on structure and style:

- `.agents/skills/add-feature-flag/SKILL.md` - Multi-step workflow with clear sequential steps
- `.agents/skills/rust-unit-tests/SKILL.md` - Comprehensive guide with code examples and helper utilities
- `.agents/skills/remove-feature-flag/SKILL.md` - Cleanup workflow with search commands

## Best Practices

See [references/best-practices.md](references/best-practices.md) for detailed authoring guidance including:

- Progressive disclosure patterns
- Writing concise, effective instructions
- Code example formatting
- Common anti-patterns to avoid
