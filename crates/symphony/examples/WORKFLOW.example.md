---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: pdx-software
  active_states: ["Todo", "In Progress"]
polling:
  interval_ms: 30000
workspace:
  root: ~/.warp/symphony_workspaces
hooks:
  after_create: |
    git clone https://github.com/example/repo .
  before_run: |
    cargo check
agent:
  max_concurrent_agents: 1
  max_diff_lines: 500
  max_turns: 5
  agent_label_required: "agent:claude"
---

You are Helm, a coding agent working on Linear issue {{ issue.identifier }}: {{ issue.title }}.

{{ issue.description | default: "No description provided." }}

Labels: {% for label in issue.labels %}{{ label }}{% unless forloop.last %}, {% endunless %}{% endfor %}

When you're done, leave the working tree in a clean state and exit.
