# WARPER-005: OpenRouter skill and follow-up context fidelity

## Summary

OpenRouter-backed Warper agent conversations must behave like a useful local-first agent: they preserve the user's actual request, skill instructions, local tool calls, tool results, and assistant responses across every turn. Slash-command skill invocations must behave like real skill invocations, not like plain text prompts that lose their instructions or disappear from follow-up context.

## Problem

Warper's OpenRouter-backed agent currently behaves as a chat wrapper with partial terminal access instead of as the local-first agent users expect. It can receive a prompt and sometimes run shell commands, but several user-visible agent capabilities are incomplete or disconnected: skill instructions are not reliably available to the model, local tools beyond shell commands are not generally available through OpenRouter, file-edit workflows do not complete, and follow-up turns lose important conversation state.

During the hosted-service removal, Warper also lost parts of the hosted agent path that made the agent valuable: typed skill invocations, skill reading, local file reading, local file edits, code/file search, and coherent turn replay. WARPER-005 restores those capabilities locally through OpenRouter where they do not require Warp-hosted services.

## Observed breakage

1. Skill invocations degrade into plain prompts. A user invoked `/update-tab-config` with an exact tab config file path and desired change. The agent did not behave as if the `update-tab-config` skill instructions were active; it ran broad inspection commands and did not complete the edit.

2. Dependent skill context is missing. Skills that rely on other local skill guidance, such as `update-tab-config` relying on tab config schema guidance, do not reliably expose that dependent guidance to the OpenRouter model.

3. The agent can inspect but not reliably act. In the reported flow, the agent found or read relevant files but stopped short of editing the requested file. From the user's perspective, the agent looked busy while failing the actual task.

4. Tool access is too narrow. Useful local agent operations that do not require hosted services, such as reading files, searching files, applying file diffs, reading local skills, and asking clarifying questions, are not exposed as first-class OpenRouter agent capabilities.

5. Follow-up context is broken. When the user asked "what was the first message I sent you?", the model could not see the initial user request and inferred an answer from its first tool call.

6. Debug and persistence views are misleading. A completed skill turn can appear as an empty input in local history/debug data even though the user submitted a real request.

7. The model receives an incoherent transcript shape. Prior tool calls and assistant outputs may be visible while the user request that caused them is absent, making the model reason from effects without causes.

8. The failure mode is broader than one skill. `/update-tab-config` is the concrete reproduction, but the underlying issue affects any local-first agent task that depends on skill instructions, local file/context tools, editable file workflows, or accurate follow-up memory.

9. Static slash-command modes can lose their meaning. A request submitted as `/plan make granular commits` can reach OpenRouter as only `make granular commits`, so the model does not know the user asked for planning rather than execution and later history/debug inspection cannot show the original slash-command intent.

10. Slash commands are not one category. Some commands are local UI/actions and should not be sent to OpenRouter at all, some are typed agent requests, and some resolve to real local or bundled skills. A fix that treats every slash command as a skill, or every slash command as plain prompt text, will miss important behavior.

## Goals / Non-goals

- Goal: OpenRouter skill invocations include the invoked skill's instructions.
- Goal: OpenRouter follow-up turns include the user's prior skill invocation text and enough context to answer conversation-history questions accurately.
- Goal: persisted local history and debug surfaces show skill invocations as first-class user inputs where users expect conversation history.
- Goal: tool-call history remains coherent after follow-up turns.
- Goal: OpenRouter can use the existing local client-side tools needed for bundled skills and normal coding tasks, including reading files, searching files, proposing file edits, reading skills, and asking clarifying questions.
- Goal: OpenRouter preserves structured user query modes such as `/plan` and `/orchestrate` as model-visible intent and follow-up-visible history.
- Goal: agent behavior remains local-first: local files, local skills, local MCP servers, and the user's configured OpenRouter model are enough for the supported flows.
- Non-goal: bring back Warp-hosted agent generation, hosted conversation restore, Warp Drive sync, cloud object APIs, hosted orchestration, hosted subagents, hosted web retrieval, hosted billing/quota paths, or any dependency on Warp's hosted services.
- Non-goal: redesign the skill system for Warp-hosted providers.
- Non-goal: change the tab config schema or the `update-tab-config` skill behavior itself.
- Non-goal: make OpenRouter support every former hosted server-only tool.

## Behavior

1. A local-first agent task is successful only when the agent can understand the user's request, gather local context, use the relevant local tools, perform or propose the requested change when permitted, and continue coherently on follow-up turns. Passing only one of these capabilities is not enough.

2. When a user submits a slash-command skill invocation such as `/update-tab-config Update /path/to/file.toml to...`, the current turn sent to OpenRouter includes the invoked skill's full instructions in a form the model can follow.

3. The current turn also includes the user's argument after the skill name exactly enough for the model to act on it. Paths, quoted strings, punctuation, and natural-language instructions must not be dropped or replaced by a generic command name.

4. If a skill references another bundled skill as required context, the model receives the referenced skill context or an equivalent resolved instruction set before it is expected to act. For example, `update-tab-config` must have access to the tab config schema guidance it depends on.

5. The user-visible query for a skill turn remains the slash-command invocation. The UI may show `/update-tab-config ...`, but this display form must not be the only instruction sent to OpenRouter.

6. A skill turn is stored in conversation history as a user-originated request. Later follow-up turns must be able to answer questions such as "what was the first message I sent you?" with the actual original user message, not a guess derived from tool calls or assistant output.

7. Follow-up turns preserve chronological order:
   - User skill invocation.
   - Assistant tool call requests.
   - Tool results.
   - Assistant responses.
   - Later user follow-ups.

8. A follow-up OpenRouter request includes prior user skill invocations when reconstructing conversation history. It must not start visible history at the assistant's first tool call or first textual response.

9. If a skill invocation produces tool calls, the follow-up context preserves both the requested command and the resulting output. The model can reason from the previous command and observed output without hallucinating that the user supplied that output directly.

10. If the model asks to run a command on a skill turn and the command is executed, the next request after the tool result contains the original user skill request as well as the tool result. The model must not continue from tool output alone.

11. If a skill invocation is missing required details, the model should ask a clarifying question according to the skill instructions. It should not silently downgrade the task into a broad search when the skill requires a specific file, layout, command, or behavior.

12. If the user provides an exact local file path and an exact desired change through any supported skill or normal agent request, the agent is expected to read the relevant file, propose or apply the requested edit through the normal local review/permission flow, or explain why it cannot. Ending with a generic readiness message is a failed outcome.

13. If the user provides an exact tab config file path and an exact desired change, the `update-tab-config` skill turn is expected to edit that file or explain why it cannot. This is one regression case for the broader local-edit behavior, not the only required success path.

14. OpenRouter-backed conversations can use local file and search tools through the same user-visible agent action flow as the old hosted path where those tools already have local executors. At minimum, the model can request:
    - Shell commands.
    - File reads.
    - File globbing.
    - Grep or equivalent text search.
    - Codebase search when the local index is available.
    - File edits through the existing diff-review/application UI.
    - Skill reads for known local or bundled skills.
    - Clarifying questions to the user.

15. The agent must choose tools that match the job. For local file reads and file edits, first-class file and diff tools are preferred when available; shell commands are acceptable for terminal inspection and fallback cases, but the agent should not be limited to shell-only behavior.

16. Local tool support must not require Warp-hosted services. If a tool can execute from the local client, local filesystem, local terminal session, local skill store, or user-configured local MCP server, it may be exposed to OpenRouter. If a tool requires Warp-hosted state or hosted execution, it must remain unavailable.

17. When OpenRouter requests a local tool that writes files, runs a risky command, or asks the user a question, existing permission, review, and approval behavior still applies. Restoring agent usefulness must not bypass local safety controls.

18. Unsupported hosted-only capabilities fail clearly. The agent should say the capability is unavailable in local-first Warper instead of pretending to use hosted services or silently degrading into an unrelated prompt.

19. Skill invocation history is available consistently across:
    - The live in-memory conversation.
    - Restored conversations from local persistence.
    - Up-arrow or conversation history views where user-submitted prompts are shown.
    - Debug/export surfaces that users inspect to understand what was sent.

20. Local persisted query history must not represent a completed skill turn as an empty input list where a user expects to see the submitted prompt. If a surface intentionally excludes non-query inputs, it must not be used as evidence of complete conversation history.

21. Debug IDs that include request and conversation identifiers must let a developer correlate the local conversation, the provider conversation token, and the specific request turn well enough to inspect the user input and request type.

22. OpenRouter model responses must not claim the conversation history starts with an assistant action when the user submitted an earlier skill invocation in that same conversation.

23. Provider limitations are surfaced honestly. If OpenRouter cannot support a skill because the skill requires unavailable local tools or hosted-only services, the user sees a clear unsupported-skill error before the request is sent instead of getting a degraded plain prompt.

24. Behavior is provider-consistent where possible: the same skill invocation should have the same user-visible meaning whether the selected model is OpenRouter-backed or another supported agent backend.

25. When a skill invocation is cancelled, retried, queued, or followed by a tool-result continuation, the original user request remains associated with the turn and survives the state transition.

26. Conversation compaction, summarization, or restoration must preserve the fact that a skill was invoked and the user argument that invoked it. Summaries may compress details, but they must not erase the user's first message from later context.

27. The failure mode observed with debug request `14700023-b2f7-46e0-a3b7-a683be332345` and provider conversation token `fa3d6b8e-6d7f-4fcb-b934-75d8f0be8dc6` should not recur: after a skill turn and a follow-up asking for the first message, OpenRouter should identify the original `/update-tab-config ...` request.

28. When a user submits `/plan <request>`, the OpenRouter model receives the original slash-command form or an equivalent explicit plan-mode instruction before it decides what to do. The model should produce a plan and must not silently treat the turn as permission to execute the requested work.

29. A follow-up asking about conversation history after a `/plan` turn must identify the original `/plan <request>` message, not the slash-stripped request text or the first tool call.

30. Local UI/action slash commands such as `/add-rule`, `/add-prompt`, `/add-mcp`, `/open-file`, and `/conversations` remain local actions. They should open the relevant UI, menu, pane, or local file flow without being serialized into an OpenRouter prompt unless the user subsequently submits an agent request from that flow.

31. Static agent-request slash commands such as `/compact`, `/init`, `/create-new-project`, `/pr-comments`, `/plan`, and `/orchestrate` must be represented through their structured `AIAgentInput` meaning when they do reach OpenRouter. Their OpenRouter serialization should preserve the user-visible command intent enough for correct behavior and later history/debug inspection.

32. Skill slash commands resolved through the skill system must use the generic skill invocation path. The implementation must not hardcode individual bundled skill names except for narrowly documented dependency resolution, such as including `tab-configs` context for tab-config editing skills until a general dependency resolver exists.

33. `/compact` is not only a visible summary message. After the model generates a conversation summary, later OpenRouter requests must treat that summary as the compacted context boundary.

34. Follow-up turns after `/compact` must include the latest compacted conversation summary and turns that happened after that summary. They must not replay the full pre-summary transcript unless the user explicitly asks to inspect local history outside the model context.

35. `/compact-and <request>` and equivalent fork-and-compact flows must send the queued follow-up request after the summary is available, with OpenRouter seeing the compacted summary plus the queued request rather than the full old transcript.

36. Agent-issued shell commands must not trap the user in a pager or colorized terminal UI that requires manual keypresses such as `q`. Warper should apply this as a local execution policy for AI-requested shell commands instead of relying on the model to classify individual commands.

37. OpenRouter shell-tool documentation should ask for finite, non-interactive commands. The local executor remains authoritative: pager prevention, approval checks, and command safety policy are enforced by Warper before the command is written to the terminal.
