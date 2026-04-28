# Best Practices for Warp Skills

Detailed authoring guidance for creating effective skills in `.agents/skills/`.

## Progressive Disclosure

Skills use a loading system to manage context efficiently:

1. **Metadata (name + description)** - Always loaded at startup
2. **SKILL.md body** - Loaded when skill triggers
3. **Reference files** - Loaded only when needed

### When to Use References

Keep SKILL.md under 150-200 lines. When content grows beyond this:

**Pattern 1: High-level guide with references**

SKILL.md contains the core workflow and points to detailed references:

```markdown
## Advanced Features

- **Detailed configuration**: See [references/config.md](references/config.md)
- **API reference**: See [references/api.md](references/api.md)
- **Examples**: See [references/examples.md](references/examples.md)
```

**Pattern 2: Domain-specific organization**

For skills with multiple independent domains, organize by domain:

```
skill-name/
├── SKILL.md (overview and navigation)
└── references/
    ├── domain-a.md
    ├── domain-b.md
    └── domain-c.md
```

When the user works with domain-a, the agent only loads domain-a.md, not the others.

**Pattern 3: Conditional details**

Show basic content inline, link to advanced content:

```markdown
## Basic Usage

[Core instructions here]

**For advanced configuration**: See [references/advanced.md](references/advanced.md)
```

### Important Guidelines

- **Keep references one level deep** - All reference files should link directly from SKILL.md
- **Avoid nested references** - Don't create references that reference other files
- **Add table of contents** - For reference files >100 lines, include TOC at the top

## Writing Effective Descriptions

The description field enables skill discovery. the agent uses it to decide when to load the skill.

### Description Best Practices

1. **Be specific and include key terms**
   - Good: "Add a new feature flag to gate code changes in the Warp codebase."
   - Avoid: "Helps with features."

2. **Include both what and when**
   - What the skill does: "Write, improve, and run Rust unit tests"
   - When to use it: "in the warp Rust codebase"

3. **Write in third person**
   - Good: "Adds feature flags to gate code changes"
   - Avoid: "I can help you add feature flags"
   - Avoid: "You can use this to add feature flags"

4. **Include trigger terms**
   - Mention specific files, commands, or concepts
   - Example: "Use when working with PDF files, forms, or document extraction"

## Conciseness Principles

Context window is shared across all skills, conversation history, and the system prompt. Every token matters.

### Default Assumption: Agent is Already Smart

Only add context the agent doesn't already have. Challenge each piece:

- "Does the agent really need this explanation?"
- "Can I assume the agent knows this?"
- "Does this paragraph justify its token cost?"

**Good (concise):**

```markdown
## Extract PDF text

Use pdfplumber for text extraction:

\`\`\`python
import pdfplumber

with pdfplumber.open("file.pdf") as pdf:
    text = pdf.pages[0].extract_text()
\`\`\`
```

**Bad (verbose):**

```markdown
## Extract PDF text

PDF (Portable Document Format) files are a common file format that contains
text, images, and other content. To extract text from a PDF, you'll need to
use a library. There are many libraries available for PDF processing, but we
recommend pdfplumber because it's easy to use and handles most cases well.
First, you'll need to install it using pip. Then you can use the code below...
```

The concise version assumes the agent knows what PDFs are and how libraries work.

## Code Example Formatting

### Syntax Highlighting

Always specify the language for code blocks:

```rust
pub fn example() {
    println!("Always specify language");
}
```

```bash
cargo nextest run --workspace
```

### Example Structure

For workflow-based skills, show before/after or step-by-step:

```markdown
### Before:
\`\`\`rust
if FeatureFlag::YourFeature.is_enabled() {
    // new behavior
} else {
    // old behavior (dead code)
}
\`\`\`

### After:
\`\`\`rust
// new behavior (unconditionally enabled)
\`\`\`
```

### Inline Commands

For shell commands, show the complete command with flags:

```bash
cargo clippy --workspace --all-targets --all-features --tests -- -D warnings
```

Explain non-obvious flags if necessary, but prefer self-documenting commands.

## Workflows vs Simple Instructions

### When to Use Workflows

Use numbered steps for multi-step processes where order matters:

```markdown
## Workflow

1. Analyze the form structure
2. Create field mapping
3. Validate mapping
4. Fill the form
5. Verify output
```

Include a checklist for complex workflows:

```markdown
Copy this checklist and track progress:

\`\`\`
Task Progress:
- [ ] Step 1: Analyze form
- [ ] Step 2: Create mapping
- [ ] Step 3: Validate
- [ ] Step 4: Fill form
- [ ] Step 5: Verify
\`\`\`
```

### When to Use Simple Instructions

For straightforward tasks, skip the workflow structure:

```markdown
## Adding a Feature Flag

Add the feature to `app/Cargo.toml`:

\`\`\`toml
[features]
your_feature_name = []
\`\`\`

Then gate code with the runtime check:

\`\`\`rust
if FeatureFlag::YourFeatureName.is_enabled() {
    // feature-gated behavior
}
\`\`\`
```

## Common Anti-Patterns

### ❌ Windows-Style Paths

Always use forward slashes:

- ✓ Good: `scripts/helper.py`, `references/guide.md`
- ✗ Avoid: `scripts\helper.py`, `references\guide.md`

### ❌ Vague Descriptions

Be specific:

- ✗ Avoid: "Helps with documents"
- ✓ Good: "Extract text and tables from PDF files"

### ❌ Too Many Options

Don't present multiple approaches unless necessary:

- ✗ Avoid: "You can use pypdf, or pdfplumber, or PyMuPDF, or..."
- ✓ Good: "Use pdfplumber for text extraction. For scanned PDFs requiring OCR, use pdf2image with pytesseract instead."

### ❌ Time-Sensitive Information

Don't include dates or version-specific guidance:

- ✗ Avoid: "If you're doing this before August 2025, use the old API."
- ✓ Good: Use a "Current method" and "Old patterns" section with deprecation notes

### ❌ Inconsistent Terminology

Choose one term and use it throughout:

- ✗ Avoid: Mix "API endpoint", "URL", "API route", "path"
- ✓ Good: Always "API endpoint"

### ❌ Explaining the Obvious

Skip explanations for concepts the agent already knows:

- ✗ Avoid: "Git is a version control system that tracks changes in files..."
- ✓ Good: "Use `git --no-pager diff` to see changes without pagination"

### ❌ Over-Structuring Simple Skills

Not every skill needs an Overview, Best Practices, and Examples section. Use only what adds value:

- Simple skills: Title + instructions
- Medium skills: Title + Overview + instructions
- Complex skills: Full structure with multiple sections

## Naming Conventions

Use consistent naming patterns for skills:

**Recommended: Gerund form (verb + -ing)**

- `processing-pdfs`
- `analyzing-spreadsheets`
- `managing-databases`
- `testing-code`

**Acceptable alternatives:**

- Noun phrases: `pdf-processing`, `spreadsheet-analysis`
- Action-oriented: `process-pdfs`, `analyze-spreadsheets`

**Avoid:**

- Vague names: `helper`, `utils`, `tools`
- Overly generic: `documents`, `data`, `files`

Consistent naming makes skills easier to reference, understand at a glance, and organize.

## Skill Iteration

Skills improve through usage. When updating a skill:

1. **Observe usage** - Note where the agent struggles or succeeds
2. **Identify gaps** - What information was missing or unclear?
3. **Update targeted sections** - Fix specific issues without over-explaining
4. **Test changes** - Use the skill on similar tasks to verify improvements

Keep iterations focused. Don't add content preemptively—only add what's proven necessary through real usage.
