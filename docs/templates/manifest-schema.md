<!--
  Copyright (C) 2025 Warp contributors
  SPDX-License-Identifier: AGPL-3.0-only

  This file is part of Warp.

  Warp is free software: you can redistribute it and/or modify it under the
  terms of the GNU Affero General Public License as published by the Free
  Software Foundation, version 3.

  Warp is distributed in the hope that it will be useful, but WITHOUT ANY
  WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
  FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for
  more details.

  You should have received a copy of the GNU Affero General Public License
  along with Warp. If not, see <https://www.gnu.org/licenses/>.
-->

# Project Template Manifest (`template.toml`) Schema

> Status: design spec for the `warp new <template>` scaffolding system.
> Tracking issues: PDX-62 (`warp new` command), PDX-57 (loader / substitution
> engine), PDX-63 (this document).

## 1. Overview

Every Warp project template is a Git repository (or local directory) whose
root contains a single `template.toml` file. That file is the contract
between the template author and Warp's scaffolder: it declares the
template's identity, the variables a user will be prompted for, the
post-init hooks to run after files are written, and which paths to skip
when copying.

Warp ships a curated set of starter templates that exercise this schema
end-to-end. Two are in active development:

- **`cloudflare-fullstack`** — TypeScript Workers + D1 + Vite frontend,
  prompting for project name, route domain, and which optional bindings
  (KV, R2, Vectorize) to wire up.
- **`apple-multiplatform`** — SwiftPM + Xcode workspace targeting iOS,
  macOS, and visionOS, prompting for bundle id, team id, and which
  platforms to include.

These templates live in separate repositories and are scaffolded in
parallel under `helm-templates/` — they are the canonical real-world
consumers of this schema. The loader (PDX-57) reads `template.toml` first,
prompts the user for missing variables, then materializes the rest of the
template tree into the target directory, applying substitution and running
post-init hooks. Anything not described in `template.toml` is treated as
plain content to be copied verbatim (modulo substitution, see §6).

## 2. Top-level fields

The manifest has two required top-level tables: `[template]` and
`[template.tags]`. Everything else (`[[variables]]`, `[[hooks.post_init]]`,
`[[ignore]]`) is optional but typed.

### 2.1 `[template]`

```toml
[template]
name             = "Cloudflare Fullstack"
slug             = "cloudflare-fullstack"
description      = "TypeScript Workers + D1 + Vite frontend"
version          = "0.3.1"
author           = "Warp"
homepage         = "https://github.com/warpdotdev/template-cloudflare-fullstack"
license          = "MIT"
min_warp_version = "0.2025.11.0"
```

| Field              | Type   | Required | Notes                                                                 |
| ------------------ | ------ | -------- | --------------------------------------------------------------------- |
| `name`             | string | yes      | Human-readable name shown in pickers.                                 |
| `slug`             | string | yes      | `[a-z0-9-]+`, used as the default project directory name.             |
| `description`      | string | yes      | One-line summary, < 120 chars.                                        |
| `version`          | string | yes      | Semver 2.0.0. Used for cache invalidation and update prompts.         |
| `author`           | string | yes      | Free-form, e.g. `"Warp"`, `"jane@example.com"`.                       |
| `homepage`         | string | no       | URL. Shown in `warp new --list`.                                      |
| `license`          | string | no       | SPDX identifier, e.g. `"MIT"`, `"AGPL-3.0-only"`.                     |
| `min_warp_version` | string | no       | Minimum Warp build that can scaffold this template. Semver.           |

### 2.2 `[template.tags]`

A flat array of lowercase keyword strings used for search and filtering in
`warp new --list` and the future template gallery.

```toml
[template]
# ...
tags = ["cloudflare", "fullstack", "typescript", "workers", "d1"]
```

Tags are advisory; the loader does not enforce a vocabulary, but the
gallery (out of scope, see §10) will.

## 3. `[[variables]]`

Each `[[variables]]` entry declares one prompt the user is asked when they
run `warp new <slug>`. Variables are answered top-down in the order they
appear in the manifest. Once collected, they are exposed to substitution
(§6), conditional inclusion (§7), and hooks (§4).

### 3.1 Common fields

| Field      | Type            | Required                  | Notes                                                                  |
| ---------- | --------------- | ------------------------- | ---------------------------------------------------------------------- |
| `name`     | string          | yes                       | `snake_case`, must match `^[a-z][a-z0-9_]*$`. Used in `{{ ... }}`.     |
| `prompt`   | string          | yes                       | Human-facing question. Should end without a colon — Warp adds one.     |
| `kind`     | enum string     | yes                       | `string` \| `boolean` \| `number` \| `choice` \| `path`.               |
| `default`  | (kind-typed)    | no                        | Pre-fills the prompt; type must match `kind`.                          |
| `validate` | string (regex)  | no (string/path only)     | Anchored RE2 regex. Failing input re-prompts.                          |
| `choices`  | array of string | yes when `kind = "choice"` | At least 2 entries. `default` (if set) must be a member.              |
| `required` | boolean         | no, default `true`        | If `false`, an empty answer becomes `null`/empty in templates.         |

### 3.2 Examples — one per `kind`

**`string`** — free-form text, optionally regex-validated:

```toml
[[variables]]
name     = "project_slug"
prompt   = "Project slug (lowercase, hyphenated)"
kind     = "string"
default  = "my-app"
validate = "^[a-z][a-z0-9-]{1,38}[a-z0-9]$"
```

**`boolean`** — yes/no, rendered as a `[Y/n]` prompt:

```toml
[[variables]]
name    = "install_deps"
prompt  = "Run `npm install` after scaffolding?"
kind    = "boolean"
default = true
```

**`number`** — integer or float; the loader infers from `default`:

```toml
[[variables]]
name    = "worker_cpu_ms"
prompt  = "CPU limit per request (ms, 10–30000)"
kind    = "number"
default = 50
```

**`choice`** — picker from a fixed list:

```toml
[[variables]]
name    = "frontend_framework"
prompt  = "Frontend framework"
kind    = "choice"
choices = ["react", "svelte", "solid", "none"]
default = "react"
```

**`path`** — filesystem path, relative to the scaffolded project root:

```toml
[[variables]]
name     = "workspace_dir"
prompt   = "Where should the Xcode workspace live?"
kind     = "path"
default  = "./Workspace"
validate = "^\\./[A-Za-z0-9_./-]+$"
required = false
```

## 4. `[[hooks.post_init]]`

After all files are materialized, the loader runs each `post_init` hook in
declared order, from the project root, with the resolved variable set
exported as `WARP_TPL_<NAME>` env vars.

| Field           | Type                | Required | Notes                                                                |
| --------------- | ------------------- | -------- | -------------------------------------------------------------------- |
| `name`          | string              | yes      | Display label streamed to the user.                                  |
| `command`       | string              | yes      | Single shell line, executed via `sh -c`.                             |
| `working_dir`   | string              | no       | Relative to project root. Defaults to `.`.                           |
| `env`           | table<string,string>| no       | Extra env vars merged on top of inherited environment.               |
| `condition`     | string (Tera expr)  | no       | Hook is skipped when expression evaluates falsy.                     |
| `fail_strategy` | enum string         | no       | `abort` (default) \| `warn` \| `ignore`.                             |

`condition` is evaluated as a Tera expression in a context populated with
the resolved variables. Truthiness rules match Tera's: non-empty string,
non-zero number, `true`, and non-empty arrays are truthy.

### 4.1 Examples

```toml
[[hooks.post_init]]
name      = "Install npm dependencies"
command   = "npm install"
condition = "{{ install_deps }}"

[[hooks.post_init]]
name    = "Initialize git"
command = "git init -b main && git add . && git commit -m 'chore: initial scaffold'"

[[hooks.post_init]]
name          = "Verify Cloudflare auth"
command       = "wrangler whoami"
fail_strategy = "warn"
condition     = "{{ frontend_framework != 'none' }}"

[[hooks.post_init]]
name        = "Resolve Swift packages"
command     = "xcodebuild -resolvePackageDependencies"
working_dir = "{{ workspace_dir }}"
env         = { DEVELOPER_DIR = "/Applications/Xcode.app/Contents/Developer" }
```

`fail_strategy` semantics:

- `abort` — non-zero exit aborts the scaffold; the project directory is
  left in place for inspection.
- `warn` — exit code is logged, scaffold continues.
- `ignore` — exit code is silently dropped.

## 5. `[[ignore]]`

A list of gitignore-style glob patterns that the loader skips when copying
the template tree. Patterns match relative to the template root and follow
gitignore semantics (leading `/` anchors, `**` for recursive, trailing `/`
for directories).

```toml
[[ignore]]
patterns = [
  ".git/",
  "node_modules/",
  ".DS_Store",
  "*.log",
  "/template.toml",          # the manifest itself is never copied
  "/.template-condition",    # condition sentinels (see §7)
  "**/.template-condition",
]
```

The loader implicitly adds `template.toml` and every `.template-condition`
file to the ignore set, but listing them explicitly is allowed and
encouraged for clarity.

## 6. Substitution syntax

Warp uses the [Tera](https://keats.github.io/tera/) template engine for
all substitution. Tera was picked over plain mustache for two reasons:
expressive enough for `condition`/`.template-condition` (filters,
comparisons, `and`/`or`) and well-supported in Rust.

Substitution applies to:

1. **File contents** of every text file copied from the template, unless
   the file matches `[[ignore]]`.
2. **Filenames and directory names** along the relative path from the
   template root to the destination.

A Tera expression is delimited by `{{` and `}}`. Inside, you may reference
any declared variable by `name`, apply Tera filters, and use literals.

Filename rename example — the file
`workspace/{{project_slug}}.code-workspace` in the template becomes
`workspace/my-app.code-workspace` in the scaffolded project when the user
answers `project_slug = "my-app"`.

Binary files (detected via content sniffing) are copied byte-for-byte and
are not substituted, even if their filename contains `{{ ... }}`.

## 7. Conditional inclusion

Any file or directory may be conditionally included by placing a sibling
file named `.template-condition` next to it. The contents of
`.template-condition` are a single Tera expression (whitespace and a
trailing newline are ignored). If the expression evaluates truthy, the
adjacent path is included; otherwise it is skipped entirely (recursively
for directories).

For files: the sentinel must be named `<filename>.template-condition`
(e.g. `wrangler.toml.template-condition`).

For directories: the sentinel lives **inside** the directory as
`.template-condition`.

Sentinel files are themselves never copied (see §5).

### 7.1 Example

```text
ios/
  .template-condition           # contents: {{ "ios" in platforms }}
  Project.swift
  Sources/...
visionos/
  .template-condition           # contents: {{ "visionos" in platforms }}
  Project.swift
wrangler.toml
wrangler.toml.template-condition  # contents: {{ frontend_framework != "none" }}
```

If the user picks `platforms = ["ios", "macos"]` and
`frontend_framework = "react"`, the `visionos/` directory is omitted while
`ios/` and `wrangler.toml` are kept.

## 8. Worked example: `minimal-rust-cli`

A complete `template.toml`, exercising every field:

```toml
[template]
name             = "Minimal Rust CLI"
slug             = "minimal-rust-cli"
description      = "A clap-based Rust CLI with anyhow + tracing wired up"
version          = "1.0.0"
author           = "Warp"
homepage         = "https://github.com/warpdotdev/template-minimal-rust-cli"
license          = "MIT"
min_warp_version = "0.2025.11.0"
tags             = ["rust", "cli", "starter"]

[[variables]]
name     = "crate_name"
prompt   = "Crate name"
kind     = "string"
default  = "my-cli"
validate = "^[a-z][a-z0-9_-]{0,63}$"

[[variables]]
name    = "use_tokio"
prompt  = "Add a tokio runtime?"
kind    = "boolean"
default = false

[[variables]]
name    = "edition"
prompt  = "Rust edition"
kind    = "choice"
choices = ["2021", "2024"]
default = "2024"

[[variables]]
name    = "msrv"
prompt  = "Minimum supported Rust version"
kind    = "number"
default = 1.80

[[variables]]
name     = "bin_path"
prompt   = "Where to put the bin entrypoint"
kind     = "path"
default  = "src/main.rs"
required = false

[[hooks.post_init]]
name    = "Initialize git"
command = "git init -b main && git add . && git commit -m 'chore: scaffold'"

[[hooks.post_init]]
name      = "Cargo fetch"
command   = "cargo fetch"
condition = "{{ use_tokio }}"
fail_strategy = "warn"

[[ignore]]
patterns = [".git/", "target/", "*.log"]
```

Corresponding directory layout:

```text
minimal-rust-cli/
  template.toml
  Cargo.toml                       # contains {{ crate_name }}, {{ edition }}, {{ msrv }}
  src/
    main.rs
  src/runtime_tokio.rs
  src/runtime_tokio.rs.template-condition   # {{ use_tokio }}
  README.md                        # contains {{ crate_name }}
  .gitignore
```

## 9. Validation rules

The loader (PDX-57) splits validation into two phases.

**Parse-time** — runs as soon as `template.toml` is read, before any user
prompt:

1. Manifest is syntactically valid TOML.
2. `[template]` is present and every required field (§2.1) is set.
3. `template.slug` matches `^[a-z0-9][a-z0-9-]{1,62}[a-z0-9]$`.
4. `template.version` and `template.min_warp_version` are valid semver.
5. Every `[[variables]].name` is unique and matches `^[a-z][a-z0-9_]*$`.
6. Each `[[variables]].kind` is one of the five enum members; `default`
   (if set) is the matching scalar type; `choices` is set iff
   `kind = "choice"` and contains the `default`.
7. `[[hooks.post_init]].fail_strategy`, when set, is one of `abort`,
   `warn`, `ignore`.

**Instantiate-time** — runs after prompts, before any file is written:

8. Every `{{ identifier }}` referenced in file contents, filenames, hook
   `condition`s, hook `command`s, and `.template-condition` sentinels
   resolves to a declared variable. Unknown identifiers fail the scaffold.
9. Each `validate` regex compiles and matches the answered string/path.
10. Resolved `path` variables stay within the project root after
    normalization (no `..` escape).

### 9.1 Explicit error cases the validator must surface

| Code                     | When it fires                                                          |
| ------------------------ | ---------------------------------------------------------------------- |
| `manifest.parse`         | `template.toml` is not valid TOML.                                     |
| `manifest.missing_field` | A required field in `[template]` or a variable entry is absent.        |
| `manifest.bad_semver`    | `version` or `min_warp_version` is not valid semver.                   |
| `variable.bad_kind`      | `kind` is not one of the five enum members.                            |
| `variable.choice_mismatch` | `default` is not in `choices`, or `choices` missing for `kind=choice`. |
| `variable.duplicate_name`| Two `[[variables]]` share a `name`.                                    |
| `template.unknown_var`   | A `{{ ident }}` references a name not declared in `[[variables]]`.     |
| `template.path_escape`   | A resolved `path` variable normalizes outside the project root.        |

The CLI surfaces these by `code`, with file/line spans where applicable.

## 10. Future / out of scope

The following are explicitly **not** part of this schema. They are tracked
separately and may grow new top-level tables in a later, backwards-
compatible revision:

- **Template repo signing.** Cosign/minisign signatures over the template
  tree are not yet specified. Today, trust is "did the user type the URL".
- **Remote template registries.** Discovery beyond `warp new <slug>`
  resolving against a hard-coded list of GitHub repos (and local paths)
  is not in scope; a hosted gallery is future work.
- **AI-generated template scaffolds.** Producing `template.toml` and the
  template tree from a natural-language description is not part of this
  schema; it would consume this schema, not extend it.
- **Cross-template composition / inheritance.** A template cannot today
  `extend` or `include` another template's manifest. If this is added it
  will be a new `[template.extends]` table.
- **Per-file substitution opt-out beyond binary detection.** A future
  `[[no_substitute]] patterns = [...]` table may be added if real
  templates need to ship literal `{{` sequences in text files.
