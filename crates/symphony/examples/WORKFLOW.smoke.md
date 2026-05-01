---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: symphony-sandbox-4c92d5ea8f4c
  active_states: ["Todo", "In Progress"]
polling:
  interval_ms: 30000
workspace:
  root: ~/.warp/symphony_workspaces
hooks:
  timeout_ms: 60000
agent:
  max_concurrent_agents: 1
  max_diff_lines: 500
  max_turns: 3
  agent_label_required: "agent:claude"
---

You are a coding agent working on Linear issue {{ issue.identifier }}: {{ issue.title }}.

## Task

{{ issue.description | default: "No description provided." }}

## Context

- Your working directory is the workspace dir Symphony created for this issue.
- Do exactly what the issue asks, nothing more.
- When done, exit cleanly. Symphony will check the diff stat and audit-log the result.
