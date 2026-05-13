# WARPER-005: OpenRouter skill and follow-up context fidelity - Tech spec

Companion to `PRODUCT.md` in this directory; refer there for user-visible behavior.

## Context

- Historical diff assessment:
  - `46ac60a warper: amputate hosted agent runtime` removed the hosted request path from `app/src/ai/agent/api/impl.rs` and routed all generation through OpenRouter. That was correct for local-first Warper, but it left OpenRouter responsible for behavior that used to be provided by the hosted agent service.
  - `6c4ab90 refactor: delete hosted AI request conversion` deleted `app/src/ai/agent/api/convert_to.rs`. That file had first-class conversion for `AIAgentInput::InvokeSkill` into `api::request::input::Type::InvokeSkill`, including skill content/reference, user query, context, and attachments.
  - `7717ab5 refactor: delete hosted AI tool list helpers` deleted the hosted tool negotiation helpers from `app/src/ai/agent/api/impl.rs`. Before deletion, local sessions advertised useful local tools including `ReadFiles`, `ApplyFileDiffs`, `SearchCodebase`, `Grep`, `FileGlob`, `ReadSkill`, MCP tools, shell command tools, and `AskUserQuestion`.
  - `28661cb Persist local OpenRouter conversation turns` added OpenRouter task replay, but persisted only `AIAgentInput::UserQuery` inputs into task messages. `InvokeSkill` still disappears from follow-up history.
  - `ce7ca15 fix: include attached context in openrouter prompts` added local context serialization for OpenRouter, but still ignores `AIAgentContext::Skills` and still lets `InvokeSkill` fall back to display text.
  - Conclusion: WARPER-005 should not restore hosted generation or hosted services. It should restore the local pieces that used to make the agent useful: structured skill invocation, local tool schemas mapped to existing client-side executors, and faithful task/history replay.
- `app/src/terminal/input/slash_command_model.rs:230` parses slash-prefixed input. It checks static slash commands first, then resolves skills through `SkillManager` and returns `SlashCommandEntryState::SkillCommand`.
- `app/src/terminal/input.rs:3999` handles detected skill commands through `execute_skill_command`. It resolves the `SkillReference`, clears the input, optionally enters Agent View, and sends `SlashCommandRequest::InvokeSkill`.
- `app/src/ai/blocklist/controller/slash_command.rs:208` converts `SlashCommandRequest::InvokeSkill` into `AIAgentInput::InvokeSkill { skill, user_query, context }`. At this point the parsed `ParsedSkill` contains the skill name, description, content, provider, and scope.
- `app/src/ai/blocklist/controller.rs:1738` sends every request through `send_request_input`, computes active tasks for the existing conversation, and builds `api::RequestParams`.
- `app/src/ai/agent/api/openrouter.rs:403` builds OpenRouter chat messages. It includes a generic system prompt, prior task messages converted by `openrouter_messages_from_tasks`, and the current request converted by `input_to_prompt_text`.
- `app/src/ai/agent/api/openrouter.rs:439` only has explicit prompt conversion for `UserQuery`, `ActionResult`, `AutoCodeDiffQuery`, `SummarizeConversation`, and `PassiveSuggestionResult`. Other inputs, including `InvokeSkill`, fall through to `input.user_query()`.
- `app/src/ai/agent/mod.rs:2538` renders `AIAgentInput::InvokeSkill` as display text like `/update-tab-config <argument>`. It does not include `skill.content`.
- `app/src/ai/agent/api/openrouter.rs:513` explicitly ignores `AIAgentContext::Skills`. The OpenRouter request therefore does not receive the available skill list or descriptions through context conversion.
- `app/src/ai/agent/api/openrouter.rs:289` exposes only one OpenRouter function tool, `run_shell_command`. The local action model still has executors for richer local tools, but OpenRouter cannot request them because it never receives schemas and `openrouter_tool_call_to_message` only maps shell command calls.
- `app/src/ai/agent/api/convert_from.rs:620` still converts many `warp_multi_agent_api::message::ToolCall` variants into local `AIAgentAction`s, including `ReadFiles`, `SearchCodebase`, `Grep`, `FileGlob`, `ApplyFileDiffs`, `ReadSkill`, MCP tools, and `AskUserQuestion`. This means OpenRouter can regain local-first tool capability by emitting the same task-message tool calls the existing client executors already understand.
- `app/src/ai/blocklist/action_model/execute/*` still contains the local executors for these tools. For example, `read_files.rs`, `grep.rs`, `file_glob.rs`, `search_codebase.rs`, `read_skill.rs`, `ask_user_question.rs`, and `request_file_edits.rs` still execute through local client/session state and local permission gates.
- `app/src/ai/agent/api/openrouter.rs:658` reconstructs previous OpenRouter messages from task messages. It only maps selected API message types: user queries, system queries, tool call results, agent output, and run-shell tool calls.
- `app/src/ai/agent/api/openrouter.rs:806` writes current OpenRouter inputs back to the task as persisted API messages, but `input_to_user_query_message` only accepts `AIAgentInput::UserQuery`. `InvokeSkill` turns are not written as user-query or invoke-skill messages.
- `app/src/ai/blocklist/persistence.rs:55` only persists `UserQuery`, `AutoCodeDiffQuery`, and prompt passive suggestions as `ai_queries`. `InvokeSkill` is intentionally excluded, which makes `ai_queries.input` show `[]` for completed skill turns.
- The observed local task for provider conversation token `fa3d6b8e-6d7f-4fcb-b934-75d8f0be8dc6` contained tool calls and assistant output, but no durable user message for the original `/update-tab-config` request. A follow-up asking for the first message caused the model to infer from the first `find` tool call.
- A later local test using provider conversation token `e017afae-dd84-446f-8917-10789815586a` showed a related slash-mode failure: the user submitted `/plan make granular commits`, but the OpenRouter-facing current input and local debug row showed only `make granular commits`. `UserQueryMode::Plan` existed in the model type, but OpenRouter prompt construction and history replay did not serialize that mode into model-visible text.
- Slash-command routing splits into three categories:
  - Local UI/action commands are handled in `app/src/terminal/input/slash_commands/mod.rs` and should not become OpenRouter prompts. Examples include `/add-rule`, `/add-prompt`, `/add-mcp`, `/open-file`, `/conversations`, `/open-rules`, `/open-mcp-servers`, `/model`, `/profile`, and export/open menu commands.
  - Static agent-request commands become typed `AIAgentInput` variants or `UserQueryMode` values. Examples include `/compact`, `/init`, `/create-new-project`, `/pr-comments`, `/plan`, and `/orchestrate`.
  - Real skill invocations resolve through `SkillManager` and become `AIAgentInput::InvokeSkill`.

## Proposed changes

### 1. Make OpenRouter understand `InvokeSkill`

- Add an explicit `AIAgentInput::InvokeSkill` branch in `input_to_prompt_text`.
- The generated current-turn prompt should include:
  - The skill name.
  - The user argument after the skill name.
  - The full `skill.content`.
  - Request context converted by `prompt_text_with_context`.
- Keep the visible UI query unchanged; only the OpenRouter prompt construction should expand the skill into executable instructions.
- Avoid generic "slash command" phrasing in the internal prompt. The model should see this as an invoked skill with instructions, not as a user asking about a command named `/update-tab-config`.
- This replaces the hosted `convert_to.rs` skill request conversion with an OpenRouter-native prompt representation. Do not reintroduce the hosted request path.

### 2. Resolve bundled skill dependencies for OpenRouter

- The immediate failure involves `update-tab-config`, whose `SKILL.md` says to use `tab-configs` as canonical schema context.
- Add a small resolver for bundled skill references used inside bundled skill content, or add targeted metadata to bundled tab-config skills so OpenRouter can include dependent skill content.
- Prefer a structured solution owned by `SkillManager` over string-specific parsing in the OpenRouter adapter:
  - `SkillManager` can expose resolved skill instruction bundles for a `ParsedSkill`.
  - The OpenRouter adapter can call that resolver before prompt serialization.
- If dependency resolution is too broad for the first implementation, explicitly support the bundled tab config skill chain as a narrow slice and document the follow-up.

### 3. Restore the local OpenRouter tool bridge

- Expand `openrouter_tools()` from shell-only to a local-first subset that maps cleanly to existing `warp_multi_agent_api::message::tool_call::Tool` variants and existing local executors:
  - `run_shell_command`
  - `read_files`
  - `grep`
  - `file_glob_v2`
  - `search_codebase`
  - `apply_file_diffs`
  - `read_skill`
  - `ask_user_question`
- Consider `read_mcp_resource` and `call_mcp_tool` if the active `mcp_context` can be serialized to OpenRouter accurately and the tool call arguments can be validated locally. Keep this optional in the first implementation if MCP tool schemas make the slice too large.
- Do not expose hosted-only tools through OpenRouter:
  - Hosted generation/server tools.
  - Hosted subagents/orchestration.
  - Hosted conversation fetch/restore.
  - Warp Drive/cloud object tools.
  - Hosted web retrieval.
  - Billing/quota/credits surfaces.
- Add OpenRouter function schemas that are intentionally close to the local action structs, not the deleted hosted request schema. The adapter should convert OpenRouter function calls into `api::Message::ToolCall` values so the existing `convert_from.rs` and action executors handle execution and result messages.
- Use provider capability gating before sending the request. Only include a tool schema when the corresponding local executor and current session can support it:
  - File reads, grep, glob, codebase search, and file edits only for local sessions or sessions where the existing executor supports the operation.
  - `search_codebase` only when the local index is available; otherwise rely on grep/glob/read files.
  - `apply_file_diffs` only when the diff-review UI can be registered and local write permissions can be enforced.
  - `read_skill` only for skills known to `SkillManager`.
  - `ask_user_question` only when the current execution profile allows it.
- Keep the shell tool as a general fallback, but prefer typed local tools for file reads, searches, and edits when the model can use them. This restores the old hosted-path value without delegating execution to hosted services.

### 4. Convert and replay non-shell tool calls/results for OpenRouter

- Extend `openrouter_tool_call_to_message` to parse each new OpenRouter function and emit the matching `api::message::ToolCall` variant.
- Extend `tool_call_to_prompt_text` and `tool_call_result_to_prompt_text` so follow-up OpenRouter requests can see prior non-shell tool calls and results in natural language:
  - File reads should include file paths and returned file content or read errors.
  - Grep/glob/search results should include matched paths and relevant snippets where available.
  - File edit calls should include target files and unified diff summaries.
  - File edit results should include whether edits were applied, rejected, cancelled, or failed.
  - Skill reads should include the requested skill and returned content or error.
  - User-question results should include the question and selected/entered answers.
- Preserve the current task-message ordering: user input, model-used, assistant output/tool calls, tool results. Follow-up reconstruction must never start with a tool call when a user request preceded it.
- Add defensive parsing for OpenRouter function arguments. Invalid JSON, missing required fields, absolute path problems, unsupported tools, or hosted-only requests should produce an assistant-visible error result or a clear user-facing failure, not a panic and not a silent dropped call.

### 5. Persist skill invocations into task history

- Add an OpenRouter task-history message for `AIAgentInput::InvokeSkill` in `input_messages_for_task`.
- Preferred representation:
  - If `warp_multi_agent_api` has an `InvokeSkill` message type suitable for task history, write that message and update `api_message_to_openrouter_message` to reconstruct it for future OpenRouter requests.
  - If the API message type is not available in this path, write a `UserQuery` message whose query is the display invocation (`/skill arg`) and whose server data or adjacent metadata records the skill reference. This is less expressive but fixes follow-up context loss.
- The persisted task message must include enough data for later OpenRouter turns to reconstruct:
  - The visible user request.
  - The skill name/reference.
  - The user argument.
  - Whether the message originated as a skill invocation.
- Ensure the message is appended before assistant output and tool calls for the same request so chronological reconstruction stays correct.

### 6. Reconstruct skill history for follow-up OpenRouter requests

- Extend `api_message_to_openrouter_message` to handle whichever skill-history representation is written in step 3.
- Reconstructed history should include the visible invocation in the user role. It should also include skill instructions when needed for the model to interpret later tool results or follow-up questions.
- Avoid duplicating large skill content on every historical turn if it causes context bloat:
  - Current turn must include full skill instructions.
  - Historical skill turns may include a compact record by default.
  - If a follow-up depends on skill semantics, include full or summarized skill instructions in the system/current context.
- Preserve prior tool calls and results as today, but ensure they are not the first visible historical messages when a user skill request preceded them.

### 7. Fix local history/debug representation

- Decide whether `ai_queries` should persist skill invocations for user-facing history.
- If yes, extend `PersistedAIInputType` to include a skill invocation variant and update readers to display it.
- If no, stop relying on `ai_queries` for complete conversation/debug history and expose task-derived history where debugging requires full fidelity.
- At minimum, the debug path for a request ID and conversation ID must identify that the request input was `InvokeSkill`, not an empty `[]`.

### 8. Preserve provider boundaries

- Keep these changes scoped to OpenRouter serialization and local persistence. Do not change the hosted provider request format unless the shared model types require a compatible addition.
- OpenRouter tool support is local-first tool support, not hosted tool parity. It should use existing client executors and permission gates.
- If a skill requires unavailable tools, fail before sending or ask the user for an alternative. Do not silently send an underpowered prompt when the skill cannot work.
- Keep removed hosted code removed. Do not restore `ServerApi::generate_multi_agent_output`, hosted `convert_to.rs`, hosted tool-list negotiation, cloud conversation restore, hosted workspace state, or hosted orchestration state.

### 9. Preserve user query modes in OpenRouter prompts

- `AIAgentInput::UserQuery` must serialize `UserQueryMode` when building OpenRouter messages.
- For `UserQueryMode::Plan`, the OpenRouter current-turn message should include the original slash-command form, for example `/plan make granular commits`, plus explicit plan-mode instructions that prevent accidental execution.
- For prior task history, `api_message_to_openrouter_message` must read `message::UserQuery.mode` and reconstruct the same mode-aware prompt. Follow-up turns must not see only the slash-stripped query.
- This is not hosted orchestration restoration. It is local prompt fidelity for structured slash-command intent already represented in client-side types.

### 10. Audit static slash commands by request type

- Do not treat local UI/action commands as model-facing inputs. They should continue dispatching local `TerminalAction`, `WorkspaceAction`, or menu-opening events.
- Audit every `SlashCommandRequest` and `AIAgentInput` variant that can reach OpenRouter:
  - `SummarizeConversation` from `/compact`.
  - `InitProjectRules` from `/init`.
  - `CreateNewProject` from `/create-new-project`.
  - `FetchReviewComments` from `/pr-comments`.
  - `InvokeSkill` from real skill commands.
  - `UserQuery` with `UserQueryMode::Plan` or `UserQueryMode::Orchestrate`.
- Each model-facing variant should have an explicit OpenRouter prompt conversion and task-history replay story. Falling back to `input.user_query()` is acceptable only when that display string is sufficient for the model and history/debug behavior.
- Keep skill handling generic through `AIAgentInput::InvokeSkill`; do not add a separate OpenRouter branch for every bundled skill command.

### 11. Make OpenRouter compaction a real context boundary

- Store OpenRouter output for `AIAgentInput::SummarizeConversation` as `api::message::Message::Summarization` with `SummaryType::ConversationSummary`, not as normal `AgentOutput`.
- When reconstructing OpenRouter history from task messages, find the latest conversation summary message and replay:
  - One user-role message containing the compacted conversation summary.
  - Only task messages that happened after that summary.
- Do not delete local task messages during OpenRouter compaction. The local conversation/debug record may keep the full transcript, but the model-facing OpenRouter prompt should be compacted.
- This restores the local-first behavior expected from `/compact` without bringing back hosted summarization subagents, hosted conversation restore, or hosted state migration.
- `/compact-and` and fork-and-compact flows can keep their existing UI/workspace sequencing as long as the OpenRouter replay layer sees the summary boundary before sending the queued follow-up request.

### 12. Enforce non-paging shell execution locally

- Do not maintain command-specific pager heuristics in the OpenRouter adapter. The adapter should translate tool calls, not classify commands such as `git diff`, `less`, or `man`.
- Apply a Codex-style execution environment centrally for AI-issued shell commands in `ShellCommandExecutor`:
  - `NO_COLOR=1`
  - `TERM=dumb`
  - `LANG=C.UTF-8`
  - `LC_CTYPE=C.UTF-8`
  - `LC_ALL=C.UTF-8`
  - `COLORTERM=`
  - `PAGER=cat`
  - `GIT_PAGER=cat`
  - `GH_PAGER=cat`
  - `WARPER_CI=1`
- Because Warper executes commands by writing to the live terminal rather than spawning them directly with an env map, apply these values with shell-specific command decoration:
  - POSIX shells use a subshell with exported variables.
  - Fish uses a `begin` block with local exported variables.
  - PowerShell saves existing values, assigns temporary values, and restores them in `finally`.
- Keep `uses_pager` only as compatibility metadata for existing action shapes. OpenRouter should not depend on the model setting it correctly.
- Continue to enforce read-only/risky-command permissions separately. Pager prevention is not a substitute for approval policy on mutating commands.

## Testing and validation

- Unit-test `input_to_prompt_text` for `AIAgentInput::InvokeSkill`:
  - Covers PRODUCT behavior 2, 3, and 5.
  - Assert the output contains skill content and user argument.
  - Assert the output is not only `/skill arg`.
- Unit-test tab config dependency inclusion:
  - Covers PRODUCT behavior 4.
  - Invoke `update-tab-config` and assert the OpenRouter prompt contains schema guidance from `tab-configs` or the resolved equivalent.
- Unit-test local OpenRouter tool schemas and conversions:
  - Covers PRODUCT behavior 14, 15, 16, 17, 18, and 23.
  - Assert `openrouter_tools()` exposes the local subset under the right capability gates.
  - Assert each OpenRouter function call maps to the expected `api::message::ToolCall` variant.
  - Assert hosted-only tools are not exposed.
- Unit-test non-shell tool replay:
  - Covers PRODUCT behavior 7, 9, 10, 14, and 22.
  - Build prior task messages containing read-files, grep/glob/search, apply-file-diffs, read-skill, and ask-user-question calls/results; assert OpenRouter follow-up messages summarize them coherently.
- Unit-test invalid tool arguments:
  - Covers PRODUCT behavior 18 and 23.
  - Invalid OpenRouter tool arguments should produce a clear failure path rather than silently dropping the call.
- Unit-test `input_messages_for_task` with `InvokeSkill`:
  - Covers PRODUCT behavior 6, 7, 8, and 25.
  - Assert a durable user-originated task message is emitted before model output.
- Unit-test `openrouter_messages_from_tasks` on a task containing a prior skill invocation, run-shell tool call, run-shell result, and assistant output:
  - Covers PRODUCT behavior 7, 8, 9, 10, and 22.
  - Assert reconstructed messages start with the user's skill invocation.
- Add a regression test for the reported follow-up:
  - First request: `/update-tab-config Update /home/user/.warp-oss/tab_configs/example.toml to be a light theme tab config for ~/Projects/Notes folder`.
  - Simulate a tool call and result.
  - Second request: `what was the first message i sent you?`
  - Assert the generated OpenRouter messages contain the original first request before assistant/tool history.
- Add persistence coverage:
  - Covers PRODUCT behavior 19, 20, and 21.
  - Persist and restore a conversation containing `InvokeSkill`; verify the task-derived conversation preserves the skill input.
  - If `ai_queries` gains a skill variant, verify it no longer stores `[]` for skill turns.
- Add slash-mode coverage:
  - Build a current OpenRouter request from `AIAgentInput::UserQuery` with `UserQueryMode::Plan`; assert the prompt includes `/plan <request>` and plan-mode instructions.
  - Build prior task history containing `message::UserQuery.mode = Plan`; assert a follow-up asking for the first message sees the original `/plan <request>` turn before assistant/tool history.
- Add slash routing coverage:
  - Assert local UI/action slash commands do not create OpenRouter inputs directly.
  - Assert each `SlashCommandRequest` variant that does create an agent request has explicit OpenRouter prompt conversion coverage.
  - Assert real skill commands continue to route through `AIAgentInput::InvokeSkill`, not through user-query mode handling.
- Add compact coverage:
  - Assert OpenRouter summarize output persists as a `Summarization` message.
  - Assert prior task replay after a summary includes the summary and post-summary turns, but excludes pre-summary user and assistant messages.
- Add shell execution policy coverage:
  - Assert AI shell command decoration injects the non-paging environment for POSIX shells.
  - Assert Fish receives local exported variables scoped to the command block.
  - Assert PowerShell command decoration restores prior environment values.
  - Assert OpenRouter no longer guesses pager usage from command names.
- Manual verification:
  - Use OpenRouter with `qwen/qwen3.6-plus` or another configured OpenRouter model.
  - Run the reported `/update-tab-config` request.
  - Ask `what was the first message i sent you?`.
  - Confirm the answer quotes or accurately identifies the original `/update-tab-config ...` request.
  - Confirm the model can read the tab config schema guidance before editing.
  - Confirm the model edits the target tab config through the local file-edit path or gives a precise local permission/tooling reason why it cannot.

## Risks and mitigations

- Large skill content can increase OpenRouter prompt size. Mitigate by including full skill content for the current invocation and compact skill records for older turns unless full history is required.
- Skill dependency resolution can become ad hoc. Mitigate by putting dependency lookup in `SkillManager` or a small skill-instruction resolver instead of hardcoding OpenRouter-specific string scans.
- OpenRouter function schemas can drift from local action structs. Mitigate with conversion tests for every exposed tool and keep the schemas in the OpenRouter adapter near the conversion code.
- Exposing file-edit tools can bypass expected review flows if mapped incorrectly. Mitigate by converting to existing `ApplyFileDiffs`/`RequestFileEdits` actions and using the existing diff UI and write permission checks.
- Persisting skill invocations into `ai_queries` may affect up-arrow history. Mitigate by adding a distinct display label and keeping command recall behavior intentional.
- Writing skill invocations as plain `UserQuery` messages may lose structured metadata. Prefer a real invoke-skill message when the API supports it; use plain user messages only as a compatibility fallback.

## Follow-ups

- Add a provider capability matrix for bundled skills so unsupported OpenRouter skill/tool combinations fail early.
- Add an in-app debug view that shows task-derived request inputs, request IDs, provider model IDs, and provider conversation tokens without requiring direct SQLite inspection.
- Consider a generic transcript export that displays `InvokeSkill`, tool calls, tool results, and assistant output in the same order OpenRouter receives them.
