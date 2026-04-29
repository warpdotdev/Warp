# Tech Spec: Support for Agent Client Protocol (ACP)

**Issue:** [warpdotdev/warp#7326](https://github.com/warpdotdev/warp/issues/7326)
**Product spec:** `specs/GH7326/product.md`

## Context

Warp currently supports third-party coding agents through a `ThirdPartyHarness`
trait defined in `app/src/ai/agent_sdk/driver/harness/mod.rs`. Each supported
agent (Claude Code, Gemini) implements this trait in its own file and is
registered as a variant of the `Harness` enum in `crates/warp_cli/src/agent.rs`
and `CLIAgent` enum in `app/src/terminal/cli_agent.rs`.

ACP (Agent Client Protocol) is a standard JSON-RPC-over-stdio protocol for
agent-editor communication. Rather than adding a new per-agent harness for every
ACP-compatible CLI, a single `AcpHarness` implementation covers all of them.
The user supplies the command (e.g. `opencode`, `kimi`) and Warp handles the
protocol layer generically.

### Relevant files

- `crates/warp_cli/src/agent.rs:123` — `pub enum Harness` — needs new `Acp` variant
- `app/src/terminal/cli_agent.rs:108` — `pub enum CLIAgent` — needs new `Acp` variant with user-configurable command
- `app/src/ai/agent_sdk/driver/harness/mod.rs:57` — `ThirdPartyHarness` trait definition — the contract `AcpHarness` must implement
- `app/src/ai/agent_sdk/driver/harness/gemini.rs` — `GeminiHarness` — closest structural analog to `AcpHarness`
- `app/src/ai/agent_sdk/driver/harness/mod.rs:133` — `HarnessKind` enum — may need `Acp` variant
- `app/src/terminal/view/ambient_agent/harness_selector.rs:61` — harness dropdown — needs ACP entry
- `app/src/server/server_api/harness_support.rs` — server-side harness registration — needs ACP format slug

## Proposed changes

### 1. `crates/warp_cli/src/agent.rs`

Add a new `Acp` variant to `pub enum Harness`:

```rust
/// Delegate to any ACP-compatible agent CLI (user-configured command).
#[value(name = "acp")]
Acp,
```

Place between `OpenCode` and the `Unknown` fallback variant.

### 2. `app/src/terminal/cli_agent.rs`

Add `Acp` to `pub enum CLIAgent`. Unlike other variants whose commands are
hardcoded, `CLIAgent::Acp` carries the user-supplied command string:

```rust
/// An ACP-compatible agent with a user-configured command.
Acp(String),
```

Update `command_prefix()`, `display_name()`, and `icon()` match arms:

```rust
// command_prefix
CLIAgent::Acp(cmd) => cmd.as_str(),

// display_name
CLIAgent::Acp(cmd) => cmd.as_str(),  // show the actual command as the name

// icon — no dedicated icon yet; fall through to None
CLIAgent::Acp(_) => None,
```

### 3. New file: `app/src/ai/agent_sdk/driver/harness/acp.rs`

Create `AcpHarness` implementing `ThirdPartyHarness`. The struct carries the
user-configured command and optional args:

```rust
pub(crate) struct AcpHarness {
    command: String,
    args: Vec<String>,
}
```

Key `ThirdPartyHarness` method implementations:

- `harness()` → `Harness::Acp`
- `cli_agent()` → `CLIAgent::Acp(self.command.clone())`
- `validate()` → call `validate_cli_installed(&self.command, self.install_docs_url())`
- `install_docs_url()` → `None` (unknown for arbitrary agents; follow-up can add a registry)
- `prepare_environment_config()` → write ACP session config if needed; for v1 this may be a no-op or minimal JSON config file with MCP server list
- `build_command()` → spawn `self.command` with `self.args`, plus the stdio transport flags required by ACP (`--acp` or similar, depending on agent)

The ACP protocol handshake (JSON-RPC `initialize` / `initialized` exchange over
stdio) is handled in a new `run_acp_session()` function within this file, called
from `run()`. The initialize request must include:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "clientInfo": { "name": "warp", "version": "<warp_version>" },
    "capabilities": {}
  }
}
```

Warp then reads the `InitializeResult` from the agent's stdout, sends
`initialized`, and transitions to the active session loop.

**Timeout:** If no `InitializeResult` arrives within 10 seconds, `run()` returns
`AgentDriverError::HarnessConfigSetupFailed` with a descriptive message.

**MCP passthrough:** After initialization, if Warp has configured MCP servers,
they are passed to the agent via the ACP session configuration per the protocol
spec before the first prompt is sent.

### 4. `app/src/ai/agent_sdk/driver/harness/mod.rs`

- Add `mod acp;` and `pub(crate) use acp::AcpHarness;`
- Add `Acp` variant to `HarnessKind` (line 133) if this enum is exhaustively matched anywhere
- Register `AcpHarness` in the harness dispatch logic alongside `ClaudeHarness` and `GeminiHarness`

### 5. `app/src/terminal/view/ambient_agent/harness_selector.rs`

Add `Harness::Acp` to the harness selector dropdown. For user-configured ACP
agents, each appears as a separate entry using its display name (the command
string). This requires reading the list of configured ACP agents from settings
and generating one selector entry per agent.

### 6. `app/src/server/server_api/harness_support.rs`

Add an `"acp"` format slug for ACP conversations, used when creating a
conversation record on the server. This follows the pattern of `"gemini_cli"`
for Gemini.

## Data flow

```
User selects ACP agent + sends prompt
  → AcpHarness::validate() checks command on PATH
  → AcpHarness::build_command() spawns process over stdio
  → run_acp_session() sends initialize, awaits InitializeResult (10s timeout)
  → sends initialized notification
  → passes MCP servers to session config (if any)
  → enters prompt/response loop:
      Warp sends prompt as ACP session/message
      Agent streams response chunks
      Warp renders chunks as conversation blocks
      File edits → diff view
      Tool calls → tool call blocks
  → on agent exit or crash → conversation marked ended
```

## Tradeoffs

- **Per-agent command vs. shared variant:** Storing the command inside
  `CLIAgent::Acp(String)` is a departure from the hardcoded variants pattern.
  This makes serialization slightly more complex but avoids adding a new enum
  variant for every ACP agent. If a specific ACP agent later warrants a custom
  harness (e.g. for session resumption), it can graduate to its own variant.
- **Stdio-only in v1:** Excludes ACP agents that communicate over HTTP. This
  covers the majority of current ACP agents (opencode, Kimi, Gemini, Codex all
  support stdio) and avoids auth complexity for v1.

## Testing and validation

Invariant-to-test mapping (from `product.md` success criteria):

1. **Unit test in `acp.rs`:** `AcpHarness::validate()` returns `Ok(())` for a
   command on PATH and `Err(AgentDriverError::CliNotInstalled)` for a missing
   command.
2. **Unit test:** `run_acp_session()` returns a timeout error if the mock agent
   process does not send `InitializeResult` within the deadline.
3. **Integration test under `crates/integration/`:** Spawn a minimal ACP echo
   agent (a small test binary that implements initialize/initialized and
   returns a static response), send a prompt, and assert a conversation block
   is produced.
4. **Manual:** Configure `opencode` as an ACP agent, send a prompt, confirm
   response renders as a Warp conversation block.
5. **Manual:** Confirm `opencode` appears in the harness selector alongside
   Claude Code, Codex, and Gemini.
6. **Regression:** `cargo nextest run` passes across existing harness tests
   (`local_harness_launch_tests.rs`, `cli_agent_tests.rs`,
   `mod_test.rs` in `driver/harness/`).

## Risks and mitigations

- **ACP spec churn:** The ACP spec is still evolving. Mitigation: pin to the
  stable `initialize`/`initialized`/`session/message` core and treat unknown
  fields as ignored (forward-compatible deserialization with `#[serde(flatten)]`
  or `deny_unknown_fields = false`).
- **CLIAgent enum serialization:** Adding `Acp(String)` changes the shape of a
  serialized enum. Mitigation: verify that `CLIAgent` is not persisted to disk
  or sent over the network in a way that would break existing stored data; if it
  is, add a migration.
- **No icon for ACP agents:** The harness selector will show ACP agents without
  a logo. Mitigation: acceptable for v1; a generic "agent" icon can be added as
  a follow-up.

## Follow-ups

- Session resumption for ACP agents (requires ACP `loadSession` capability).
- A curated registry of known ACP agents with pre-filled commands and install URLs.
- Warp as ACP server (exposing Warp Agent to external IDEs like Zed).
- Per-agent icons for known ACP CLIs.
- HTTP transport support for remote ACP agents.
