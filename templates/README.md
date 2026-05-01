# Helm `.mcp.json` Starter Template

## What this is

This directory ships a starter `.mcp.json` for Helm projects. Drop the file at the
root of your repo and Claude Code (and Symphony's `ClaudeCodeAgent`, which spawns
Claude Code subprocesses) will auto-load it on session start, exposing the four
MCP servers below to every agent that runs in that workspace.

To install:

```bash
cp templates/.mcp.json /path/to/your/project/.mcp.json
```

Token placeholders are written as `${VAR_NAME}` so they resolve from your shell
environment — in practice, from a Doppler config (see "Doppler integration"
below).

## Per-server explanation

### `github` — official GitHub MCP server

- **What it does:** lets the agent read and write GitHub on your behalf —
  list/clone repos, open and comment on issues, open and review pull requests,
  search code, manage workflow runs.
- **API key:** GitHub Personal Access Token (classic or fine-grained). Create at
  <https://github.com/settings/tokens>. Store in Doppler under the secret name
  `GITHUB_PERSONAL_ACCESS_TOKEN`.
- **Env var consumed:** `GITHUB_PERSONAL_ACCESS_TOKEN`.
- **Risk surface:** anything the token can do. A `repo`-scoped token can push to
  any repo you can push to, force-push, delete branches, merge PRs. Use a
  fine-grained token scoped to specific repos for production agents.

### `brave` — Brave Search MCP

- **What it does:** web research. The agent can query Brave Search and read back
  result snippets, useful for grounding answers about recent docs, package
  versions, and open-source landscape questions.
- **API key:** free Brave Search API key (2,000 queries/month on the free tier).
  Sign up at <https://api.search.brave.com/>. Store in Doppler as `BRAVE_API_KEY`.
- **Env var consumed:** `BRAVE_API_KEY`.
- **Risk surface:** read-only. The worst case is exhausting your query quota or
  leaking the search query string to Brave. No write access to anything.

### `cloudflare` — Cloudflare's remote MCP (observability)

- **What it does:** queries Cloudflare's Workers/D1/R2/AI Gateway control plane.
  This template wires up the **observability** remote MCP endpoint
  (`observability.mcp.cloudflare.com/sse`); swap or add other Cloudflare remote
  MCP endpoints (`bindings`, `radar`, `browser-rendering`, etc.) as needed.
  See <https://developers.cloudflare.com/agents/model-context-protocol/mcp-servers-for-cloudflare/>.
- **API key:** Cloudflare API token with the scopes you want the agent to have
  (Workers Scripts:Read, D1:Read, etc.). Create at
  <https://dash.cloudflare.com/profile/api-tokens>. Store in Doppler as
  `CLOUDFLARE_API_TOKEN`. Also set `CLOUDFLARE_ACCOUNT_ID`.
- **Env vars consumed:** `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`.
- **Risk surface:** scoped to the token. A token with `Edit` permissions can
  deploy Workers, drop D1 tables, mutate R2 buckets, change AI Gateway routes.
  For agent use, prefer read-only tokens unless the agent's job requires writes.

### `doppler` — Helm's custom Doppler MCP (metadata-only)

- **What it does:** lets the agent introspect Doppler configs (project, config,
  secret names) without exposing secret *values*. This is the metadata-only
  server defined in Helm's PDX-77 spec — agents can answer "which secrets does
  this project depend on?" and "is `STRIPE_API_KEY` set in the production
  config?" but cannot read secret values.
- **API key:** a Doppler service token, scoped to the project the agent is
  working in. Create at <https://dashboard.doppler.com/>. Store in Doppler as
  `DOPPLER_TOKEN` (in a *different* config than the one being introspected, so
  the token isn't exposed to itself).
- **Env var consumed:** `DOPPLER_TOKEN`.
- **Risk surface:** metadata only by design. The MCP server's tool surface
  enforces that secret values never leave Doppler. Verify the binary's mode
  flag stays `--mode metadata-only` for any agent you don't fully trust.

> TODO: verify `doppler-mcp` install command. The metadata-only Doppler MCP
> server is custom to Helm (per PDX-77). Update the `command` and `args` in
> `.mcp.json` once the binary publishing path is finalized. If it ships as an
> npm package, switch to `"command": "npx"` with `"args": ["-y",
> "@helm/doppler-mcp", "--mode", "metadata-only"]`. If it ships via Homebrew or
> a release artifact, document the install path here.

## Doppler integration

All four servers expect their secrets in environment variables. To resolve them
from a Doppler config rather than from your shell at launch time, run Claude
Code (or Symphony) under `doppler run`:

```bash
# from your project root, after copying .mcp.json into place
doppler run --project helm --config dev -- claude
```

Or, for Symphony agents:

```bash
doppler run --project helm --config dev -- symphony agent run claude-code ...
```

`doppler run` injects each Doppler secret as a shell env var, then exec's the
child process. The child (Claude Code, then each MCP server it spawns) inherits
the env, and the `${VAR_NAME}` references in `.mcp.json` are resolved by Claude
Code's config loader at startup.

To list which secrets you need set in Doppler for this template:

```
GITHUB_PERSONAL_ACCESS_TOKEN
BRAVE_API_KEY
CLOUDFLARE_API_TOKEN
CLOUDFLARE_ACCOUNT_ID
DOPPLER_TOKEN
```

## Adding more servers

The MCP spec is at <https://modelcontextprotocol.io/>. To add a new server,
append an entry to the `mcpServers` object — one-line shape:

```json
"my-server": { "command": "npx", "args": ["-y", "@scope/my-mcp-server"], "env": { "MY_API_KEY": "${MY_API_KEY}" } }
```

Public registry of available servers: <https://github.com/modelcontextprotocol/servers>.

## Security notes

Agents loaded with this `.mcp.json` get powerful capabilities: write to GitHub,
execute Cloudflare API calls, search the public web, introspect Doppler.
**Treat this template as a relatively-trusted-environment default**, not a
hardened production config. Before pointing an autonomous agent at it:

- Audit each server's tool surface (`list_tools` against each MCP server) and
  decide which tools should be allow-listed. Claude Code supports per-server
  tool allow-lists in `settings.json`.
- Use **fine-grained, least-privilege tokens** for GitHub and Cloudflare. A
  read-only token is almost always enough for an investigator agent; a writer
  agent should be scoped to specific repos and specific Cloudflare resources.
- Keep the Doppler MCP in `--mode metadata-only` unless you have a hard
  requirement for value reads, and even then prefer `doppler run --` injection
  so values never traverse the agent's tool path.
- Rotate tokens on a schedule. Doppler makes this easy; do it.
- Never check a populated `.mcp.json` (one with real values, not `${VAR}`
  placeholders) into git. `.gitignore` it if your team's workflow needs a
  per-developer override file.
