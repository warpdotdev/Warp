# Warp Desktop — English (source-of-truth locale)
# 本文件由多 agent 并行编辑,各自维护自己的 SECTION,key 以 surface 前缀隔离避免冲突。
# 加 key 时 ctrl-F 找到对应 SECTION 头追加;新 surface 在文件末尾加新 SECTION。
#
# 命名规范:kebab-case,前缀按 surface,例 settings-ai-title / drive-folder-rename-title
# 变量插值用 Fluent { $name } 语法,不要拼接

# =============================================================================
# SECTION: common (Owner: foundation)
# =============================================================================

app-name = Warp
app-tagline = The local-first agentic terminal for developers

common-ok = OK
common-cancel = Cancel
common-apply = Apply
common-save = Save
common-delete = Delete
common-confirm = Confirm
common-close = Close
common-reset = Reset
common-back = Back
common-next = Next
common-yes = Yes
common-no = No
common-continue = Continue
common-approve = Approve
common-deny = Deny
common-import = Import
common-upgrade = Upgrade
common-default = Default
common-editing = Editing
common-viewing = Viewing
common-tooltip-enter-edit-mode = Click to start editing
common-tooltip-exit-edit-mode = Click to exit editing
common-restored = Restored
common-continued = Continued
common-deleted = Deleted
common-send-feedback = Send Feedback
common-something-went-wrong = Something went wrong
common-no-results-found = No results found.
common-edit = Edit
common-add = Add
common-remove = Remove
common-rename = Rename
common-copy = Copy
common-paste = Paste
common-search = Search
common-view = View
common-loading = Loading…
common-error = Error
common-warning = Warning
common-info = Info
common-success = Success
common-all = All
common-none = None
common-unknown = Unknown
common-open = Open
common-restore = Restore
common-duplicate = Duplicate
common-export = Export
common-trash = Trash
common-copy-link = Copy link
common-untitled = Untitled
common-retry = Retry
common-maximize = Maximize
common-discard = Discard
common-undo = Undo
common-commit = Commit
common-push = Push
common-publish = Publish
common-create = Create
common-configure = Configure
common-dismiss = Dismiss
common-manage = Manage
common-failed = Failed
common-done = Done
common-working = Working
common-cut = Cut
common-previous = Previous
common-suggested = Suggested
common-copied-to-clipboard = Copied to clipboard
common-new = New
common-no-results = No results
common-learn-more = Learn more
common-skip = Skip
common-get-warping = Get Warping
common-try-again = Try again
common-settings = Settings
common-recommended = Recommended
common-enabled = Enabled
common-disabled = Disabled
common-free = Free
common-list-prefix = {" - "}
common-current-directory = the current directory

# =============================================================================
# SECTION: agent-management (Owner: agent-i18n-remaining)
# Files: app/src/ai/agent_management/**
# =============================================================================

agent-management-filter-all-tooltip = View your agent tasks plus all shared team tasks
agent-management-filter-personal = Personal
agent-management-filter-personal-tooltip = View agent tasks you created
agent-management-get-started = Get started
agent-management-view-agents = View Agents
agent-management-clear-filters = Clear filters
agent-management-clear-all = Clear all
agent-management-new-agent = New agent
agent-management-status = Status
agent-management-source = Source
agent-management-created-on = Created on
agent-management-has-artifact = Has artifact
agent-management-harness = Harness
agent-management-environment = Environment
agent-management-created-by = Created by
agent-management-last-24-hours = Last 24 hours
agent-management-past-3-days = Past 3 days
agent-management-last-week = Last week
agent-management-artifact-pull-request = Pull Request
agent-management-artifact-plan = Plan
agent-management-artifact-screenshot = Screenshot
agent-management-artifact-file = File
agent-management-source-scheduled = Scheduled
agent-management-source-local-agent = Warp (local agent)
agent-management-source-cloud-agent = Warp agent
agent-management-source-oz-web = Oz
agent-management-source-github-action = GitHub Action
agent-management-no-session-available = No session available
agent-management-session-expired = Session expired
agent-management-session-expired-tooltip = Sessions expire after one week and cannot be opened.
agent-management-metadata-source = Source: { $source }
agent-management-metadata-harness = Harness: { $harness }
agent-management-metadata-run-time = Run time: { $run_time }
agent-management-metadata-credits-used = Credits used: { $usage }
agent-management-environment-selected = Environment: { $environment }
agent-management-loading-cloud-runs = Loading agent runs

# =============================================================================
# SECTION: workspace-runtime (Owner: agent-i18n-remaining)
# Files: app/src/workspace/view.rs
# =============================================================================

workspace-menu-update-warp-manually = Update Warp manually
workspace-menu-whats-new = What's new
workspace-menu-settings = Settings
workspace-menu-keyboard-shortcuts = Keyboard shortcuts
workspace-menu-documentation = Documentation
workspace-menu-feedback = Feedback
workspace-menu-view-warp-logs = View Warp logs
workspace-menu-slack = Slack
workspace-toast-failed-load-conversation = Failed to load conversation.
workspace-toast-failed-load-conversation-for-forking = Failed to load conversation for forking.
workspace-toast-conversation-forking-failed = Conversation forking failed.
workspace-toast-no-terminal-pane-for-context = No terminal pane open. Open a new pane to attach as context.
workspace-toast-plan-already-in-context = This plan is already in context.
workspace-toast-command-still-running = A command in this session is still running.
workspace-toast-cannot-open-terminal-session = Cannot open a new terminal session
workspace-toast-out-of-ai-credits = Looks like you're out of AI credits.
workspace-toast-upgrade-more-credits = Upgrade for more credits.
workspace-toast-disabled-synchronized-inputs = Disabled all synchronized inputs.
workspace-toast-conversation-deleted = Conversation deleted
workspace-search-repos-placeholder = Search repos
workspace-search-tabs-placeholder = Search tabs...
workspace-rearrange-toolbar-items = Re-arrange toolbar items
workspace-new-session-agent = Agent
workspace-new-session-terminal = Terminal
workspace-new-session-cloud-oz = Agent tab
workspace-new-session-local-docker-sandbox = Local Docker Sandbox
workspace-new-worktree-config = New worktree config
workspace-new-tab-config = New tab config
workspace-reopen-closed-session = Reopen closed session
app-menu-new-window = New Window
app-menu-save-new = Save New...
app-menu-launch-configurations = Launch Configurations
app-menu-warp = Warp
app-menu-preferences = Preferences
app-menu-privacy-policy = Privacy Policy...
app-menu-debug = Debug
app-menu-set-default-terminal = Set Warp as Default Terminal
app-menu-file = File
app-menu-edit = Edit
app-menu-use-warp-prompt = Use Warp's Prompt
app-menu-copy-on-select-terminal = Copy on Select within the Terminal
app-menu-synchronize-inputs = Synchronize Inputs
app-menu-view = View
app-menu-toggle-mouse-reporting = Toggle Mouse Reporting
app-menu-toggle-scroll-reporting = Toggle Scroll Reporting
app-menu-toggle-focus-reporting = Toggle Focus Reporting
app-menu-compact-mode = Compact Mode
app-menu-tab = Tab
app-menu-ai = AI
app-menu-blocks = Blocks
app-menu-drive = Drive
app-menu-show-in-band-command-blocks = Show In-band Command Blocks
app-menu-hide-in-band-command-blocks = Hide In-band Command Blocks
app-menu-show-warpified-ssh-blocks = Show Warpified SSH Blocks
app-menu-hide-warpified-ssh-blocks = Hide Warpified SSH Blocks
app-menu-show-initialization-block = Show Initialization Block
app-menu-hide-initialization-block = Hide Initialization Block
app-menu-window = Window
app-menu-enable-shell-debug-mode = Enable Shell Debug Mode (-x) for New Sessions
app-menu-disable-shell-debug-mode = Disable Shell Debug Mode (-x) for New Sessions
app-menu-enable-pty-recording = Enable PTY Recording Mode (warp.pty.recording)
app-menu-disable-pty-recording = Disable PTY Recording Mode (warp.pty.recording)
app-menu-enable-in-band-generators = Enable in-band generators for new sessions
app-menu-disable-in-band-generators = Disable in-band generators for new sessions
app-menu-manually-toggle-network-status = Manually Toggle Network Status
app-menu-export-default-settings-csv = Export Default Settings as CSV to home dir
app-menu-create-anonymous-user = Create anonymous user
app-menu-send-feedback = Send Feedback...
app-menu-help = Help
app-menu-warp-documentation = Warp Documentation...
app-menu-github-issues = GitHub Issues...
app-menu-warp-slack-community = Warp Slack Community...
workspace-update-and-relaunch-warp = Update and relaunch Warp
workspace-updating-to-version = Updating to ({ $version })
workspace-update-warp-manually = Update Warp manually
pane-get-started-title = Get started
pane-new-tab-title = New tab

# =============================================================================
# SECTION: terminal-runtime (Owner: agent-i18n-remaining)
# Files: app/src/terminal/view.rs
# =============================================================================

terminal-banner-completions-not-working-prefix = Seems like your completions are not working (
terminal-banner-more-info-lower = more info
terminal-banner-more-info = More info
terminal-banner-completions-not-working-middle = ). Enabling tmux warpification in {" "}
terminal-banner-settings = settings
terminal-banner-completions-not-working-suffix =  may resolve this issue.
terminal-banner-shell-config-incompatible = Your shell configuration is incompatible with Warp...{"  "}
terminal-banner-did-you-intend = Did you intend {" "}
terminal-banner-move-cursor =  to move the cursor?
terminal-toast-powershell-subshells-not-supported = PowerShell subshells not supported
terminal-dont-ask-again = Don't ask me this again
terminal-clear-upload = Clear upload
terminal-manage-defaults = Manage defaults
terminal-free-credits = Free credits
terminal-cloud-agent-run = Agent run
terminal-agent-header-for-terminal = for terminal
ssh-remote-choice-title = Choose your experience for this remote session:
ssh-remote-choice-install-extension = Install Warp's SSH extension
ssh-remote-choice-install-extension-desc = Install Warp's extension to enable agent features like file browsing, code review, and intelligent command completions in this session.
ssh-remote-choice-continue-without-installing = Continue without installing
ssh-remote-choice-continue-without-installing-desc = You'll still get a Warpified experience just without the agent features.
ssh-remote-choice-manage-warpify-settings = Manage Warpify settings
ai-document-show-version-history = Show version history
ai-document-update-agent = Update Agent
ai-document-save-and-sync-tooltip = Save and auto-sync this plan to your Warp Drive
ai-document-show-in-warp-drive = Show in Warp Drive
ai-document-save-as-markdown-file = Save as markdown file
ai-document-attach-to-active-session = Attach to active session
ai-document-copy-plan-id = Copy plan ID
ai-document-plan-id-copied = Plan ID copied to clipboard
ai-conversation-view-in-oz = View run
ai-conversation-view-in-oz-tooltip = View this agent run
ai-block-open-in-github = Open in GitHub
ai-block-open-in-code-review = Open in code review
ai-block-manage-rules = Manage rules
ai-block-review-changes = Review changes
ai-block-open-all-in-code-review = Open all in code review
ai-block-dont-show-again = Don't show again
ai-block-rewind = Rewind
ai-block-rewind-tooltip = Rewind to before this block
ai-block-remove-queued-prompt = Remove queued prompt
ai-block-send-now = Send now
ai-block-check-now =  · Check now
ai-block-check-now-tooltip = Ask the agent to check this command now, skipping its timer.
ai-block-resume-conversation = Resume conversation
ai-block-continue-conversation = Continue conversation
ai-block-fork-conversation = Fork conversation
ai-block-show-credit-usage-details = Show credit usage details
ai-block-follow-up-existing-conversation = Follow up with existing conversation
ai-block-accept = Accept
ai-block-auto-approve = Auto-approve
ai-rule-add-rule = Add rule
ai-rule-edit-rule = Edit rule
ai-rule-delete-rule = Delete rule
ai-aws-refresh-credentials = Refresh AWS Credentials
ai-footer-enable-notifications = Enable notifications
ai-footer-enable-notifications-tooltip = Install the Warp plugin to enable rich agent notifications within Warp
ai-footer-notifications-setup-instructions = Notifications setup instructions
ai-footer-install-plugin-instructions-tooltip = View instructions to install the Warp plugin
ai-footer-update-warp-plugin = Update Warp plugin
ai-footer-plugin-update-available-tooltip = A new version of the Warp plugin is available
ai-footer-plugin-update-instructions = Plugin update instructions
ai-footer-plugin-update-instructions-tooltip = View instructions to update the Warp plugin
ai-footer-context-window-usage-tooltip = Context window usage
ai-footer-choose-environment-tooltip = Choose an environment
ai-footer-reasoning-depth-tooltip = Reasoning depth
ai-footer-file-explorer = File explorer
ai-footer-open-file-explorer = Open file explorer
ai-footer-rich-input = Rich Input
ai-footer-open-rich-input = Open Rich Input
ai-footer-open-coding-agent-settings = Open coding agent settings
ai-ask-user-question-placeholder = Type your answer and press Enter
ai-ask-user-questions-skipped = Questions skipped
ai-ask-user-answered-question = Answered question
ai-ask-user-answered-all-questions = Answered all { $total } questions
ai-ask-user-answered-count = Answered { $answered_count } of { $total } questions
ai-code-diff-requested-edit-title = Requested Edit
ai-cloud-setup-visit-oz = Open agent setup
ai-inline-code-diff-review-changes = Review changes
ai-execution-profile-name-placeholder = e.g. "YOLO code"
ai-execution-profile-delete-profile = Delete profile
ai-notifications-mark-all-as-read = Mark all as read
ai-assistant-copy-transcript-tooltip = Copy transcript to clipboard
code-comment = Comment
code-copy-file-path = Copy file path
code-select-all = Select all
code-replace-all = Replace all
code-goto-line-placeholder = Line number:Column
code-open-file-unavailable-remote-tooltip = Opening files is unavailable for remote sessions
code-view-markdown-preview = View Markdown preview
markdown-display-mode-rendered = Rendered
markdown-display-mode-raw = Raw
code-review-commit-and-create-pr = Commit and create PR
notebook-link-text-placeholder = Text
notebook-link-url-placeholder = Link (web or file)
notebook-block-embed = Embed
notebook-block-divider = Divider
notebook-insert-block-tooltip = Insert block
notebook-refresh-notebook = Refresh notebook
notebook-refresh-file = Refresh file
notebook-open-in-editor = Open in editor
notebook-sign-in-to-edit = Sign in to edit
editor-custom-keybinding = Custom...
editor-change-keybinding = Change keybinding
autosuggestion-ignore-this-suggestion = Ignore this suggestion
codex-use-latest-model = Use latest codex model
openwarp-launch-visit-repo = Visit the repo
openwarp-launch-title = Warp is now open-source
openwarp-launch-description = You, our community, can participate in building Warp using an agent-first workflow.
openwarp-launch-contribute-title = Contribute
openwarp-launch-contribute-description = Warp's client code is now open source. Get started by using the /feedback skill to open an issue, and follow the contribution guidelines here.
openwarp-launch-contribute-link-text = here
openwarp-launch-oad-title = Open Automated Development
openwarp-launch-oad-description = The Warp repo is managed by an agent-first local workflow powered by Oz.
openwarp-launch-auto-model-title = Introducing 'auto (open-weights)'
openwarp-launch-auto-model-description = We've added a new auto model that picks the best open weight model for a task, like Kimi or MiniMax.
hoa-see-whats-new = See what's new
hoa-finish = Finish
session-config-get-warping = Get Warping
uri-custom-uri-invalid = Custom URI is invalid.
context-node-install-nvm = Install nvm
context-node-install-node = nvm install node
context-node-installed = Installed
context-chip-change-git-branch = Change git branch
context-chip-view-pull-request = View pull request
context-chip-change-working-directory = Change working directory
context-chip-working-directory = Working directory
settings-ai-repo-placeholder = e.g. ~/code-repos/repo
settings-ai-commands-comma-separated-placeholder = Commands, comma separated
settings-ai-regex-example-placeholder = e.g. ls .*
settings-ai-command-supports-regex-placeholder = command (supports regex)
settings-ai-aws-login-placeholder = aws login
settings-ai-default-placeholder = default
settings-working-directory-path-placeholder = Directory path
settings-startup-shell-executable-path-placeholder = Executable path
settings-agent-providers-base-url-placeholder = https://api.deepseek.com/v1
drive-sharing-only-people-invited = Only people invited
drive-sharing-anyone-with-link = Anyone with the link
drive-sharing-only-invited-teammates = Local access only
drive-sharing-teammates-with-link = Local access with link
terminal-warpify-subshell = Warpify subshell
terminal-warpify-subshell-tooltip = Enable Warp shell integration in this session
terminal-use-agent = Use agent
terminal-use-agent-tooltip = Ask the Warp agent to assist
terminal-give-control-back-to-agent = Give control back to agent
terminal-resume-agent-tooltip = Ask the Warp agent to resume
terminal-voice-input-tooltip = Voice input
terminal-attach-file-tooltip = Attach file
terminal-slash-commands-tooltip = Slash commands
terminal-manage-api-keys-tooltip = Manage API keys
terminal-profiles = Profiles
terminal-manage-profiles = Manage profiles
terminal-continue-locally = Continue locally
terminal-fork-conversation-locally-tooltip = Fork this conversation locally
terminal-open-in-warp = Open in Warp
terminal-open-conversation-in-warp-tooltip = Open this conversation in the Warp desktop app
terminal-stop-sharing = Stop sharing
terminal-copy-session-sharing-link = Copy session sharing link
terminal-shared-session-make-editor = Make editor
terminal-shared-session-make-viewer = Make viewer
terminal-shared-session-change-role = Change role
terminal-choose-execution-profile-tooltip = Choose an AI execution profile
terminal-choose-agent-model-tooltip = Choose an agent model
terminal-input-cli-agent-rich-input-hint = Tell the agent what to build...
terminal-input-enter-prompt-for-agent = Enter prompt for { $agent }...
terminal-input-cloud-agent-hint = Kick off an agent
terminal-input-a11y-label = Command Input.
terminal-input-a11y-helper = Input your shell command, press enter to execute. Press cmd-up to navigate to output of previously executed commands. Press cmd-l to re-focus command input.
terminal-input-ai-command-search-hint = Type '#' for AI command suggestions
terminal-input-run-commands-hint = Run commands
terminal-input-agent-hint-deploy-react-vercel = Warp anything e.g. Deploy my React app to Vercel and set up environment variables
terminal-input-agent-hint-debug-python-ci = Warp anything e.g. Help me debug why my Python tests are failing in CI
terminal-input-agent-hint-setup-microservice = Warp anything e.g. Set up a new microservice with Docker and create the deployment pipeline
terminal-input-agent-hint-fix-node-memory-leak = Warp anything e.g. Find and fix the memory leak in my Node.js application
terminal-input-agent-hint-backup-postgres = Warp anything e.g. Create a backup script for my PostgreSQL database and schedule it
terminal-input-agent-hint-migrate-mysql-postgres = Warp anything e.g. Help me migrate my data from MySQL to PostgreSQL
terminal-input-agent-hint-monitor-aws = Warp anything e.g. Set up monitoring and alerts for my AWS infrastructure
terminal-input-agent-hint-build-fastapi = Warp anything e.g. Build a REST API for my mobile app using FastAPI
terminal-input-agent-hint-optimize-sql = Warp anything e.g. Help me optimize my SQL queries that are running slowly
terminal-input-agent-hint-github-actions = Warp anything e.g. Create a GitHub Actions workflow to automatically deploy on merge
terminal-input-agent-hint-redis-cache = Warp anything e.g. Set up Redis caching for my web application
terminal-input-agent-hint-kubernetes-pods = Warp anything e.g. Help me troubleshoot why my Kubernetes pods keep crashing
terminal-input-agent-hint-bigquery-pipeline = Warp anything e.g. Build a data pipeline to process CSV files and load them into BigQuery
terminal-input-agent-hint-ssl-https = Warp anything e.g. Set up SSL certificates and configure HTTPS for my domain
terminal-input-agent-hint-refactor-legacy-code = Warp anything e.g. Help me refactor this legacy code to use modern design patterns
terminal-input-agent-hint-unit-tests = Warp anything e.g. Create unit tests for my authentication service
terminal-input-agent-hint-elk-logs = Warp anything e.g. Set up log aggregation with ELK stack for my distributed system
terminal-input-agent-hint-oauth-express = Warp anything e.g. Help me implement OAuth2 authentication in my Express.js app
terminal-input-agent-hint-optimize-docker = Warp anything e.g. Optimize my Docker images to reduce build times and size
terminal-input-agent-hint-ab-testing = Warp anything e.g. Set up A/B testing infrastructure for my web application
terminal-input-steer-agent-hint = Steer the running agent
terminal-input-steer-agent-backspace-hint = Steer the running agent, or backspace to exit
terminal-input-follow-up-hint = Ask a follow up
terminal-input-follow-up-backspace-hint = Ask a follow up, or backspace to exit
terminal-input-search-queries = Search queries
terminal-input-search-queries-rewind = Search queries to rewind to
terminal-input-search-conversations = Search conversations
terminal-input-search-skills = Search skills
terminal-input-search-models = Search models
terminal-input-search-profiles = Search profiles
terminal-input-search-commands = Search commands
terminal-input-search-prompts = Search prompts
terminal-input-search-indexed-repos = Search indexed repos
terminal-input-search-plans = Search plans
terminal-input-choose-agent-model = Choose agent model
terminal-message-new-agent-conversation = {" "}new /agent conversation
terminal-message-agent-for-new-conversation = /agent for new conversation
terminal-message-selected-text-attached = selected text attached as context
terminal-message-to-remove = {" "}to remove
terminal-message-to-dismiss = {" "}to dismiss
terminal-message-plan-with-agent = {" "}plan with agent
terminal-message-continue-conversation = {" "}to continue conversation
terminal-message-to-execute = {" "}to execute
terminal-message-to-send = {" "}to send
terminal-message-open-conversation-title = {" "}to open '{ $title }'
terminal-message-autodetected = {" "}(autodetected){" "}
terminal-message-to-override = {" "}to override
terminal-message-to-navigate = {" "}to navigate
terminal-message-to-cycle-tabs = {" "}to cycle tabs
terminal-message-to-select = {" "}to select
terminal-message-select-save-profile = {" "}select and save to profile
terminal-message-open-plan = {" "}open plan
terminal-starting-shell = Starting shell...
terminal-input-no-skills-found = No skills found
terminal-model-specs-title = Model Specs
terminal-model-specs-description = Warp's benchmarks for how well a model performs in our harness, the rate at which it consumes credits, and task speed.
terminal-model-specs-reasoning-level-title = Reasoning level
terminal-model-specs-reasoning-level-description = Increased reasoning levels consume more credits and have higher latency, but higher performance for complicated tasks.
terminal-model-auto-mode-title = Auto mode
terminal-model-auto-mode-description = Auto will select the best model for the task. Cost-efficiency optimizes for cost, Responsiveness optimizes for response speed.
terminal-model-banner-base-agent = You're using the base agent. Full terminal use models only apply to the full terminal use agent.
terminal-model-banner-full-terminal-agent = You're using the full terminal use agent. Base models only apply to the base agent.
terminal-filter-block-output-placeholder = Filter block output

# =============================================================================
# SECTION: object-surfaces (Owner: agent-i18n-remaining)
# Files: app/src/code_review/**, app/src/notebooks/**, app/src/workflows/**, app/src/drive/**
# =============================================================================

code-review-tooltip-show-file-navigation = Show file navigation
code-review-discard-changes = Discard changes
code-review-create-pr = Create PR
code-review-add-diff-set-context = Add diff set as context
code-review-show-saved-comment = Show saved comment
code-review-add-comment = Add comment
code-review-discard-all = Discard all
code-review-initialize-codebase = Initialize codebase
code-review-initialize-codebase-tooltip = Enables codebase indexing and WARP.md
code-review-open-repository = Open repository
code-review-open-repository-tooltip = Navigate to a repo and initialize it for coding
code-review-open-file = Open file
code-review-add-file-diff-context = Add file diff as context
code-review-copy-file-path = Copy file path
code-review-no-open-changes = No open changes
code-review-header-reviewing-changes = Reviewing code changes
code-review-search-diff-placeholder = Search diff sets or branches to compare…
code-review-one-comment = 1 Comment
code-review-copy-text = Copy text
code-review-file-level-comment-cannot-edit = File-level comments currently can't be edited.
code-review-outdated-comment-cannot-edit = Outdated comments can't be edited.
code-review-view-in-github = View in GitHub
notebook-menu-attach-active-session = Attach to active session
object-menu-open-on-desktop = Open on Desktop
notebook-tooltip-restore-from-trash = Restore notebook from trash
notebook-tooltip-copy-to-personal = Copy notebook contents into your personal workspace
notebook-copy-to-personal = Copy to Personal
notebook-tooltip-copy-to-clipboard = Copy notebook contents to your clipboard
notebook-copy-all = Copy All
object-toast-link-copied = Link copied to clipboard
drive-toast-finished-exporting = Finished exporting objects

# =============================================================================
# SECTION: remaining-settings-tabs-env (Owner: agent-i18n-remaining)
# Files: app/src/settings_view/**, app/src/tab_configs/**, app/src/env_vars/**
# =============================================================================

settings-environment-delete-button = Delete environment
settings-language-system-default = System default
settings-language-english = English
tab-config-open-tab = Open Tab
tab-config-make-default = Make default
tab-config-already-default = Already the default
tab-config-edit-config = Edit config
env-vars-restore-tooltip = Restore environment variables from trash
env-vars-variables-label = Variables

# =============================================================================
# SECTION: onboarding-callout (Owner: agent-i18n-remaining)
# Files: crates/onboarding/src/callout/view.rs
# =============================================================================

onboarding-callout-meet-input-title = Meet the Warp input
onboarding-callout-meet-input-text-prefix = Your terminal input accepts both terminal commands and agent prompts and automatically detects which you're using. Use
onboarding-callout-meet-input-text-suffix = to lock the input to Agent mode (natural language) or Terminal mode (commands).
onboarding-callout-talk-agent-title = Talk to the agent
onboarding-callout-talk-agent-text = You can type in natural language to engage the agent. Submit the query below to start: What tests exist in this repo, how are they structured, and what do they cover?
onboarding-callout-skip = Skip
onboarding-callout-submit = Submit
onboarding-callout-finish = Finish
onboarding-callout-meet-terminal-title = Meet your terminal input
onboarding-callout-meet-updated-terminal-title = Meet your updated terminal input
onboarding-callout-meet-terminal-text-prefix = Run commands from the terminal, or use
onboarding-callout-meet-terminal-text-suffix = to start or send to the agent.
onboarding-callout-nl-overrides-title = Natural language overrides
onboarding-callout-nl-overrides-text-prefix = You can always override any auto-detection using
onboarding-callout-nl-support-title = Natural language support
onboarding-callout-nl-support-text-prefix = Natural language input is off by default. If enabled, you can type requests in plain English and Warp will autodetect queries for the agent. You can always override them using
onboarding-callout-enable-nl-detection = Enable Natural Language Detection
onboarding-callout-new-agent-title = Introducing Warp's new agent experience
onboarding-callout-new-agent-text = Agent conversations are now their own scoped view outside of your terminal. Simply hit ESC to return to the terminal at any point.
onboarding-callout-updated-agent-input-title = Updated agent input
onboarding-callout-updated-agent-input-project-text = Your agent input will detect natural language as well as commands by default. Use ! to lock the input in bash mode to write commands.\n\nSubmit the query below to have the agent initialize this project, or ⊗ to clear the input and start your own!
onboarding-callout-skip-initialization = Skip initialization
onboarding-callout-initialize = Initialize
onboarding-callout-updated-agent-input-text = Your agent input will detect natural language as well as commands by default. Use ! to lock the input in bash mode to write commands.
onboarding-callout-back-terminal = Back to terminal

# =============================================================================
# SECTION: language (Owner: foundation)
# Files: app/src/settings_view/appearance_page.rs (Language widget + restart modal)
# =============================================================================

language-widget-label = Language
language-widget-secondary = Restart Warp for the change to fully take effect.
language-restart-required-title = Language changed
language-restart-required-body = Warp's UI language has been updated. Some text will switch immediately, but a full restart is required for the change to take effect everywhere.

# =============================================================================
# SECTION: settings (Owner: agent-settings)
# Files: app/src/settings_view/**
# =============================================================================

# --- ANCHOR-SUB-MOD-NAV (agent-settings-mod) ---
# settings_view/mod.rs SettingsSection Display labels + context menu pane actions

# Sidebar / SettingsSection labels (Display impl)
settings-section-about = About
settings-section-account = Account
settings-section-mcp-servers = MCP Servers
settings-section-billing-and-usage = Billing and usage
settings-section-appearance = Appearance
settings-section-features = Features
settings-section-keybindings = Keyboard shortcuts
settings-section-privacy = Privacy
settings-section-referrals = Referrals
settings-section-shared-blocks = Shared blocks
settings-section-warp-drive = Warp Drive
settings-section-warpify = Warpify
settings-section-ai = AI
settings-section-warp-agent = Warp Agent
settings-section-agent-profiles = Profiles
settings-section-agent-mcp-servers = MCP servers
settings-section-agent-providers = Providers
settings-section-knowledge = Knowledge
settings-section-third-party-cli-agents = Third party CLI agents
settings-section-code = Code
settings-section-editor-and-code-review = Editor and Code Review
settings-section-cloud-environments = Environments
settings-section-oz-cloud-api-keys = Agent API Keys
settings-title = Settings

# Context menu items (split / close pane)
settings-pane-split-right = Split pane right
settings-pane-split-left = Split pane left
settings-pane-split-down = Split pane down
settings-pane-split-up = Split pane up
settings-pane-close = Close pane

# Debug toggle setting descriptions (command palette)
settings-debug-show-init-block = Show initialization block
settings-debug-hide-init-block = Hide initialization block
settings-debug-show-inband-blocks = Show in-band command blocks
settings-debug-hide-inband-blocks = Hide in-band command blocks

# --- ANCHOR-SUB-ABOUT (agent-settings-about) ---
# 此锚点下放 settings_view/about_page.rs + main_page.rs 字符串
# 命名前缀:settings-about-* / settings-main-*

# about_page.rs
settings-about-copyright = Copyright 2026 Warp
settings-about-automatic-updates-label = Automatic updates
settings-about-automatic-updates-description = When enabled, OpenWarp checks for new versions in the background. When a new version is available, it will be shown above with a link to download manually from GitHub. OpenWarp never downloads or installs updates automatically.
settings-about-update-checking = Checking for updates…
settings-about-update-up-to-date = OpenWarp is up to date.
settings-about-update-available = New version { $version } is available.
settings-about-update-check-now = Check for updates
settings-about-update-open-release = Download from GitHub

# main_page.rs — account
settings-main-sign-up = Local profile
settings-main-local-profile = Local profile
settings-main-plan-free = Free
settings-main-compare-plans = Compare plans
settings-main-contact-support = Contact support
settings-main-manage-billing = Manage billing
settings-main-upgrade-to-turbo = Upgrade to Turbo plan
settings-main-upgrade-to-lightspeed = Upgrade to Lightspeed plan

# main_page.rs — version / autoupdate
settings-main-version-label = Version
settings-main-status-up-to-date = Up to date
settings-main-cta-check-for-updates = Check for updates
settings-main-status-checking = checking for update...
settings-main-status-downloading = downloading update...
settings-main-status-update-available = Update available
settings-main-cta-relaunch-warp = Relaunch Warp
settings-main-status-updating = Updating...
settings-main-status-installed-update = Installed update
settings-main-status-cant-install = A new version of Warp is available but can't be installed
settings-main-status-cant-launch = A new version of Warp is installed but can't be launched.
settings-main-cta-update-manually = Update Warp manually

# --- ANCHOR-SUB-MCP (agent-settings-mcp) ---
# 此锚点下放 settings_view/mcp_servers_page.rs 字符串
# 命名前缀:settings-mcp-*
settings-mcp-page-title = MCP Servers
settings-mcp-logout-success-named = Successfully logged out of {$name} MCP server
settings-mcp-logout-success = Successfully logged out of MCP server
settings-mcp-install-modal-busy = Finish the current MCP install before opening another install link.
settings-mcp-unknown-server = Unknown MCP server '{$name}'
settings-mcp-install-from-link-failed = MCP server '{$name}' cannot be installed from this link.

# ---- destructive_mcp_confirmation_dialog.rs ----
settings-mcp-confirm-delete-local-title = Delete MCP server?
settings-mcp-confirm-delete-local-description = This will uninstall and remove this MCP server from this device.
settings-mcp-confirm-delete-shared-title = Delete MCP server?
settings-mcp-confirm-delete-shared-description = This removes the saved MCP server from this device.
settings-mcp-confirm-unshare-title = Remove saved MCP server?
settings-mcp-confirm-unshare-description = This removes the saved MCP server from this device.
settings-mcp-confirm-delete-button = Delete MCP
settings-mcp-confirm-remove-from-team-button = Remove saved copy
settings-mcp-confirm-cancel-button = Cancel

# ---- edit_page.rs ----
settings-mcp-edit-save = Save
settings-mcp-edit-edit-variables = Edit Variables
settings-mcp-edit-delete = Delete MCP
settings-mcp-edit-remove-from-team = Remove saved copy
settings-mcp-edit-editing-disabled-banner = This MCP server cannot be edited from this view.
settings-mcp-edit-add-new-title = Add New MCP Server
settings-mcp-edit-edit-named-title = Edit { $name } MCP Server
settings-mcp-edit-edit-title = Edit MCP Server
settings-mcp-edit-logout-tooltip = Log out
settings-mcp-edit-secrets-error = This MCP server contains secrets. Visit Settings > Privacy to modify your secret redaction settings.
settings-mcp-edit-no-server-error = No MCP Server specified.
settings-mcp-edit-multiple-servers-error = Cannot add multiple MCP servers while editing a single server.

# ---- installation_modal.rs ----
settings-mcp-install-modal-title = Install { $name }
settings-mcp-install-modal-source-shared = Saved preset
settings-mcp-install-modal-source-other-device = From another device
settings-mcp-install-modal-cancel = Cancel
settings-mcp-install-modal-install = Install
settings-mcp-install-modal-no-server = No MCP server selected

# ---- list_page.rs ----
settings-mcp-list-description = Add MCP servers to extend the Warp Agent's capabilities. MCP servers expose data sources or tools to agents through a standardized interface, essentially acting like plugins. Add a custom server, or use the presets to get started with popular servers.
settings-mcp-list-learn-more = Learn more.
settings-mcp-list-empty-state = Once you add a MCP server, it will be shown here.
settings-mcp-list-no-search-results = No search results found
settings-mcp-list-search-placeholder = Search MCP Servers
settings-mcp-list-add-button = Add
settings-mcp-list-file-based-toggle-label = Auto-spawn servers from third-party agents
settings-mcp-list-file-based-description = Automatically detect and spawn MCP servers from globally-scoped third-party AI agent configuration files (e.g. in your home directory). Servers detected inside a repository are never spawned automatically and must be enabled individually in the "Detected from" sections below.
settings-mcp-list-file-based-supported-providers = See supported providers.
settings-mcp-list-template-available-to-install = Available to install
settings-mcp-list-file-based-detected = Detected from config file
settings-mcp-list-toast-server-updated = MCP server updated
settings-mcp-list-section-my-mcps = My MCPs
settings-mcp-list-section-shared-by-warp-and-team = Available from Warp and { $name }
settings-mcp-list-section-shared-by-warp-and-other-devices = Shared by Warp and from other devices
settings-mcp-list-section-shared-from-warp = Shared from Warp
settings-mcp-list-section-detected-from = Detected from { $provider }
settings-mcp-list-chip-global = global
settings-mcp-list-chip-shared-by-creator = Shared by: { $creator }
settings-mcp-list-chip-shared-by-team-member = Saved preset
settings-mcp-list-chip-from-another-device = From another device

# ---- server_card.rs ----
settings-mcp-card-tooltip-show-logs = Show logs
settings-mcp-card-tooltip-log-out = Log out
settings-mcp-card-tooltip-share-server = Share server
settings-mcp-card-tooltip-edit = Edit
settings-mcp-card-tooltip-update-available = Server update available
settings-mcp-card-button-view-logs = View logs
settings-mcp-card-button-edit-config = Edit config
settings-mcp-card-button-set-up = Set up
settings-mcp-card-tools-none = No tools available
settings-mcp-card-tools-available = { $count } tools available
settings-mcp-card-status-offline = Offline
settings-mcp-card-status-starting = Starting server...
settings-mcp-card-status-authenticating = Authenticating...
settings-mcp-card-status-shutting-down = Shutting down...

# ---- update_modal.rs ----
settings-mcp-update-modal-default-name = Server
settings-mcp-update-modal-title = Update { $name }
settings-mcp-update-modal-description = This server has { $count } updates available, which would you like to proceed with?
settings-mcp-update-modal-publisher-another-device = another device
settings-mcp-update-modal-publisher-team-member = a local source
settings-mcp-update-modal-update-from = Update from { $publisher }
settings-mcp-update-modal-version = Version { $version }
settings-mcp-update-modal-cancel = Cancel
settings-mcp-update-modal-update = Update
settings-mcp-update-modal-no-updates = No updates available

# --- ANCHOR-SUB-PLATFORM (agent-settings-platform) ---
# 此锚点下放 settings_view/platform_page.rs 字符串
# 命名前缀:settings-platform-*
settings-platform-section-title = Agent API Keys
settings-platform-description = Create and manage API keys to allow local agents to access your Warp account.
    For more information, visit the
settings-platform-documentation-link = Documentation.
settings-platform-create-button = + Create API Key
settings-platform-modal-title-new = New API key
settings-platform-modal-title-save = Save your key
settings-platform-toast-deleted = API key deleted
settings-platform-column-name = Name
settings-platform-column-key = Key
settings-platform-column-scope = Scope
settings-platform-column-created = Created
settings-platform-column-last-used = Last used
settings-platform-column-expires-at = Expires at
settings-platform-value-never = Never
settings-platform-scope-personal = Personal
settings-platform-scope-team = Team
settings-platform-zero-state-title = No API Keys
settings-platform-zero-state-description = Create a key to manage external access to Warp
settings-platform-create-api-key-description-personal = This API key is tied to your user and can make requests against your Warp account.
settings-platform-create-api-key-description-team = This API key is tied to your team and can make requests on behalf of your team.
settings-platform-create-api-key-name-placeholder = Warp API Key
settings-platform-create-api-key-expiration-one-day = 1 day
settings-platform-create-api-key-expiration-thirty-days = 30 days
settings-platform-create-api-key-expiration-ninety-days = 90 days
settings-platform-create-api-key-label-type = Type
settings-platform-create-api-key-label-expiration = Expiration
settings-platform-create-api-key-error-no-current-team = Unable to create a team API key because there is no current team.
settings-platform-create-api-key-error-create-failed = Failed to create API key. Please try again.
settings-platform-create-api-key-secret-once = This secret key is shown only once. Copy and store it securely.
settings-platform-create-api-key-copied = Copied
settings-platform-create-api-key-done = Done
settings-platform-create-api-key-creating = Creating…
settings-platform-create-api-key-create = Create key
settings-platform-create-api-key-toast-secret-copied = Secret key copied.

# --- ANCHOR-SUB-KEYBINDINGS (agent-settings-keybindings) ---
settings-keybindings-search-placeholder = Search by name or by keys (ex. "cmd d")
settings-keybindings-conflict-warning = This shortcut conflicts with other keybinds
settings-keybindings-button-default = Default
settings-keybindings-button-cancel = Cancel
settings-keybindings-button-clear = Clear
settings-keybindings-button-save = Save
settings-keybindings-press-new-shortcut = Press new keyboard shortcut
settings-keybindings-description = Add your own custom keybindings to existing actions below.
settings-keybindings-use-prefix = Use
settings-keybindings-use-suffix = to reference these keybindings in a side pane at anytime.
settings-keybindings-not-synced-tooltip = Keyboard shortcuts are stored locally on this machine
settings-keybindings-subheader = Configure keyboard shortcuts
settings-keybindings-command-column = Command

# --- ANCHOR-SUB-REFERRALS (agent-settings-referrals) ---
settings-referrals-page-title = Invite a friend to Warp
settings-referrals-anonymous-header = Referral program is unavailable in local OpenWarp builds
settings-referrals-sign-up = Unavailable locally
settings-referrals-link-label = Link
settings-referrals-email-label = Email
settings-referrals-link-error = Failed to load referral code.
settings-referrals-loading = Loading...
settings-referrals-copy-link-button = Copy link
settings-referrals-email-send-button = Send
settings-referrals-email-sending-button = Sending...
settings-referrals-link-copied-toast = Link copied.
settings-referrals-email-success-toast = Successfully sent emails.
settings-referrals-email-failure-toast = Failed to send emails. Please try again.
settings-referrals-email-empty-error = Please enter an email.
settings-referrals-email-invalid-error = Please ensure the following email is valid: { $email }
settings-referrals-reward-intro = Get exclusive Warp goodies when you refer someone*
settings-referrals-claimed-count-singular = Current referral
settings-referrals-claimed-count-plural = Current referrals
settings-referrals-terms-link = Certain restrictions apply.
settings-referrals-terms-contact = { " " }If you have any questions about the referral program, please contact referrals@warp.dev.
settings-referrals-reward-theme = Exclusive theme
settings-referrals-reward-keycaps = Keycaps + stickers
settings-referrals-reward-tshirt = T-shirt
settings-referrals-reward-notebook = Notebook
settings-referrals-reward-cap = Baseball cap
settings-referrals-reward-hoodie = Hoodie
settings-referrals-reward-hydroflask = Premium Hydro Flask
settings-referrals-reward-backpack = Backpack

# --- ANCHOR-SUB-WARPIFY (agent-settings-warpify) ---
settings-warpify-page-title = Warpify
settings-warpify-description-prefix = Configure whether Warp attempts to "Warpify" (add support for blocks, input modes, etc) certain shells.
settings-warpify-learn-more = Learn more
settings-warpify-section-subshells = Subshells
settings-warpify-section-subshells-subtitle = Subshells supported: bash, zsh, and fish.
settings-warpify-section-ssh = SSH
settings-warpify-section-ssh-subtitle = Warpify your interactive SSH sessions.
settings-warpify-added-commands = Added commands
settings-warpify-denylisted-commands = Denylisted commands
settings-warpify-denylisted-hosts = Denylisted hosts
settings-warpify-command-placeholder = command (supports regex)
settings-warpify-host-placeholder = host (supports regex)
settings-warpify-enable-ssh = Warpify SSH Sessions
settings-warpify-install-ssh-extension = Install SSH extension
settings-warpify-install-ssh-extension-description = Controls the installation behavior for Warp's SSH extension when a remote host doesn't have it installed.
settings-warpify-use-tmux = Use Tmux Warpification
settings-warpify-tmux-description = The tmux ssh wrapper works in many situations where the default one does not, but may require you to hit a button to warpify. Takes effect in new tabs.
settings-warpify-ssh-tmux-toggle-binding-label = SSH session detection for Warpification

# --- ANCHOR-SUB-AI-PAGE (agent-settings-ai-page) ---
# Section / sub-headers
settings-ai-warp-agent-header = Warp Agent
settings-ai-active-ai-section = Active AI
settings-ai-input-section = Input
settings-ai-mcp-servers-section = MCP Servers
settings-ai-knowledge-section = Knowledge
settings-ai-voice-section = Voice
settings-ai-other-section = Other
settings-ai-third-party-cli-section = Third party CLI agents
settings-ai-experimental-section = Experimental
settings-ai-aws-bedrock-section = AWS Bedrock
settings-ai-agents-header = Agents
settings-ai-profiles-header = Profiles
settings-ai-models-subheader = Models
settings-ai-permissions-subheader = Permissions
settings-ai-usage-header = Usage
settings-ai-credits-label = Credits

# Active AI toggle labels
settings-ai-next-command-label = Next Command
settings-ai-prompt-suggestions-label = Prompt Suggestions
settings-ai-suggested-code-banners-label = Suggested Code Banners
settings-ai-natural-language-autosuggestions-label = Natural Language Autosuggestions
settings-ai-git-operations-autogen-label = Commit & Pull Request Generation

# Permissions dropdown options
settings-ai-permission-agent-decides = Agent decides
settings-ai-permission-always-allow = Always allow
settings-ai-permission-always-ask = Always ask
settings-ai-permission-ask-on-first-write = Ask on first write
settings-ai-permission-read-only = Read only
settings-ai-permission-supervised = Supervised
settings-ai-permission-allow-specific-dirs = Allow in specific directories

# Permission row labels
settings-ai-apply-code-diffs = Apply code diffs
settings-ai-read-files = Read files
settings-ai-execute-commands = Execute commands
settings-ai-interact-running-commands = Interact with running commands
settings-ai-call-mcp-servers = Call MCP servers
settings-ai-command-denylist = Command denylist
settings-ai-command-denylist-description = Regular expressions to match commands that the Warp Agent should always ask permission to execute.
settings-ai-command-allowlist = Command allowlist
settings-ai-command-allowlist-description = Regular expressions to match commands that can be automatically executed by the Warp Agent.
settings-ai-directory-allowlist = Directory allowlist
settings-ai-directory-allowlist-description = Give the agent file access to certain directories.
settings-ai-mcp-allowlist = MCP allowlist
settings-ai-mcp-allowlist-description = Allow the Warp Agent to call these MCP servers.
settings-ai-mcp-denylist = MCP denylist
settings-ai-mcp-denylist-description = The Warp Agent will always ask for permission before calling any MCP servers on this list.
settings-ai-info-banner-managed-by-workspace = Some of your permissions are managed by your workspace.

# Models / Profiles
settings-ai-base-model = Base model
settings-ai-base-model-description = This model serves as the primary engine behind the Warp Agent. It powers most interactions and invokes other models for tasks like planning or code generation when necessary. Warp may automatically switch to alternate models based on model availability or for auxiliary tasks such as conversation summarization.
settings-ai-show-model-picker-in-prompt = Show model picker in prompt
settings-ai-codebase-context = Codebase Context
settings-ai-codebase-context-description = Allow the Warp Agent to generate an outline of your codebase that can be used for context. No code is ever stored on our servers.
settings-ai-add-profile = Add Profile
settings-ai-agents-description = Set the boundaries for how your Agent operates. Choose what it can access, how much autonomy it has, and when it must ask for your approval. You can also fine-tune behavior around natural language input, codebase awareness, and more.
settings-ai-profiles-description = Profiles let you define how your Agent operates — from the actions it can take and when it needs approval, to the models it uses for tasks like coding and planning. You can also scope them to individual projects.

# Anonymous / org gates
settings-ai-sign-up = Enable local AI
settings-ai-anonymous-create-account = Local AI features do not require an account.
settings-ai-org-disallows-remote-session = Your organization disallows AI when the active pane contains content from a remote session
settings-ai-org-enforced-tooltip = This option is enforced by your organization's settings and cannot be customized.
settings-ai-restricted-billing = Restricted due to billing issue
settings-ai-unlimited = Unlimited

# AI Input section
settings-ai-show-input-hint-text = Show input hint text
settings-ai-show-agent-tips = Show agent tips
settings-ai-include-agent-commands-in-history = Include agent-executed commands in history
settings-ai-autodetect-agent-prompts = Autodetect agent prompts in terminal input
settings-ai-autodetect-terminal-commands = Autodetect terminal commands in agent input
settings-ai-natural-language-detection = Natural language detection
settings-ai-natural-language-denylist = Natural language denylist
settings-ai-natural-language-denylist-description = Commands listed here will never trigger natural language detection.
settings-ai-let-us-know = Let us know

# MCP Servers
settings-ai-learn-more = Learn more
settings-ai-add-server = Add a server
settings-ai-manage-mcp-servers = Manage MCP servers
settings-ai-file-based-mcp-toggle = Auto-spawn servers from third-party agents
settings-ai-file-based-mcp-supported-providers = See supported providers.
settings-ai-mcp-dropdown-header = Select MCP servers

# Knowledge / Rules
settings-ai-rules-label = Rules
settings-ai-suggested-rules-label = Suggested Rules
settings-ai-suggested-rules-description = Let AI suggest rules to save based on your interactions.
settings-ai-manage-rules = Manage rules
settings-ai-rules-description = Rules help the Warp Agent follow your conventions, whether for codebases or specific workflows.

# Voice
settings-ai-voice-input-label = Voice Input
settings-ai-voice-key = Key for Activating Voice Input
settings-ai-voice-key-hint = Press and hold to activate.

# Other section
settings-ai-show-use-agent-footer = Show "Use Agent" footer
settings-ai-use-agent-footer-description = Shows hint to use the "Full Terminal Use"-enabled agent in long running commands.
settings-ai-show-conversation-history = Show conversation history in tools panel
settings-ai-thinking-display = Agent thinking display
settings-ai-thinking-display-description = Controls how reasoning/thinking traces are displayed.
settings-ai-conversation-layout-label = Preferred layout when opening existing agent conversations
settings-ai-conversation-layout-newtab = New Tab
settings-ai-conversation-layout-splitpane = Split Pane
settings-ai-toolbar-layout = Toolbar layout

# Third-party CLI agents
settings-ai-show-coding-agent-toolbar = Show coding agent toolbar
settings-ai-auto-show-rich-input = Auto show/hide Rich Input based on agent status
settings-ai-auto-show-rich-input-tooltip = Requires the Warp plugin for your coding agent
settings-ai-auto-open-rich-input = Auto open Rich Input when a coding agent session starts
settings-ai-auto-dismiss-rich-input = Auto dismiss Rich Input after prompt submission
settings-ai-toolbar-commands-label = Commands that enable the toolbar
settings-ai-toolbar-commands-description = Add regex patterns to show the coding agent toolbar for matching commands.
settings-ai-coding-agent-other = Other
settings-ai-coding-agent-select-header = Select coding agent

# Experimental / Agent
settings-ai-cloud-agent-computer-use = Computer use in agents
settings-ai-cloud-agent-computer-use-description = Enable computer use in agent conversations started from the Warp app.
settings-ai-orchestration-label = Orchestration
settings-ai-orchestration-description = Enable multi-agent orchestration, allowing the agent to spawn and coordinate parallel sub-agents.

# AWS Bedrock
settings-ai-aws-bedrock-toggle = Use AWS Bedrock credentials
settings-ai-aws-bedrock-description = Warp loads and sends local AWS CLI credentials for Bedrock-supported models.
settings-ai-aws-bedrock-description-managed = Warp loads and sends local AWS CLI credentials for Bedrock-supported models. This setting is managed by your organization.
settings-ai-aws-login-command = Login Command
settings-ai-aws-profile = AWS Profile
settings-ai-aws-auto-login = Automatically run login command
settings-ai-aws-auto-login-description = When enabled, the login command will run automatically when AWS Bedrock credentials expire.
settings-ai-refresh = Refresh

# --- ANCHOR-SUB-FEATURES (agent-settings-features) ---
# settings_view/features_page.rs P0 + P1(category + toggle labels)
# 命名前缀:settings-features-*
settings-features-category-general = General
settings-features-category-session = Session
settings-features-category-keys = Keys
settings-features-category-text-editing = Text Editing
settings-features-category-terminal-input = Terminal Input
settings-features-category-terminal = Terminal
settings-features-category-notifications = Notifications
settings-features-category-workflows = Workflows
settings-features-category-system = System
settings-features-open-links-in-desktop = Open links in desktop app
settings-features-open-links-in-desktop-tooltip = Automatically open links in desktop app whenever possible.
settings-features-restore-session = Restore windows, tabs, and panes on startup
settings-features-persist-conversations = Save agent conversations to local history
settings-features-show-sticky-command-header = Show sticky command header
settings-features-show-link-tooltip = Show tooltip on click on links
settings-features-show-quit-warning = Show warning before quitting/logging out
settings-features-quit-on-last-window-closed = Quit when all windows are closed
settings-features-show-changelog-after-update = Show changelog toast after updates
settings-features-mouse-scroll-multiplier = Lines scrolled by mouse wheel interval
settings-features-auto-open-code-review = Auto open code review panel
settings-features-max-rows-per-block = Maximum rows in a block
settings-features-ssh-wrapper = Warp SSH Wrapper
settings-features-receive-desktop-notifications = Receive desktop notifications from Warp
settings-features-show-in-app-agent-notifications = Show in-app agent notifications
settings-features-confirm-close-shared-session = Confirm before closing read-only session
settings-features-global-hotkey-label = Global hotkey:
settings-features-global-hotkey-not-supported-on-wayland = Not supported on Wayland.
settings-features-autocomplete-symbols = Autocomplete quotes, parentheses, and brackets
settings-features-error-underlining = Error underlining for commands
settings-features-syntax-highlighting = Syntax highlighting for commands
settings-features-completions-while-typing = Open completions menu as you type
settings-features-command-corrections = Suggest corrected commands
settings-features-expand-aliases = Expand aliases as you type
settings-features-middle-click-paste = Middle-click to paste
settings-features-vim-mode = Edit code and commands with Vim keybindings
settings-features-at-context-menu = Enable '@' context menu in terminal mode
settings-features-slash-commands-in-terminal = Enable slash commands in terminal mode
settings-features-outline-codebase-symbols = Outline codebase symbols for '@' context menu
settings-features-show-input-message-bar = Show terminal input message line
settings-features-show-autosuggestion-hint = Show autosuggestion keybinding hint
settings-features-show-autosuggestion-ignore = Show autosuggestion ignore button
settings-features-enable-mouse-reporting = Enable Mouse Reporting
settings-features-enable-scroll-reporting = Enable Scroll Reporting
settings-features-enable-focus-reporting = Enable Focus Reporting
settings-features-use-audible-bell = Use Audible Bell
settings-features-double-click-smart-selection = Double-click smart selection
settings-features-show-help-block-in-new-sessions = Show help block in new sessions
settings-features-copy-on-select = Copy on select
settings-features-show-global-workflows-in-command-search = Show Global Workflows in Command Search (ctrl-r)
settings-features-linux-selection-clipboard = Honor linux selection clipboard
settings-features-prefer-low-power-gpu = Prefer rendering new windows with integrated GPU (low power)
settings-features-use-wayland = Use Wayland for window management
settings-features-use-wayland-tooltip = Enables the use of Wayland
settings-features-ctrl-tab-behavior-label = Ctrl+Tab behavior:
settings-features-extra-meta-key-left-mac = Left Option key is Meta
settings-features-extra-meta-key-right-mac = Right Option key is Meta
settings-features-extra-meta-key-left-other = Left Alt key is Meta
settings-features-extra-meta-key-right-other = Right Alt key is Meta
settings-features-default-shell-header = Default shell for new sessions
settings-features-working-directory-header = Working directory for new sessions
settings-features-notify-agent-task-completed = Notify when an agent completes a task
settings-features-notify-needs-attention = Notify when a command or agent needs your attention to continue
settings-features-play-notification-sounds = Play notification sounds
settings-features-default-session-mode = Default mode for new sessions
settings-features-block-rows-description = Setting the limit above 100k lines may impact performance. Maximum rows supported is { $max_rows }.
settings-features-toast-duration-label = Toast notifications stay visible for
settings-features-tab-key-behavior = Tab key behavior
settings-features-graphics-backend-label = Preferred graphics backend
settings-features-graphics-backend-current = Current backend: { $backend }
settings-features-working-dir-home = Home directory
settings-features-working-dir-previous = Previous session's directory
settings-features-working-dir-custom = Custom directory
settings-features-undo-close-enable = Enable reopening of closed sessions
settings-features-undo-close-grace-period = Grace period (seconds)
settings-features-configure-global-hotkey = Configure Global Hotkey
settings-features-make-default-terminal = Make Warp the default terminal
settings-features-pin-top = Pin to top
settings-features-pin-bottom = Pin to bottom
settings-features-pin-left = Pin to left
settings-features-pin-right = Pin to right
settings-features-default-option = Default
settings-features-tab-behavior-completions = Open completions menu
settings-features-tab-behavior-autosuggestions = Accept autosuggestion
settings-features-tab-behavior-user-defined = User defined
settings-features-new-tab-placement-all = After all tabs
settings-features-new-tab-placement-current = After current tab
settings-features-width-percent = Width %
settings-features-height-percent = Height %
settings-features-autohide-on-focus-loss = Autohides on loss of keyboard focus
settings-features-long-running-prefix = When a command takes longer than
settings-features-long-running-suffix = seconds to complete
settings-features-keybinding-label = Keybinding
settings-features-click-set-global-hotkey = Click to set global hotkey
settings-features-cancel = Cancel
settings-features-save = Save
settings-features-press-new-shortcut = Press new keyboard shortcut
settings-features-change-keybinding = Change keybinding
settings-features-active-screen = Active Screen
settings-features-wayland-window-restore-warning = Window positions won't be restored on Wayland.
settings-features-see-docs = See docs.
settings-features-allowed-values-1-20 = Allowed Values: 1-20
settings-features-supports-floating-1-20 = Supports floating point values between 1 and 20.
settings-features-auto-open-code-review-description = When this setting is on, the code review panel will open on the first accepted diff of a conversation
settings-features-default-terminal-current = Warp is the default terminal
settings-features-takes-effect-new-sessions = This change will take effect in new sessions
settings-features-seconds = seconds
settings-features-vim-system-clipboard = Set unnamed register as system clipboard
settings-features-vim-status-bar = Show Vim status bar
settings-features-tab-behavior-right-arrow-accepts = → accepts autosuggestions.
settings-features-tab-behavior-key-accepts = { $keybinding } accepts autosuggestions.
settings-features-completions-open-while-typing-sentence = Completions open as you type.
settings-features-completions-open-while-typing-or-key = Completions open as you type (or { $keybinding }).
settings-features-open-completions-unbound = Opening the completion menu is unbound.
settings-features-tab-behavior-key-opens-completions = { $keybinding } opens completion menu.
settings-features-word-characters-label = Characters considered part of a word
settings-features-new-tab-placement = New tab placement
settings-features-linux-selection-clipboard-tooltip = Whether the Linux primary clipboard should be supported.
settings-features-changes-apply-new-windows = Changes will apply to new windows.
settings-features-wayland-description = Enabling this setting disables global hotkey support. When disabled, text may be blurry if your Wayland compositor is using fraction scaling (ex: 125%).
settings-features-restart-warp-to-apply = Restart Warp for changes to take effect.

# --- ANCHOR-SUB-SETTINGS-PAGE-NAV (agent-settings-page-nav) ---
# 此锚点下放 settings_view/{settings_page,nav,delete_environment_confirmation_dialog,directory_color_add_picker,pane_manager}.rs 字符串
# 命名前缀:settings-page-* / settings-nav-* / settings-confirm-* / settings-color-picker-*

# ---- settings_page.rs ----
settings-page-info-icon-tooltip = Click to learn more in docs
settings-page-local-only-icon-tooltip = This setting is not synced to your other devices
settings-page-reset-to-default = Reset to default

# ---- delete_environment_confirmation_dialog.rs ----
settings-confirm-cancel = Cancel
settings-confirm-delete-environment-button = Delete environment
settings-confirm-delete-environment-title = Delete environment?
settings-confirm-delete-environment-description = Are you sure you want to remove the { $name } environment?

# ---- directory_color_add_picker.rs ----
settings-color-picker-add-directory-footer = + Add directory…
settings-color-picker-add-directory-color = Add directory color

# ---- settings_file_footer.rs ----
settings-footer-open-file = Open settings file
settings-footer-alert-open-file = Open file
settings-footer-alert-fix-with-oz = Fix with Oz

# --- ANCHOR-SUB-CODE (agent-settings-code) ---
settings-code-auto-open-review-panel = Auto open code review panel
settings-code-auto-open-review-panel-desc = When this setting is on, the code review panel will open on the first accepted diff of a conversation
settings-code-show-code-review-button = Show code review button
settings-code-show-code-review-button-desc = Show a button in the top right of the window to toggle the code review panel.
settings-code-show-diff-stats = Show diff stats on code review button
settings-code-show-diff-stats-desc = Show lines added and removed counts on the code review button.
settings-code-project-explorer = Project explorer
settings-code-project-explorer-desc = Adds an IDE-style project explorer / file tree to the left side tools panel.
settings-code-global-search = Global file search
settings-code-global-search-desc = Adds global file search to the left side tools panel.

# --- ANCHOR-SUB-PRIVACY (agent-settings-privacy) ---
settings-privacy-page-title = Privacy
settings-privacy-modal-add-regex-title = Add regex pattern
settings-privacy-safe-mode-title = Secret redaction
settings-privacy-safe-mode-description = When this setting is enabled, Warp will scan blocks, the contents of Warp Drive objects, and Oz prompts for potential sensitive information and prevent saving or sending this data to any servers. You can customize this list via regexes.
settings-privacy-user-secret-regex-title = Custom secret redaction
settings-privacy-user-secret-regex-description = Use regex to define additional secrets or data you'd like to redact. This will take effect when the next command runs. You can use the inline (?i) flag as a prefix to your regex to make it case-insensitive.
settings-privacy-telemetry-title = Help improve Warp
settings-privacy-telemetry-description = OpenWarp keeps diagnostics local by default. Local Agent features do not require remote collection.
settings-privacy-telemetry-description-old = OpenWarp keeps diagnostics local by default. Console input and output stay local unless you configure an external provider.
settings-privacy-telemetry-free-tier-note = Local Agent features do not require analytics.
settings-privacy-telemetry-docs-link = Read more about Warp's use of data
settings-privacy-policy-title = Privacy policy
settings-privacy-policy-link = Read Warp's privacy policy
settings-privacy-tab-personal = Personal
settings-privacy-tab-enterprise = Enterprise
settings-privacy-enterprise-readonly = Enterprise secret redaction cannot be modified.
settings-privacy-enterprise-empty = No enterprise regexes have been configured by your organization.
settings-privacy-recommended = Recommended
settings-privacy-add-all = Add all
settings-privacy-add-regex-button = Add regex
settings-privacy-enterprise-enabled-by-org = Enabled by your organization.
settings-privacy-zdr-badge = ZDR
settings-privacy-zdr-tooltip = Your administrator has enabled zero data retention for your team. User generated content will never be collected.
settings-privacy-secret-display-mode-title = Secret visual redaction mode
settings-privacy-secret-display-mode-description = Choose how secrets are visually presented in the block list while keeping them searchable. This setting only affects what you see in the block list.
settings-privacy-crash-reports-title = Send crash reports
settings-privacy-crash-reports-description = Crash reports assist with debugging and stability improvements.
settings-privacy-cloud-conv-title = Store AI conversations locally
settings-privacy-cloud-conv-description-on = Agent conversations are stored on this machine for local product functionality.
settings-privacy-cloud-conv-description-off = Agent conversations are stored locally on this machine and are removed when local data is cleared.
settings-privacy-org-managed-tooltip = This setting is managed by your organization.
settings-privacy-network-log-title = Network log console
settings-privacy-network-log-description = We've built a native console that allows you to view all communications from Warp to external servers to ensure you feel comfortable that your work is always kept safe.
settings-privacy-network-log-link = View network logging

# --- ANCHOR-SUB-EXEC-MODAL-BLOCKS (agent-settings-misc) ---
# ---- execution_profile_view ----
settings-exec-profile-edit-button = Edit
settings-exec-profile-auto = Auto
settings-exec-profile-section-models = MODELS
settings-exec-profile-section-permissions = PERMISSIONS
settings-exec-profile-base-model = Base model:
settings-exec-profile-full-terminal-use = Full terminal use:
settings-exec-profile-title-model = Title generation:
settings-exec-profile-active-ai-model = Active AI:
settings-exec-profile-next-command-model = Next Command:
settings-exec-profile-computer-use = Computer use:
settings-exec-profile-apply-code-diffs = Apply code diffs:
settings-exec-profile-read-files = Read files:
settings-exec-profile-execute-commands = Execute commands:
settings-exec-profile-interact-running-commands = Interact with running commands:
settings-exec-profile-ask-questions = Ask questions:
settings-exec-profile-call-mcp-servers = Call MCP servers:
settings-exec-profile-call-web-tools = Call web tools:
settings-exec-profile-chips-none = None
settings-exec-profile-perm-agent-decides = Agent decides
settings-exec-profile-perm-always-allow = Always allow
settings-exec-profile-perm-always-ask = Always ask
settings-exec-profile-perm-unknown = Unknown
settings-exec-profile-perm-ask-on-first-write = Ask on first write
settings-exec-profile-perm-never = Never
settings-exec-profile-perm-never-ask = Never ask
settings-exec-profile-perm-ask-unless-auto-approve = Ask unless auto-approve
settings-exec-profile-perm-on = On
settings-exec-profile-perm-off = Off
settings-exec-profile-directory-allowlist = Directory allowlist:
settings-exec-profile-command-allowlist = Command allowlist:
settings-exec-profile-command-denylist = Command denylist:
settings-exec-profile-mcp-allowlist = MCP allowlist:
settings-exec-profile-mcp-denylist = MCP denylist:

# ---- execution_profile_editor (Profile Editor pane) ----
settings-exec-profile-editor-header = Profile Editor
settings-exec-profile-editor-title = Edit Profile
settings-exec-profile-editor-name-label = Name
settings-exec-profile-editor-default-name-info = Default profile name cannot be changed.
settings-exec-profile-editor-workspace-override-tooltip = This option is enforced by your organization's settings and cannot be customized.
settings-exec-profile-editor-section-models = MODELS
settings-exec-profile-editor-section-permissions = PERMISSIONS
settings-exec-profile-editor-base-model = Base model
settings-exec-profile-editor-base-model-desc = This model serves as the primary engine behind the agent. It powers most interactions and invokes other models for tasks like planning or code generation when necessary. Warp may automatically switch to alternate models based on model availability or for auxiliary tasks such as conversation summarization.
settings-exec-profile-editor-full-terminal-use-model = Full terminal use model
settings-exec-profile-editor-full-terminal-use-model-desc = The model used when the agent operates inside interactive terminal applications like database shells, debuggers, REPLs, or dev servers—reading live output and writing commands to the PTY.
settings-exec-profile-editor-title-model = Title generation model
settings-exec-profile-editor-title-model-desc = The model used to generate concise conversation titles. Defaults to the base model — pick a cheaper/faster model here to save tokens on title summarization without affecting the agent's main reasoning.
settings-exec-profile-editor-active-ai-model = Active AI model
settings-exec-profile-editor-active-ai-model-desc = The model used by proactive AI features: prompt suggestions after a command finishes, natural-language autocomplete in the agent input, and codebase relevance ranking. Defaults to the base model — pick a small/fast model here to keep these features snappy without affecting the agent's main reasoning.
settings-exec-profile-editor-next-command-model = Next Command model
settings-exec-profile-editor-next-command-model-desc = The model used to predict your next shell command (gray inline autosuggestion + zero-state suggestion after a block finishes). Latency-sensitive — pick the smallest/fastest BYOP model you have. Defaults to the base model.
settings-exec-profile-editor-computer-use-model = Computer use model
settings-exec-profile-editor-computer-use-model-desc = The model used when the agent takes control of your computer to interact with graphical applications through mouse movements, clicks, and keyboard input.
settings-exec-profile-editor-apply-code-diffs = Apply code diffs
settings-exec-profile-editor-read-files = Read files
settings-exec-profile-editor-execute-commands = Execute commands
settings-exec-profile-editor-interact-running-commands = Interact with running commands
settings-exec-profile-editor-computer-use = Computer use
settings-exec-profile-editor-ask-questions = Ask questions
settings-exec-profile-editor-call-mcp-servers = Call MCP servers
settings-exec-profile-editor-call-web-tools = Call web tools
settings-exec-profile-editor-call-web-tools-desc = The agent may use web search when helpful for completing tasks.
settings-exec-profile-editor-directory-allowlist = Directory allowlist
settings-exec-profile-editor-directory-allowlist-desc = Give the agent file access to certain directories.
settings-exec-profile-editor-command-allowlist = Command allowlist
settings-exec-profile-editor-command-allowlist-desc = Regular expressions to match commands that can be automatically executed by Oz.
settings-exec-profile-editor-command-denylist = Command denylist
settings-exec-profile-editor-command-denylist-desc = Regular expressions to match commands that Oz should always ask permission to execute.
settings-exec-profile-editor-mcp-allowlist = MCP allowlist
settings-exec-profile-editor-mcp-allowlist-desc = MCP servers that are allowed to be called by Oz.
settings-exec-profile-editor-mcp-denylist = MCP denylist
settings-exec-profile-editor-mcp-denylist-desc = MCP servers that are not allowed to be called by Oz.

# ---- agent_assisted_environment_modal ----
settings-env-modal-add-repo = Add repo
settings-env-modal-cancel = Cancel
settings-env-modal-create-environment = Create environment
settings-env-modal-selected-repos = Selected repos
settings-env-modal-no-repos-selected = No repos selected yet
settings-env-modal-available-repos = Available indexed repos
settings-env-modal-loading = Loading locally indexed repos…
settings-env-modal-empty-no-indexed = No locally indexed repos found yet. Index a repo, then try again.
settings-env-modal-unavailable-build = Local repo selection is unavailable in this build.
settings-env-modal-all-selected = All locally indexed repos are already selected.
settings-env-modal-unknown-repo-name = (unknown)
settings-env-modal-not-git-repo = Selected folder is not a Git repository: { $path }
settings-env-modal-no-directory-selected = No directory selected
settings-env-modal-dialog-title = Select repos for your environment
settings-env-modal-dialog-description-indexed = Select locally indexed repos to provide context for the environment creation agent.
settings-env-modal-dialog-description-default = Select repos to provide context for the environment creation agent.

# ---- show_blocks_view ----
settings-show-blocks-page-title = Shared blocks
settings-show-blocks-unshare-menu-item = Unshare
settings-show-blocks-copy-link = Copy link
settings-show-blocks-deleting = Deleting...
settings-show-blocks-executed-on = Executed on: { $time }
settings-show-blocks-empty = You don't have any shared blocks yet.
settings-show-blocks-loading = Getting blocks...
settings-show-blocks-load-failed = Failed to load blocks. Please try again.
settings-show-blocks-link-copied = Link copied.
settings-show-blocks-unshare-success = Block was successfully unshared.
settings-show-blocks-unshare-failed = Failed to unshare block. Please try again.
settings-show-blocks-confirm-dialog-title = Unshare block
settings-show-blocks-confirm-dialog-text = Are you sure you want to unshare this block?

    It will no longer be accessible by link and will be permanently deleted from Warp servers.
settings-show-blocks-confirm-cancel = Cancel
settings-show-blocks-confirm-unshare = Unshare

# --- ANCHOR-SUB-APPEARANCE (agent-settings-appearance) ---
# 此锚点下放 settings_view/appearance_page.rs 剩余字符串(不含已完成的 Language widget)
# 命名前缀:settings-appearance-*

# Categories
settings-appearance-category-themes = Themes
settings-appearance-category-language = Language
settings-appearance-category-icon = Icon
settings-appearance-category-window = Window
settings-appearance-category-input = Input
settings-appearance-category-panes = Panes
settings-appearance-category-blocks = Blocks
settings-appearance-category-text = Text
settings-appearance-category-cursor = Cursor
settings-appearance-category-tabs = Tabs
settings-appearance-category-fullscreen-apps = Full-screen Apps

# Theme widget
settings-appearance-theme-create-custom = Create your own custom theme
settings-appearance-theme-mode-light = Light
settings-appearance-theme-mode-dark = Dark
settings-appearance-theme-mode-current = Current theme
settings-appearance-theme-sync-os-label = Sync with OS
settings-appearance-theme-sync-os-description = Automatically switch between light and dark themes when your system does.

# Custom App Icon widget
settings-appearance-custom-icon-label = Customize your app icon
settings-appearance-custom-icon-bundle-warning = Changing the app icon requires the app to be bundled.
settings-appearance-custom-icon-restart-warning = You may need to restart Warp for MacOS to apply the preferred icon style.

# Window widgets
settings-appearance-window-custom-size-label = Open new windows with custom size
settings-appearance-window-columns-label = Columns
settings-appearance-window-rows-label = Rows
settings-appearance-window-opacity-label = Window Opacity:
settings-appearance-window-opacity-value = Window Opacity: { $value }
settings-appearance-window-opacity-not-supported = Transparency is not supported with your graphics drivers.
settings-appearance-window-opacity-graphics-warning = The selected graphics settings may not support rendering transparent windows.
settings-appearance-window-opacity-graphics-warning-hint = Try changing the settings for the graphics backend or integrated GPU in Features > System.
settings-appearance-window-blur-radius = Window Blur Radius: { $value }
settings-appearance-window-blur-texture-label = Use Window Blur (Acrylic texture)
settings-appearance-window-blur-texture-not-supported = The selected hardware may not support rendering transparent windows.
settings-appearance-tools-panel-consistent-label = Tools panel visibility is consistent across tabs

# Input
settings-appearance-input-type-label = Input type
settings-appearance-input-type-warp = Warp
settings-appearance-input-type-shell = Shell (PS1)
settings-appearance-input-position-label = Input position
settings-appearance-input-mode-pinned-bottom = Pin to the bottom (Warp mode)
settings-appearance-input-mode-pinned-top = Pin to the top (Reverse mode)
settings-appearance-input-mode-waterfall = Start at the top (Classic mode)

# Panes
settings-appearance-pane-dim-inactive-label = Dim inactive panes
settings-appearance-pane-focus-follows-mouse-label = Focus follows mouse

# Blocks
settings-appearance-block-compact-label = Compact mode
settings-appearance-block-jump-bottom-label = Show Jump to Bottom of Block button
settings-appearance-block-show-dividers-label = Show block dividers

# Text / Fonts
settings-appearance-font-agent-label = Agent font
settings-appearance-font-match-terminal = Match terminal
settings-appearance-font-terminal-label = Terminal font
settings-appearance-font-view-all-system = View all available system fonts
settings-appearance-font-weight-label = Font weight
settings-appearance-font-size-label = Font size (px)
settings-appearance-font-line-height-label = Line height
settings-appearance-font-reset-default = Reset to default
settings-appearance-font-notebook-size-label = Notebook font size
settings-appearance-font-thin-strokes-label = Use thin strokes
settings-appearance-font-thin-strokes-never = Never
settings-appearance-font-thin-strokes-low-dpi = On low-DPI displays
settings-appearance-font-thin-strokes-high-dpi = On high-DPI displays
settings-appearance-font-thin-strokes-always = Always
settings-appearance-font-min-contrast-label = Enforce minimum contrast
settings-appearance-font-min-contrast-always = Always
settings-appearance-font-min-contrast-named-only = Only for named colors
settings-appearance-font-min-contrast-never = Never
settings-appearance-font-ligatures-label = Show ligatures in terminal
settings-appearance-font-ligatures-perf-tooltip = Ligatures may reduce performance

# Cursor
settings-appearance-cursor-type-label = Cursor type
settings-appearance-cursor-disabled-vim = Cursor type is disabled in Vim mode
settings-appearance-cursor-blink-label = Blinking cursor

# Tabs
settings-appearance-tab-close-position-label = Tab close button position
settings-appearance-tab-close-position-right = Right
settings-appearance-tab-close-position-left = Left
settings-appearance-tab-show-indicators-label = Show tab indicators
settings-appearance-tab-show-code-review-label = Show code review button
settings-appearance-tab-preserve-active-color-label = Preserve active tab color for new tabs
settings-appearance-tab-vertical-layout-label = Use vertical tab layout
settings-appearance-tab-show-vertical-panel-in-restored-windows-label = Show vertical tabs panel in restored windows
settings-appearance-tab-show-vertical-panel-in-restored-windows-description = When enabled, reopening or restoring a window opens the vertical tabs panel even if it was closed when the window was last saved.
settings-appearance-tab-show-title-bar-search-bar-label = Show search bar in title bar
settings-appearance-tab-show-title-bar-search-bar-description = Show the "Search sessions, agents, files…" search bar in the middle of the title bar; click to open the command palette. Disable to leave the slot empty. Only applies to vertical tabs layout.
workspace-title-bar-search-placeholder = Search sessions, agents, files...
settings-appearance-tab-use-prompt-as-title-label = Use latest user prompt as conversation title in tab names
settings-appearance-tab-use-prompt-as-title-description = Show the latest user prompt instead of the generated conversation title for built-in AI and third-party agent sessions in vertical tabs.
settings-appearance-tab-toolbar-layout-label = Header toolbar layout
settings-appearance-tab-directory-colors-label = Directory tab colors
settings-appearance-tab-directory-colors-description = Automatically color tabs based on the directory or repo you're working in.
settings-appearance-tab-directory-color-default-tooltip = Default (no color)
settings-appearance-zen-mode-label = Show the tab bar
settings-appearance-zen-decoration-always = Always
settings-appearance-zen-decoration-windowed = When windowed
settings-appearance-zen-decoration-on-hover = Only on hover

# Full-screen apps
settings-appearance-alt-screen-padding-label = Use custom padding in alt-screen
settings-appearance-alt-screen-uniform-padding-label = Uniform padding (px)

# Zoom
settings-appearance-zoom-label = Zoom
settings-appearance-zoom-secondary = Adjusts the default zoom level across all windows

# --- ANCHOR-SUB-ENVIRONMENTS (agent-settings-environments) ---
settings-environments-page-title = Environments
settings-environments-page-description = Environments define where your ambient agents run. Set one up in minutes via GitHub (recommended), Warp-assisted setup, or manual configuration.
settings-environments-search-placeholder = Search environments...
settings-environments-no-matches = No environments match your search.
settings-environments-section-personal = Personal
settings-environments-section-team-default = Provided by Warp and this device
settings-environments-section-team-named = Shared by Warp and { $team }
settings-environments-env-id-prefix = Env ID: { $id }
settings-environments-detail-image = Image: { $image }
settings-environments-detail-repos = Repos: { $repos }
settings-environments-detail-setup-commands = Setup commands: { $commands }
settings-environments-last-edited = Last edited: { $time }
settings-environments-last-used = Last used: { $time }
settings-environments-last-used-never = Last used: never
settings-environments-view-my-runs = View my runs
settings-environments-tooltip-share = Share
settings-environments-tooltip-edit = Edit
settings-environments-empty-header = You haven’t set up any environments yet.
settings-environments-empty-subheader = Choose how you’d like to set up your environment:
settings-environments-empty-quick-setup-title = Quick setup
settings-environments-empty-suggested-badge = Suggested
settings-environments-empty-quick-setup-subtitle = Select the GitHub repositories you’d like to work with and we’ll suggest a base image and config
settings-environments-empty-use-agent-title = Use the agent
settings-environments-empty-use-agent-subtitle = Choose a locally set up project and we’ll help you set up an environment based on it
settings-environments-button-loading = Loading...
settings-environments-button-retry = Retry
settings-environments-button-authorize = Authorize
settings-environments-button-get-started = Get started
settings-environments-button-launch-agent = Launch agent
settings-environments-toast-update-success = Successfully updated environment
settings-environments-toast-create-success = Successfully created environment
settings-environments-toast-delete-success = Environment deleted successfully
settings-environments-toast-share-success = Successfully shared environment
settings-environments-toast-share-failure = Failed to share environment with team
settings-environments-toast-create-not-logged-in = Unable to create environment: not logged in.
settings-environments-toast-save-not-found = Unable to save: environment no longer exists.
settings-environments-toast-share-no-team = Unable to share environment: you are not currently on a team.
settings-environments-toast-share-not-synced = Unable to share environment: environment is not yet synced.
settings-update-environment-name-placeholder = Environment name
settings-update-environment-docker-image-placeholder = e.g. python:3.11, node:20-alpine
settings-update-environment-repos-placeholder-authed = Enter repos (owner/repo format)
settings-update-environment-repos-placeholder-unauthenticated = Paste repo URL(s)
settings-update-environment-setup-command-placeholder = e.g. cd my-repo && pip install -r requirements.txt
settings-update-environment-description-placeholder = e.g., this environment is for all front end focused agents

# --- ANCHOR-SUB-AGENT-PROVIDERS (agent-settings-agent-providers) ---
# 此锚点下放 settings_view/agent_providers_widget.rs 字符串
# 命名前缀:settings-agent-providers-*
settings-agent-providers-title = Agent providers
settings-agent-providers-description = Configure custom Agent providers across multiple protocols — OpenAI-compatible (DeepSeek, Zhipu GLM, Moonshot, DashScope, SiliconFlow, OpenRouter, etc.), Anthropic, Gemini, and local Ollama. You can add models manually (display name + model ID mapping) or fetch them automatically from the API. Provider metadata is stored in the local settings.toml; API keys are stored securely in the system keychain.
settings-agent-providers-empty = No providers configured yet. Click [+ Add provider] in the top-right to add one.
settings-agent-providers-add-button = + Add provider
settings-agent-providers-search-placeholder = Search providers…
settings-agent-providers-quick-add-title = Quick add
settings-agent-providers-refresh-catalog = Refresh catalog
settings-agent-providers-loading-catalog = Loading models.dev catalog… (the first load may take a few seconds)
settings-agent-providers-catalog-empty = models.dev catalog is empty. Click [Refresh catalog] to retry.
settings-agent-providers-no-match = No match for "{ $query }"
settings-agent-providers-collapse = Collapse ▲
settings-agent-providers-expand-remaining = Expand remaining { $count } ▼
settings-agent-providers-row-missing = (no editors bound for this provider yet: { $id })
settings-agent-providers-field-name = Name
settings-agent-providers-field-base-url = Base URL
settings-agent-providers-field-api-key = API Key
settings-agent-providers-field-api-type = API Type
settings-agent-providers-api-type-hint = (genai uses this to bind the adapter explicitly, avoiding misdetection by model name. If Base URL is empty, the default will be used: { $url })
settings-agent-providers-name-placeholder = Custom provider name (e.g. DeepSeek, local Ollama)
settings-agent-providers-api-key-placeholder = sk-... (optional, leave empty for local providers like ollama)
settings-agent-providers-models-label = Models ({ $count })
settings-agent-providers-models-empty-hint = No models configured yet. Click [+ Add model] to add manually, or [Fetch from API] to fetch automatically.
settings-agent-providers-models-header-name = Display name
settings-agent-providers-models-header-id = Model ID
settings-agent-providers-models-header-context = Context (tok)
settings-agent-providers-models-header-output = Output (tok)
settings-agent-providers-model-name-placeholder = Display name (e.g. DS-V3 General)
settings-agent-providers-model-id-placeholder = Model ID (the `model` field sent to the API, e.g. deepseek-chat)
settings-agent-providers-model-context-placeholder = Context (tokens)
settings-agent-providers-model-output-placeholder = Output (tokens)
settings-agent-providers-add-model = + Add model
settings-agent-providers-fetch-from-api = Fetch from API
settings-agent-providers-sync-models-dev = Sync from models.dev
settings-agent-providers-remove = Remove
settings-agent-providers-save = Save
settings-agent-providers-saved-toast = Saved

# ---- AI page (settings_view/ai_page.rs) ----
settings-ai-title = AI
settings-ai-active-ai = Active AI
settings-ai-input-autodetection = terminal command autodetection in agent input
settings-ai-input-autodetection-legacy = natural language detection
settings-ai-next-command-description = Let AI suggest the next command to run based on your command history, outputs, and common workflows.
settings-ai-prompt-suggestions-description = Let AI suggest natural language prompts, as inline banners in the input, based on recent commands and their outputs.
settings-ai-suggested-code-banners-description = Let AI suggest code diffs and queries as inline banners in the blocklist, based on recent commands and their outputs.
settings-ai-natural-language-autosuggestions = Let AI suggest natural language autosuggestions, based on recent commands and their outputs.
settings-ai-git-operations-autogen-description = Let AI generate commit messages and pull request titles and descriptions.

# =============================================================================
# SECTION: ai (Owner: agent-ai)
# Files: app/src/ai/**, app/src/ai_assistant/**
# =============================================================================

# (placeholder — to be filled by agent-ai)

# =============================================================================
# SECTION: command-palette (Owner: agent-cmdpal)
# Files: app/src/command_palette.rs, app/src/palette/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: drive (Owner: agent-drive)
# Files: app/src/drive/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: workspace (Owner: agent-workspace)
# Files: app/src/workspace/**, app/src/workspaces/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: modal (Owner: agent-modal)
# Files: app/src/modal/**, app/src/prompt/**, app/src/quit_warning/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: auth (Owner: agent-auth)
# Files: app/src/auth/**
# =============================================================================

# (placeholder)

# =============================================================================
# SECTION: banner (Owner: agent-banner)
# Files: app/src/banner/**
# =============================================================================

banner-dont-show-again = Don't show me again

# =============================================================================
# SECTION: quit-warning (Owner: agent-quit-warning)
# Files: app/src/quit_warning/mod.rs
# =============================================================================

# ---- Dialog titles ----
quit-warning-title-pane = Close pane?
quit-warning-title-tab-singular = Close tab?
quit-warning-title-tab-plural = Close tabs?
quit-warning-title-window = Close window?
quit-warning-title-app = Quit Warp?
quit-warning-title-editor-tab = Save changes?

# ---- Buttons ----
quit-warning-button-confirm-close = Yes, close
quit-warning-button-confirm-quit = Yes, quit
quit-warning-button-save = Save
quit-warning-button-discard = Don't Save
quit-warning-button-show-processes = Show running processes
quit-warning-button-cancel = Cancel

# ---- Warning body lines ----
# Suffix appended to each warning line, indicating the scope.
quit-warning-suffix-tab = { " " }in this tab.
quit-warning-suffix-window = { " " }in this window.
quit-warning-suffix-pane = { " " }in this pane.
quit-warning-suffix-default = .

# Process info: "{count} process(es) running" with optional window/tab qualifier.
quit-warning-processes-running = You have { $count } { $count ->
        [one] process
       *[other] processes
    } running
quit-warning-processes-in-windows = { " " }in { $count } windows
quit-warning-processes-in-tabs = { " " }in { $count } tabs

# Shared sessions line.
quit-warning-shared-sessions = You are sharing { $count } { $count ->
        [one] session
       *[other] sessions
    }

# Unsaved code changes (generic scope).
quit-warning-unsaved-changes = You have unsaved file changes

# Unsaved code changes for a specific editor tab.
quit-warning-unsaved-editor-tab = Do you want to save the changes you made to { $file }? Your changes will be discarded if you don't save them.
quit-warning-unsaved-editor-tab-fallback-name = this file

# --- ANCHOR-SUB-RULES-PAGE (agent-rules-page) ---
# Manage Rules 页面(Warp Drive 中的 AI Fact Collection)。
rules-collection-name = Rules

# --- ANCHOR-SUB-KEYBINDING-DESC (agent-keybinding-descriptions) ---
# Description 文案 for keyboard binding entries shown in the Settings >
# Keyboard Shortcuts page and the command palette. Each key corresponds to
# a binding registered via `EditableBinding::new(name, description, action)`
# or `BindingDescription::new("…")`. The binding `name` (e.g.
# `workspace:open_settings_file`) is **not** translated — it is a protocol
# field used to persist user-customised shortcuts.

# Tabs / sessions
keybinding-desc-workspace-cycle-next-session = Switch to next tab
keybinding-desc-workspace-cycle-prev-session = Switch to previous tab
keybinding-desc-workspace-add-window = Create New Window
keybinding-desc-workspace-new-file = New File
keybinding-desc-workspace-zoom-in = Zoom In
keybinding-desc-workspace-zoom-out = Zoom Out
keybinding-desc-workspace-reset-zoom = Reset Zoom
keybinding-desc-workspace-increase-font-size = Increase font size
keybinding-desc-workspace-decrease-font-size = Decrease font size
keybinding-desc-workspace-reset-font-size = Reset font size to default
keybinding-desc-workspace-increase-zoom = Increase zoom level
keybinding-desc-workspace-decrease-zoom = Decrease zoom level
keybinding-desc-workspace-reset-zoom-level = Reset zoom level to default
keybinding-desc-workspace-save-launch-config = Save new launch configuration

# Project Explorer / panels
keybinding-desc-workspace-toggle-project-explorer = Toggle project explorer
keybinding-desc-workspace-toggle-project-explorer-menu = Project Explorer
keybinding-desc-workspace-show-theme-chooser = Open theme picker
keybinding-desc-workspace-toggle-tab-configs-menu = Open tab configs menu

# Switch to N-th tab
keybinding-desc-workspace-activate-1st-tab = Switch to 1st tab
keybinding-desc-workspace-activate-2nd-tab = Switch to 2nd tab
keybinding-desc-workspace-activate-3rd-tab = Switch to 3rd tab
keybinding-desc-workspace-activate-4th-tab = Switch to 4th tab
keybinding-desc-workspace-activate-5th-tab = Switch to 5th tab
keybinding-desc-workspace-activate-6th-tab = Switch to 6th tab
keybinding-desc-workspace-activate-7th-tab = Switch to 7th tab
keybinding-desc-workspace-activate-8th-tab = Switch to 8th tab
keybinding-desc-workspace-activate-last-tab = Switch to last tab
keybinding-desc-workspace-activate-prev-tab = Activate previous tab
keybinding-desc-workspace-activate-next-tab = Activate next tab

# Pane navigation
keybinding-desc-pane-group-navigate-prev = Activate previous pane
keybinding-desc-pane-group-navigate-next = Activate next pane

# Mouse / Notebooks / Workflows / Folders
keybinding-desc-workspace-toggle-mouse-reporting = Toggle Mouse Reporting
keybinding-desc-workspace-create-personal-notebook = Create a new personal notebook
keybinding-desc-workspace-create-personal-notebook-menu = New Personal Notebook
keybinding-desc-workspace-create-personal-workflow = Create a new personal workflow
keybinding-desc-workspace-create-personal-workflow-menu = New Personal Workflow
keybinding-desc-workspace-create-personal-folder = Create a new personal folder
keybinding-desc-workspace-create-personal-folder-menu = New Personal Folder

# New tab variants
keybinding-desc-workspace-new-tab = Create new tab
keybinding-desc-workspace-new-terminal-tab = New Terminal Tab
keybinding-desc-workspace-new-agent-tab = New Agent Tab
keybinding-desc-workspace-new-cloud-agent-tab = New Agent Tab
new-session-create-new-tab = Create New Tab
new-session-create-new-window = Create New Window
new-session-split-pane-down = Split Pane Down
new-session-split-pane-right = Split Pane Right
new-session-split-pane-up = Split Pane Up
new-session-split-pane-left = Split Pane Left
new-session-create-new-tab-with-shell = Create New Tab: { $shell }
new-session-create-new-window-with-shell = Create New Window: { $shell }
new-session-split-pane-with-shell = Split Pane { $direction }: { $shell }
new-session-direction-down = Down
new-session-direction-right = Right
new-session-direction-up = Up
new-session-direction-left = Left

# Left / right panel toggles
keybinding-desc-workspace-toggle-left-panel = Open Left Panel
keybinding-desc-workspace-toggle-right-panel = Toggle code review
keybinding-desc-workspace-toggle-right-panel-menu = Toggle Code Review
keybinding-desc-workspace-toggle-vertical-tabs = Toggle vertical tabs panel
keybinding-desc-workspace-toggle-vertical-tabs-menu = Toggle Vertical Tabs Panel
keybinding-desc-workspace-left-panel-agent-conversations = Left Panel: Agent conversations
keybinding-desc-workspace-left-panel-project-explorer = Left Panel: Project explorer
keybinding-desc-workspace-left-panel-global-search = Left Panel: Global search
keybinding-desc-workspace-left-panel-warp-drive = Left Panel: Warp Drive
keybinding-desc-workspace-left-panel-ssh-manager = Left Panel: SSH Manager
keybinding-desc-workspace-left-panel-skill-manager = Left Panel: Skill Manager
keybinding-desc-workspace-open-global-search = Open global search
keybinding-desc-workspace-open-global-search-menu = Global Search
keybinding-desc-workspace-toggle-warp-drive = Toggle Warp Drive
keybinding-desc-workspace-toggle-warp-drive-menu = Warp Drive
keybinding-desc-workspace-toggle-conversation-list-view = Toggle Agent conversation list view
keybinding-desc-workspace-toggle-conversation-list-view-menu = Agent conversation list view
keybinding-desc-workspace-close-panel = Close focused panel

# Command palette / navigation
keybinding-desc-workspace-toggle-command-palette = Toggle command palette
keybinding-desc-workspace-toggle-command-palette-menu = Command Palette
keybinding-desc-workspace-toggle-navigation-palette = Toggle navigation palette
keybinding-desc-workspace-toggle-navigation-palette-menu = Navigation Palette
keybinding-desc-workspace-toggle-launch-config-palette = Launch configuration palette
keybinding-desc-workspace-toggle-files-palette = Toggle Files Palette
keybinding-desc-workspace-search-drive = Search Warp Drive
keybinding-desc-workspace-move-tab-left = Move tab left
keybinding-desc-workspace-move-tab-up = move tab up
keybinding-desc-workspace-move-tab-right = Move tab right
keybinding-desc-workspace-move-tab-down = move tab down

# Keybindings settings
keybinding-desc-workspace-toggle-keybindings-page = Toggle keyboard shortcuts
keybinding-desc-workspace-show-keybinding-settings = Open keybindings editor
keybinding-desc-workspace-toggle-block-snackbar = Toggle sticky command header

# Window / tab close
keybinding-desc-workspace-rename-active-tab = Rename the current tab
keybinding-desc-workspace-terminate-app = Quit Warp
keybinding-desc-workspace-close-window = Close Window
keybinding-desc-workspace-close-active-tab = Close the current tab
keybinding-desc-workspace-close-other-tabs = Close other tabs
keybinding-desc-workspace-close-tabs-right = Close tabs to the right
keybinding-desc-workspace-close-tabs-below = close tabs below

# Notifications
keybinding-desc-workspace-toggle-notifications-on = Turn notifications on
keybinding-desc-workspace-toggle-notifications-off = Turn notifications off

# Updates / changelog
keybinding-desc-workspace-update-and-relaunch = Install update and relaunch
keybinding-desc-workspace-check-for-updates = Check for updates
keybinding-desc-workspace-view-changelog = View latest changelog

# Resource center / Drive export / CLI
keybinding-desc-workspace-toggle-resource-center = Toggle resource center
keybinding-desc-workspace-export-all-warp-drive-objects = Export all Warp Drive objects
keybinding-desc-workspace-install-cli = Install Oz CLI command
keybinding-desc-workspace-uninstall-cli = Uninstall Oz CLI command

# AI assistant / agents
keybinding-desc-workspace-toggle-ai-assistant = Toggle Warp AI

# Env vars / prompts
keybinding-desc-workspace-create-personal-env-vars = Create new personal environment variables
keybinding-desc-workspace-create-personal-env-vars-menu = New Personal Environment Variables
keybinding-desc-workspace-create-personal-ai-prompt = Create a new personal prompt
keybinding-desc-workspace-create-personal-ai-prompt-menu = New Personal Prompt

# Focus / import
keybinding-desc-workspace-shift-focus-left = Switch Focus to Left Panel
keybinding-desc-workspace-shift-focus-right = Switch Focus to Right Panel
keybinding-desc-workspace-import-to-personal-drive = Import To Personal Drive

# Drive / repository / AI rules / MCP
keybinding-desc-workspace-open-repository = Open repository
keybinding-desc-workspace-open-repository-menu = Open Repository
keybinding-desc-workspace-open-ai-fact-collection = Open AI Rules
keybinding-desc-workspace-open-mcp-servers = Open MCP Servers
keybinding-desc-workspace-jump-to-latest-toast = Jump to latest agent task
keybinding-desc-workspace-toggle-notification-mailbox = Toggle notification mailbox
keybinding-desc-workspace-toggle-agent-management-view = Toggle the agent management view

# Settings pages
keybinding-desc-workspace-show-settings = Open Settings
keybinding-desc-workspace-show-settings-menu = Settings
keybinding-desc-workspace-show-settings-account = Open Settings: Account
keybinding-desc-workspace-show-settings-appearance = Open Settings: Appearance
keybinding-desc-workspace-show-settings-appearance-menu = Appearance...
keybinding-desc-workspace-show-settings-features = Open Settings: Features
keybinding-desc-workspace-show-settings-shared-blocks = Open Settings: Shared Blocks
keybinding-desc-workspace-show-settings-shared-blocks-menu = View Shared Blocks...
keybinding-desc-workspace-show-settings-keyboard-shortcuts = Open Settings: Keyboard Shortcuts
keybinding-desc-workspace-show-settings-keyboard-shortcuts-menu = Configure Keyboard Shortcuts...
keybinding-desc-workspace-show-settings-about = Open Settings: About
keybinding-desc-workspace-show-settings-about-menu = About Warp
keybinding-desc-workspace-show-settings-privacy = Open Settings: Privacy
keybinding-desc-workspace-show-settings-warpify = Open Settings: Warpify
keybinding-desc-workspace-show-settings-warpify-menu = Configure Warpify...
keybinding-desc-workspace-show-settings-ai = Open Settings: AI
keybinding-desc-workspace-show-settings-code = Open Settings: Code
keybinding-desc-workspace-show-settings-referrals = Open Settings: Referrals
keybinding-desc-workspace-show-settings-environments = Open Settings: Environments
keybinding-desc-workspace-show-settings-mcp-servers = Open Settings: MCP Servers
keybinding-desc-workspace-open-settings-file = Open settings file

# Overflow menu / external links
keybinding-desc-workspace-link-to-slack = Join our Slack community (opens external link)
keybinding-desc-workspace-link-to-user-docs = View user docs (opens external link)
keybinding-desc-workspace-send-feedback = Send feedback (opens external link)
keybinding-desc-workspace-send-feedback-oz = Send feedback with Oz
keybinding-desc-workspace-view-logs = View Warp logs
keybinding-desc-workspace-link-to-privacy-policy = View privacy policy (opens external link)

# Input / terminal / project bindings (registered outside workspace/mod.rs)
keybinding-desc-input-edit-prompt = Edit Prompt
keybinding-desc-terminal-attach-block-as-context = Attach Selected Block as Agent Context
keybinding-desc-terminal-attach-text-as-context = Attach Selected Text as Agent Context
keybinding-desc-terminal-attach-as-context-menu = Attach Selection as Agent Context
keybinding-desc-workspace-init-project = Initiate project for warp
keybinding-desc-workspace-add-current-folder = Add current folder as project

# Workspace debug / crash / sentry / heap profile bindings
keybinding-desc-workspace-crash-macos = Crash the app (for testing sentry-cocoa)
keybinding-desc-workspace-crash-other = Crash the app (for testing sentry-native)
keybinding-desc-workspace-log-review-comment-send-status = [Debug] Log review comment send status for active tab
keybinding-desc-workspace-panic = Trigger a panic (for testing sentry-rust)
keybinding-desc-workspace-open-view-tree-debugger = Open view tree debugger
keybinding-desc-workspace-view-first-time-user-experience = [Debug] View first-time user experience
keybinding-desc-workspace-undismiss-aws-login-banner = [Debug] Un-dismiss AWS login banner
keybinding-desc-workspace-open-oz-launch-modal = [Debug] Open Oz Launch Modal
keybinding-desc-workspace-reset-oz-launch-modal-state = [Debug] Reset Oz Launch Modal State
keybinding-desc-workspace-open-openwarp-launch-modal = [Debug] Open OpenWarp Launch Modal
keybinding-desc-workspace-reset-openwarp-launch-modal-state = [Debug] Reset OpenWarp Launch Modal State
keybinding-desc-workspace-install-opencode-warp-plugin = [Debug] Install OpenCode Warp plugin
keybinding-desc-workspace-use-local-opencode-warp-plugin = [Debug] Use local OpenCode Warp plugin (testing only)
keybinding-desc-workspace-open-session-config-modal = [Debug] Open Session Config Modal
keybinding-desc-workspace-start-hoa-onboarding-flow = [Debug] Start HOA Onboarding Flow
keybinding-desc-workspace-sample-process = Sample Process
keybinding-desc-workspace-dump-heap-profile = Dump heap profile (can only be done once)

# Terminal input bindings
keybinding-desc-input-show-network-log = Show Warp network log
keybinding-desc-input-clear-screen = Clear screen
keybinding-desc-input-toggle-classic-completions = (Experimental) Toggle classic completions mode
keybinding-desc-input-command-search = Command Search
keybinding-desc-input-history-search = History Search
keybinding-desc-input-open-completions-menu = Open completions menu
keybinding-desc-input-workflows = Workflows
keybinding-desc-input-open-ai-command-suggestions = Open AI Command Suggestions
keybinding-desc-input-new-agent-conversation = New agent conversation
keybinding-desc-input-trigger-auto-detection = Trigger Auto Detection
keybinding-desc-input-clear-and-reset-ai-context-menu-query = Clear and reset AI context menu query

# Terminal view bindings
keybinding-desc-terminal-alternate-paste = Alternate terminal paste
keybinding-desc-terminal-toggle-cli-agent-rich-input = Toggle CLI Agent Rich Input
keybinding-desc-terminal-warpify-subshell = Warpify subshell
keybinding-desc-terminal-warpify-ssh-session = Warpify ssh session
keybinding-desc-terminal-accept-prompt-suggestion = Accept Prompt Suggestion
keybinding-desc-terminal-cancel-process-windows = Copy text or cancel active process
keybinding-desc-terminal-cancel-process = Cancel active process
keybinding-desc-terminal-focus-input = Focus terminal input
keybinding-desc-terminal-paste = Paste
keybinding-desc-terminal-copy = Copy
keybinding-desc-terminal-reinput-commands = Reinput selected commands
keybinding-desc-terminal-reinput-commands-sudo = Reinput selected commands as root
keybinding-desc-terminal-find = Find in Terminal
keybinding-desc-terminal-select-bookmark-up = Select the closest bookmark up
keybinding-desc-terminal-select-bookmark-down = Select the closest bookmark down
keybinding-desc-terminal-open-block-context-menu = Open block context menu
keybinding-desc-terminal-toggle-workflows-modal = Toggle workflows modal
keybinding-desc-terminal-copy-git-branch = Copy git branch
keybinding-desc-terminal-clear-blocks = Clear Blocks
keybinding-desc-terminal-cursor-word-left = Move cursor one word to the left within an executing command
keybinding-desc-terminal-cursor-word-right = Move cursor one word to the right within an executing command
keybinding-desc-terminal-cursor-home = Move cursor home within an executing command
keybinding-desc-terminal-cursor-end = Move cursor end within an executing command
keybinding-desc-terminal-delete-word-left = Delete word left within an executing command
keybinding-desc-terminal-delete-line-start = Delete to line start within an executing command
keybinding-desc-terminal-delete-line-end = Delete to line end within an executing command
keybinding-desc-terminal-backward-tabulation = Backward tabulation within an executing command
keybinding-desc-terminal-select-previous-block = Select previous block
keybinding-desc-terminal-select-next-block = Select next block
keybinding-desc-terminal-share-selected-block = Share selected block
keybinding-desc-terminal-bookmark-selected-block = Bookmark selected block
keybinding-desc-terminal-find-within-selected-block = Find within selected block
keybinding-desc-terminal-copy-command-and-output = Copy command and output
keybinding-desc-terminal-copy-command-output = Copy command output
keybinding-desc-terminal-copy-command = Copy command
keybinding-desc-terminal-scroll-up-one-line = Scroll terminal output up one line
keybinding-desc-terminal-scroll-down-one-line = Scroll terminal output down one line
keybinding-desc-terminal-scroll-up-one-page = Scroll terminal output up one page
keybinding-desc-terminal-scroll-down-one-page = Scroll terminal output down one page
keybinding-desc-terminal-scroll-to-top-of-block = Scroll to top of selected block
keybinding-desc-terminal-scroll-to-bottom-of-block = Scroll to bottom of selected block
keybinding-desc-terminal-select-all-blocks = Select all blocks
keybinding-desc-terminal-expand-blocks-above = Expand selected blocks above
keybinding-desc-terminal-expand-blocks-below = Expand selected blocks below
keybinding-desc-terminal-insert-command-correction = Insert Command Correction
keybinding-desc-terminal-setup-guide = Setup Guide
keybinding-desc-terminal-onboarding-warp-input-terminal = [Debug] Onboarding Callout: WarpInput - Terminal
keybinding-desc-terminal-onboarding-warp-input-project = [Debug] Onboarding Callout: WarpInput - Project
keybinding-desc-terminal-onboarding-warp-input-no-project = [Debug] Onboarding Callout: WarpInput - No Project
keybinding-desc-terminal-onboarding-modality-project = [Debug] Onboarding Callout: Modality - Project
keybinding-desc-terminal-onboarding-modality-no-project = [Debug] Onboarding Callout: Modality - No Project
keybinding-desc-terminal-onboarding-modality-terminal = [Debug] Onboarding Callout: Modality - Terminal
keybinding-desc-terminal-import-external-settings = Import External Settings
keybinding-desc-terminal-share-current-session = Share current session
keybinding-desc-terminal-stop-sharing-current-session = Stop sharing current session
keybinding-desc-terminal-toggle-block-filter = Toggle block filter on selected or last block
keybinding-desc-terminal-toggle-sticky-command-header = Toggle Sticky Command Header in Active Pane
keybinding-desc-terminal-toggle-autoexecute-mode = Toggle Auto-execute Mode
keybinding-desc-terminal-toggle-queue-next-prompt = Toggle Queue Next Prompt

# Pane group bindings
keybinding-desc-pane-group-close-current-session = Close Current Session
keybinding-desc-pane-group-split-left = Split pane left
keybinding-desc-pane-group-split-up = Split pane up
keybinding-desc-pane-group-split-down = Split pane down
keybinding-desc-pane-group-split-right = Split pane right
keybinding-desc-pane-group-switch-left = Switch panes left
keybinding-desc-pane-group-switch-right = Switch panes right
keybinding-desc-pane-group-switch-up = Switch panes up
keybinding-desc-pane-group-switch-down = Switch panes down
keybinding-desc-pane-group-resize-left = Resize pane > Move divider left
keybinding-desc-pane-group-resize-right = Resize pane > Move divider right
keybinding-desc-pane-group-resize-up = Resize pane > Move divider up
keybinding-desc-pane-group-resize-down = Resize pane > Move divider down
keybinding-desc-pane-group-toggle-maximize = Toggle Maximize Active Pane

# Root view bindings
keybinding-desc-root-view-toggle-fullscreen = Toggle fullscreen
keybinding-desc-root-view-enter-onboarding-state = [Debug] Enter Onboarding State

# Workflow view bindings
keybinding-desc-workflow-view-save = Save workflow
keybinding-desc-workflow-view-close = Close

# Editor view binding desc (shared by editor/view/mod.rs, code/editor/view/actions.rs, notebooks/editor/view.rs)
keybinding-desc-editor-copy = Copy
keybinding-desc-editor-cut = Cut
keybinding-desc-editor-paste = Paste
keybinding-desc-editor-undo = Undo
keybinding-desc-editor-redo = Redo
keybinding-desc-editor-select-left-by-word = Select one word to the left
keybinding-desc-editor-select-right-by-word = Select one word to the right
keybinding-desc-editor-select-left = Select one character to the left
keybinding-desc-editor-select-right = Select one character to the right
keybinding-desc-editor-select-up = Select up
keybinding-desc-editor-select-down = Select down
keybinding-desc-editor-select-all = Select all
keybinding-desc-editor-select-to-line-start = Select to start of line
keybinding-desc-editor-select-to-line-end = Select to end of line
keybinding-desc-editor-select-to-line-start-cap = Select To Line Start
keybinding-desc-editor-select-to-line-end-cap = Select To Line End
keybinding-desc-editor-clear-and-copy-lines = Copy and clear selected lines
keybinding-desc-editor-add-next-occurrence = Add selection for next occurrence
keybinding-desc-editor-up = Move cursor up
keybinding-desc-editor-down = Move cursor down
keybinding-desc-editor-left = Move cursor left
keybinding-desc-editor-right = Move cursor right
keybinding-desc-editor-move-to-line-start = Move to start of line
keybinding-desc-editor-move-to-line-end = Move to end of line
keybinding-desc-editor-move-to-line-start-short = Move to line start
keybinding-desc-editor-move-to-line-end-short = Move to line end
keybinding-desc-editor-home = Home
keybinding-desc-editor-end = End
keybinding-desc-editor-cmd-down = Move cursor to the bottom
keybinding-desc-editor-cmd-up = Move cursor to the top
keybinding-desc-editor-move-to-and-select-buffer-start = Select and move to the top
keybinding-desc-editor-move-to-and-select-buffer-end = Select and move to the bottom
keybinding-desc-editor-move-forward-one-word = Move forward one word
keybinding-desc-editor-move-backward-one-word = Move backward one word
keybinding-desc-editor-move-forward-one-word-cap = Move Forward One Word
keybinding-desc-editor-move-backward-one-word-cap = Move Backward One Word
keybinding-desc-editor-move-to-paragraph-start = Move to the start of the paragraph
keybinding-desc-editor-move-to-paragraph-end = Move to the end of the paragraph
keybinding-desc-editor-move-to-paragraph-start-short = Move to start of paragraph
keybinding-desc-editor-move-to-paragraph-end-short = Move to end of paragraph
keybinding-desc-editor-move-to-buffer-start = Move to the start of the buffer
keybinding-desc-editor-move-to-buffer-end = Move to the end of the buffer
keybinding-desc-editor-cursor-at-buffer-start = Cursor at buffer start
keybinding-desc-editor-cursor-at-buffer-end = Cursor at buffer end
keybinding-desc-editor-backspace = Remove the previous character
keybinding-desc-editor-cut-word-left = Cut word left
keybinding-desc-editor-cut-word-right = Cut word right
keybinding-desc-editor-delete-word-left = Delete word left
keybinding-desc-editor-delete-word-right = Delete word right
keybinding-desc-editor-cut-all-left = Cut all left
keybinding-desc-editor-cut-all-right = Cut all right
keybinding-desc-editor-delete-all-left = Delete all left
keybinding-desc-editor-delete-all-right = Delete all right
keybinding-desc-editor-delete = Delete
keybinding-desc-editor-clear-lines = Clear selected lines
keybinding-desc-editor-insert-newline = Insert newline
keybinding-desc-editor-fold = Fold
keybinding-desc-editor-unfold = Unfold
keybinding-desc-editor-fold-selected-ranges = Fold selected ranges
keybinding-desc-editor-insert-last-word-prev-cmd = Insert last word of previous command
keybinding-desc-editor-move-backward-one-subword = Move Backward One Subword
keybinding-desc-editor-move-forward-one-subword = Move Forward One Subword
keybinding-desc-editor-select-left-by-subword = Select one subword to the left
keybinding-desc-editor-select-right-by-subword = Select one subword to the right
keybinding-desc-editor-accept-autosuggestion = Accept autosuggestion
keybinding-desc-editor-inspect-command = Inspect Command
keybinding-desc-editor-clear-buffer = Clear command editor
keybinding-desc-editor-add-cursor-above = Add cursor above
keybinding-desc-editor-add-cursor-below = Add cursor below
keybinding-desc-editor-insert-nonexpanding-space = Insert non-expanding space
keybinding-desc-editor-vim-exit-insert-mode = Exit Vim insert mode
keybinding-desc-editor-toggle-comment = Toggle comment
keybinding-desc-editor-go-to-line = Go to line
keybinding-desc-editor-find-in-code-editor = Find in code editor

# Code editor (Code) binding desc
keybinding-desc-code-save-as = Save file as
keybinding-desc-code-close-all-tabs = Close all tabs
keybinding-desc-code-close-saved-tabs = Close saved tabs

# Welcome view binding desc
keybinding-desc-welcome-terminal-session = Terminal session
keybinding-desc-welcome-add-repository = Add repository

# AI assistant panel binding desc
keybinding-desc-ai-assistant-close = Close Warp AI
keybinding-desc-ai-assistant-focus-terminal-input = Focus Terminal Input From Warp AI
keybinding-desc-ai-assistant-restart = Restart Warp AI

# Code review binding desc
keybinding-desc-code-review-save-all = Save all unsaved files in code review
keybinding-desc-code-review-show-find = Show find bar in code review

# Project buttons binding desc
keybinding-desc-project-buttons-open-repository = Open repository
keybinding-desc-project-buttons-create-new-project = Create new project

# Find view binding desc
keybinding-desc-find-next-occurrence = Find the next occurrence of your search query
keybinding-desc-find-prev-occurrence = Find the previous occurrence of your search query

# Notebook file / notebook binding desc
keybinding-desc-notebook-focus-terminal-input-from-file = Focus Terminal Input from File
keybinding-desc-notebook-reload-file = Reload file
keybinding-desc-notebook-increase-font-size = Increase notebook font size
keybinding-desc-notebook-decrease-font-size = Decrease notebook font size
keybinding-desc-notebook-reset-font-size = Reset notebook font size
keybinding-desc-notebook-focus-terminal-input = Focus Terminal Input from Notebook
keybinding-desc-notebook-fb-increase-font-size = Increase font size
keybinding-desc-notebook-fb-decrease-font-size = Decrease font size

# Notebook editor binding desc (extra to shared editor keys)
keybinding-desc-nbeditor-deselect-command = De-select shell commands
keybinding-desc-nbeditor-select-command = Select shell command at cursor
keybinding-desc-nbeditor-select-previous-command = Select previous command
keybinding-desc-nbeditor-select-next-command = Select next command
keybinding-desc-nbeditor-run-commands = Run selected commands
keybinding-desc-nbeditor-toggle-debug = Toggle rich-text debug mode
keybinding-desc-nbeditor-debug-copy-buffer = Copy rich-text buffer
keybinding-desc-nbeditor-debug-copy-selection = Copy rich-text selection
keybinding-desc-nbeditor-log-state = Log editor state
keybinding-desc-nbeditor-edit-link = Create or edit link
keybinding-desc-nbeditor-inline-code = Toggle inline code styling
keybinding-desc-nbeditor-strikethrough = Toggle strikethrough styling
keybinding-desc-nbeditor-underline = Toggle underline styling
keybinding-desc-nbeditor-find = Find in Notebook
keybinding-desc-nbeditor-next-find-match = Focus next match
keybinding-desc-nbeditor-previous-find-match = Focus previous match
keybinding-desc-nbeditor-toggle-regex-find = Toggle regular expression search
keybinding-desc-nbeditor-toggle-case-sensitive-find = Toggle case-sensitive search

# Pane group / undo close binding desc
keybinding-desc-get-started-terminal-session = Terminal session
keybinding-desc-undo-close-reopen-session = Reopen closed session
keybinding-desc-right-panel-toggle-maximize-code-review = Toggle Maximize Code Review Panel

# Workspace sync inputs binding desc
keybinding-desc-workspace-disable-sync-inputs = Stop Synchronizing Any Panes
keybinding-desc-workspace-toggle-sync-inputs-tab = Toggle Synchronizing All Panes in Current Tab
keybinding-desc-workspace-toggle-sync-inputs-all-tabs = Toggle Synchronizing All Panes in All Tabs

# Workspace a11y / debug binding desc
keybinding-desc-workspace-a11y-concise = [a11y] Set concise accessibility announcements
keybinding-desc-workspace-a11y-verbose = [a11y] Set verbose accessibility announcements
keybinding-desc-workspace-copy-access-token = Copy access token to clipboard

# Env var collection binding desc
keybinding-desc-env-var-collection-close = Close

# Auth / share modal binding desc
keybinding-desc-share-block-copy = Copy
keybinding-desc-auth-paste-token = Paste
keybinding-desc-conversation-details-copy = Copy

# Terminal extras binding desc
keybinding-desc-terminal-show-history = Show History
keybinding-desc-terminal-ask-ai-selection = Ask Warp AI about Selection
keybinding-desc-terminal-ask-ai-last-block = Ask Warp AI about last block
keybinding-desc-terminal-ask-ai = Ask Warp AI
keybinding-desc-terminal-load-agent-conversation = Load agent mode conversation (from debug link in clipboard)
keybinding-desc-terminal-toggle-session-recording = Toggle PTY Recording for Session

# Notebook editor extra
keybinding-desc-nbeditor-select-to-paragraph-start = Select to start of paragraph
keybinding-desc-nbeditor-select-to-paragraph-end = Select to end of paragraph

# Misc binding desc(收尾批次:常量/LazyLock/动态描述去硬编码)
keybinding-desc-save-file = Save file
keybinding-desc-new-agent-pane = New Agent Pane
keybinding-desc-edit-code-diff = Edit Code Diff
keybinding-desc-edit-requested-command = Edit requested command
keybinding-desc-set-input-mode-agent = Set Input Mode to Agent Mode
keybinding-desc-set-input-mode-terminal = Set Input Mode to Terminal Mode
keybinding-desc-toggle-hide-cli-responses = Toggle Hide CLI Responses
keybinding-desc-slash-command = Slash command: { $name }
keybinding-desc-take-control-of-running-command = Take control of running command

# --- Terminal zero-state block (welcome chips) ---
terminal-zero-state-title = New terminal session
terminal-zero-state-start-agent = start a new agent conversation
terminal-zero-state-cycle-history = cycle past commands and conversations
terminal-zero-state-open-code-review = open code review
terminal-zero-state-autodetect-prompts = autodetect agent prompts in terminal sessions
terminal-zero-state-dismiss = Don't show again

# --- Rules page (ai/facts/view/rule.rs) ---
rules-description = Rules enhance the agent by providing structured guidelines that help maintain consistency, enforce best practices, and adapt to specific workflows, including codebases or broader tasks.
rules-search-placeholder = Search rules
rules-name-placeholder = e.g. Rust rules
rules-description-placeholder = e.g. Never use unwrap in Rust
rules-zero-state-global = Once you add a rule, it will be shown here.
rules-zero-state-project = Once you generate a WARP.md rules file for a project, it will appear here.
rules-disabled-banner-prefix = Your rules are disabled and won't be used as context in sessions. You can {" "}
rules-disabled-banner-link = turn it back on
rules-disabled-banner-suffix = {" "}anytime.
rules-tab-global = Global
rules-tab-project = Project based
rules-add-button = Add
rules-init-project-button = Initialize Project

# --- Agent view zero-state + message bar ---
agent-zero-state-title = New Oz agent conversation
agent-zero-state-description = Send a prompt below to start a new conversation
agent-zero-state-description-with-location = Send a prompt below to start a new conversation in `{ $location }`
agent-zero-state-recent-activity = RECENT ACTIVITY
inline-agent-header-prompt-to-interact-command = Prompt agent to interact with `{ $command }`
inline-agent-header-prompt-to-interact-running-command = Prompt agent to interact with the running command
inline-agent-header-waiting-on-instructions = Agent is waiting for instructions
inline-agent-header-waiting-for-command = Agent is waiting for command to exit
inline-agent-header-agent-blocked = Agent needs your permission to continue
inline-agent-header-agent-in-control = Agent is in control
inline-agent-header-user-in-control = User is in control
agent-toolbar-edit-agent-toolbelt = Edit agent toolbelt
agent-toolbar-edit-cli-agent-toolbelt = Edit CLI agent toolbelt
agent-toolbar-available-chips = Available chips
agent-message-bar-get-figma-mcp = Get Figma MCP
agent-message-bar-enable-figma-mcp = Enable Figma MCP
agent-message-bar-enabling = Enabling...
orchestration-parent-conversation = Parent conversation
orchestration-back-to-parent-conversation = Back to parent conversation
child-agent-default-name = Agent
agent-zero-state-switch-model = switch model
agent-zero-state-go-back-to-terminal = go back to terminal
agent-message-bar-for-help = for help
agent-message-bar-for-commands = for commands
agent-message-bar-open-conversation = open conversation
agent-message-bar-for-code-review = for code review
agent-message-bar-resume-conversation = to resume conversation
agent-message-bar-hide-plan = to hide plan
agent-message-bar-view-plans = to view plans
agent-message-bar-view-plan = to view plan
agent-message-bar-fork-continue = to fork and continue
agent-message-bar-new-pane = {" "}new pane
agent-message-bar-new-tab = {" "}new tab
agent-message-bar-current-pane = {" "}current pane
agent-message-bar-hide-help = to hide help
agent-message-bar-autodetected-shell-command-prefix = autodetected shell command, {" "}
agent-message-bar-autodetected-shell-command = autodetected shell command
agent-message-bar-override = {" "}to override
agent-message-bar-exit-shell-mode = to exit shell mode
agent-message-bar-again-stop-exit = again to stop and exit
agent-message-bar-again-exit = again to exit
agent-message-bar-again-start-new-conversation = again to start new conversation
agent-shortcuts-input-shell-command = input shell command
agent-shortcuts-slash-commands = for slash commands
agent-shortcuts-file-paths-context = for file paths and attaching other context
agent-shortcuts-open-code-review = open code review
agent-shortcuts-toggle-conversation-list = toggle conversation list
agent-shortcuts-search-continue-conversations = search and continue conversations
agent-shortcuts-start-new-conversation = start a new conversation
agent-shortcuts-toggle-auto-accept = toggle auto-accept
agent-shortcuts-pause-agent = pause agent
agent-error-will-resume-when-network-restored = Will resume conversation when network connectivity is restored...
agent-error-attempting-resume-conversation = Attempting to resume conversation...

# --- ANCHOR-SUB-TOGGLE-PAIR (settings-toggle-pair) ---
toggle-setting-enable = Enable { $suffix }
toggle-setting-disable = Disable { $suffix }

toggle-suffix-ai = AI
toggle-suffix-active-ai = Active AI
toggle-suffix-ai-input-autodetect-agent = terminal command autodetection in agent input
toggle-suffix-ai-input-autodetect-nld = natural language detection
toggle-suffix-nld-in-terminal = agent prompt autodetection in terminal input
toggle-suffix-next-command = Next Command
toggle-suffix-prompt-suggestions = prompt suggestions
toggle-suffix-code-suggestions = code suggestions
toggle-suffix-nl-autosuggestions = natural language autosuggestions
toggle-suffix-voice-input = voice input
toggle-suffix-codebase-index = codebase index
toggle-suffix-auto-indexing = auto-indexing
toggle-suffix-compact-mode = compact mode
toggle-suffix-themes-sync-os = themes: sync with OS
toggle-suffix-cursor-blink = cursor blink
toggle-suffix-jump-bottom-block = jump to bottom of block button
toggle-suffix-block-dividers = block dividers
toggle-suffix-dim-inactive-panes = dim inactive panes
toggle-suffix-tab-indicators = tab indicators
toggle-suffix-focus-follows-mouse = focus follows mouse
toggle-suffix-zen-mode = zen mode
toggle-suffix-vertical-tabs = vertical tab layout
toggle-suffix-ligature-rendering = ligature rendering
toggle-suffix-copy-on-select = copy on select within the terminal
toggle-suffix-linux-selection-clipboard = linux selection clipboard
toggle-suffix-autocomplete-symbols = autocomplete quotes, parentheses, and brackets
toggle-suffix-restore-session = restore windows, tabs, and panes on startup
toggle-suffix-left-option-meta = Left Option key is Meta
toggle-suffix-left-alt-meta = Left Alt key is Meta
toggle-suffix-right-option-meta = Right Option key is Meta
toggle-suffix-right-alt-meta = Right Alt key is Meta
toggle-suffix-scroll-reporting = scroll reporting
toggle-suffix-completions-while-typing = completions while typing
toggle-suffix-command-corrections = command corrections
toggle-suffix-error-underlining = error underlining
toggle-suffix-syntax-highlighting = syntax highlighting
toggle-suffix-audible-bell = audible terminal bell
toggle-suffix-autosuggestions = autosuggestions
toggle-suffix-autosuggestion-keybinding-hint = autosuggestion keybinding hint
toggle-suffix-ssh-wrapper = Warp SSH wrapper
toggle-suffix-link-tooltip = show tooltip on click on links
toggle-suffix-quit-warning = quit warning modal
toggle-suffix-alias-expansion = alias expansion
toggle-suffix-middle-click-paste = middle-click paste
toggle-suffix-code-as-default-editor = code as default editor
toggle-suffix-input-hint-text = input hint text
toggle-suffix-vim-keybindings = editing commands with Vim keybindings
toggle-suffix-vim-clipboard = Vim unnamed register as system clipboard
toggle-suffix-vim-status-bar = Vim status bar
toggle-suffix-focus-reporting = focus reporting
toggle-suffix-smart-select = smart select
toggle-suffix-input-message-line = terminal input message line
toggle-suffix-slash-commands-terminal = slash commands in terminal mode
toggle-suffix-integrated-gpu = integrated GPU rendering (low power)
toggle-suffix-wayland = Wayland for window management
toggle-suffix-app-analytics = local diagnostics
toggle-suffix-crash-reporting = crash reporting
toggle-suffix-secret-redaction = secret redaction
toggle-suffix-recording-mode = recording mode
toggle-suffix-inband-generators = in-band generators for new sessions
toggle-suffix-debug-network = debug network status
toggle-suffix-memory-stats = memory statistics

# Set agent thinking display
agent-thinking-display-show-collapse = Set agent thinking display: show & collapse
agent-thinking-display-always-show = Set agent thinking display: always show
agent-thinking-display-never-show = Set agent thinking display: never show

# --- ANCHOR-SUB-EXTERNAL-EDITOR (settings-external-editor) ---
settings-external-editor-choose-default = Choose an editor to open file links
settings-external-editor-choose-code-panels = Choose an editor to open files from the code review panel, project explorer, and global search
settings-external-editor-choose-layout = Choose a layout to open files in Warp
settings-external-editor-tabbed-header = Group files into single editor pane
settings-external-editor-tabbed-desc = When this setting is on, any files opened in the same tab will be automatically grouped into a single editor pane.
settings-external-editor-prefer-markdown = Open Markdown files in Warp's Markdown Viewer by default
settings-external-editor-layout-split-pane = Split Pane
settings-external-editor-layout-new-tab = New Tab
settings-external-editor-default-app = Default App

# =============================================================================
# SECTION: context-menu (Owner: agent-context-menu)
# 鼠标右键弹出菜单。surface 前缀:menu-{block,input,ai-block,tab,pane,filetree,codeeditor}-*
# =============================================================================

# --- block 右键菜单(terminal/view.rs) ---
menu-block-copy = Copy
menu-block-copy-url = Copy URL
menu-block-copy-path = Copy path
menu-block-show-in-finder = Show in Finder
menu-block-show-containing-folder = Show containing folder
menu-block-open-in-warp = Open in Warp
menu-block-open-in-editor = Open in editor
menu-block-insert-into-input = Insert into input
menu-block-copy-command = Copy command
menu-block-copy-commands = Copy commands
menu-block-find-within-block = Find within block
menu-block-find-within-blocks = Find within blocks
menu-block-scroll-to-top-of-block = Scroll to top of block
menu-block-scroll-to-top-of-blocks = Scroll to top of blocks
menu-block-scroll-to-bottom-of-block = Scroll to bottom of block
menu-block-scroll-to-bottom-of-blocks = Scroll to bottom of blocks
menu-block-save-as-workflow = Save as workflow
menu-block-ask-warp-ai = Ask Warp AI
menu-block-copy-output = Copy output
menu-block-copy-filtered-output = Copy filtered output
menu-block-toggle-block-filter = Toggle block filter
menu-block-toggle-bookmark = Toggle bookmark
menu-block-copy-prompt = Copy prompt
menu-block-copy-right-prompt = Copy right prompt
menu-block-copy-working-directory = Copy working directory
menu-block-copy-git-branch = Copy git branch
menu-block-edit-prompt = Edit prompt
menu-block-edit-cli-agent-toolbelt = Edit CLI agent toolbelt
menu-block-edit-agent-toolbelt = Edit agent toolbelt
menu-block-split-pane-right = Split pane right
menu-block-split-pane-left = Split pane left
menu-block-split-pane-down = Split pane down
menu-block-split-pane-up = Split pane up
menu-block-close-pane = Close pane

# --- input 右键菜单(terminal/view.rs) ---
menu-input-cut = Cut
menu-input-copy = Copy
menu-input-paste = Paste
menu-input-select-all = Select all
menu-input-command-search = Command search
menu-input-ai-command-search = AI command search
menu-input-ask-warp-ai = Ask Warp AI
menu-input-save-as-workflow = Save as workflow
menu-input-hide-hint-text = Hide input hint text
menu-input-show-hint-text = Show input hint text

# --- AI block overflow 菜单(terminal/view.rs) ---
menu-ai-block-copy = Copy
menu-ai-block-copy-prompt = Copy prompt
menu-ai-block-copy-output-as-markdown = Copy output as Markdown
menu-ai-block-copy-url = Copy URL
menu-ai-block-copy-path = Copy path
menu-ai-block-copy-command = Copy command
menu-ai-block-copy-git-branch = Copy git branch
menu-ai-block-save-as-prompt = Save as prompt
menu-ai-block-copy-conversation-text = Copy conversation text
menu-ai-block-fork-from-here = Fork from here
menu-ai-block-rewind-to-before-here = Rewind to before here
menu-ai-block-fork-from-last-query = Fork from last query
menu-ai-block-fork-from-query = Fork from "{ $query }"

# --- tab 右键菜单(tab.rs) ---
menu-tab-stop-sharing = Stop sharing
menu-tab-stop-sharing-all = Stop sharing all
menu-tab-copy-link = Copy link
menu-tab-rename = Rename tab
menu-tab-reset-name = Reset tab name
menu-tab-move-down = Move Tab Down
menu-tab-move-right = Move Tab Right
menu-tab-move-up = Move Tab Up
menu-tab-move-left = Move Tab Left
menu-tab-close = Close tab
menu-tab-close-other = Close other tabs
menu-tab-close-below = Close Tabs Below
menu-tab-close-right = Close Tabs to the Right
menu-tab-save-as-new-config = Save as new config
menu-tab-default-no-color = Default (no color)

# --- pane header 溢出菜单(terminal/view/pane_impl.rs) ---
menu-pane-copy-link = Copy link
menu-pane-stop-sharing-session = Stop session broadcast
menu-pane-open-on-desktop = Open on Desktop

# --- 文件树右键菜单(code/file_tree/view.rs) ---
menu-filetree-open-in-new-pane = Open in new pane
menu-filetree-open-in-new-tab = Open in new tab
menu-filetree-open-file = Open file
menu-filetree-new-file = New file
menu-filetree-cd-to-directory = cd to directory
menu-filetree-reveal-finder = Reveal in Finder
menu-filetree-reveal-explorer = Reveal in Explorer
menu-filetree-reveal-file-manager = Reveal in file manager
menu-filetree-rename = Rename
menu-filetree-delete = Delete
menu-filetree-attach-as-context = Attach as context
menu-filetree-copy-path = Copy path
menu-filetree-copy-relative-path = Copy relative path

# --- 代码编辑器右键菜单(code/local_code_editor.rs) ---
menu-codeeditor-go-to-definition = Go to definition
menu-codeeditor-find-references = Find references

# --- 共享标签:附加为 agent 上下文(blocklist/view_util.rs) ---
menu-attach-as-agent-context = Attach as agent context

# --- ANCHOR-SUB-SLASH-COMMANDS (agent-slash-commands) ---
# Slash command palette descriptions and argument hints
# (app/src/search/slash_command_menu/static_commands/commands.rs)
slash-cmd-agent-desc = Start a new conversation
slash-cmd-add-mcp-desc = Add new MCP server
slash-cmd-pr-comments-desc = Pull GitHub PR review comments
slash-cmd-create-environment-desc = Create an Oz environment (Docker image + repos) via guided setup
slash-cmd-create-environment-hint = <optional repo paths or GitHub URLs>
slash-cmd-docker-sandbox-desc = Create a new docker sandbox terminal session
slash-cmd-create-new-project-desc = Have Oz walk you through creating a new coding project
slash-cmd-create-new-project-hint = <describe what you want to build>
slash-cmd-open-skill-desc = Open a skill's markdown file in Warp's built-in editor
slash-cmd-skills-desc = Invoke a skill
slash-cmd-add-prompt-desc = Add new Agent prompt
slash-cmd-add-rule-desc = Add a new global rule for the agent
slash-cmd-open-file-desc = Open a file in Warp's code editor
slash-cmd-open-file-hint = <path/to/file[:line[:col]]> or "@" to search
slash-cmd-rename-tab-desc = Rename the current tab
slash-cmd-rename-tab-hint = <tab name>
slash-cmd-fork-desc = Fork the current conversation in a new pane or a new tab
slash-cmd-fork-hint = <optional prompt to send in forked conversation>
slash-cmd-open-code-review-desc = Open code review
slash-cmd-init-desc = Generate or update an AGENTS.md file
slash-cmd-open-project-rules-desc = Open the project rules file (AGENTS.md)
slash-cmd-open-mcp-servers-desc = Open MCP servers
slash-cmd-open-settings-file-desc = Open settings file (TOML)
slash-cmd-changelog-desc = Open the latest changelog
slash-cmd-open-repo-desc = Switch to another indexed repository
slash-cmd-open-rules-desc = View all of your global and project rules
slash-cmd-new-desc = Start a new conversation (alias for /agent)
slash-cmd-model-desc = Switch the base agent model
slash-cmd-profile-desc = Switch the active execution profile
slash-cmd-plan-desc = Prompt the agent to do some research and create a plan for a task
slash-cmd-plan-hint = <describe your task>
slash-cmd-orchestrate-desc = Break a task into subtasks and run them in parallel with multiple agents
slash-cmd-orchestrate-hint = <describe your task>
slash-cmd-compact-desc = Free up context by summarizing convo history
slash-cmd-compact-hint = <optional custom summarization instructions>
slash-cmd-compact-and-desc = Compact conversation and then send a follow-up prompt
slash-cmd-compact-and-hint = <prompt to send after compaction>
slash-cmd-queue-desc = Queue a prompt to send after the agent finishes responding
slash-cmd-queue-hint = <prompt to send when agent is done>
slash-cmd-fork-and-compact-desc = Fork current conversation and compact it in the forked copy
slash-cmd-fork-and-compact-hint = <optional prompt to send after compaction>
slash-cmd-fork-from-desc = Fork conversation from a specific query
slash-cmd-remote-control-desc = Start remote control for this session
slash-cmd-conversations-desc = Open conversation history
slash-cmd-prompts-desc = Search saved prompts
slash-cmd-rewind-desc = Rewind to a previous point in the conversation
slash-cmd-export-to-clipboard-desc = Export current conversation to clipboard in markdown format
slash-cmd-export-to-file-desc = Export current conversation to a markdown file
slash-cmd-export-to-file-hint = <optional filename>

# --- ANCHOR-SUB-PROMPT-TIPS ---
# Prompt editor modal (app/src/prompt/editor_modal.rs)
prompt-editor-title = Edit prompt
prompt-editor-warp-prompt-section = Warp terminal prompt
prompt-editor-shell-prompt-section = Shell prompt (PS1)
prompt-editor-restore-default = Restore default
prompt-editor-same-line-prompt = Same line prompt
prompt-editor-separator = Separator
prompt-editor-cancel = Cancel
prompt-editor-save-changes = Save changes

# Welcome tips (app/src/tips/tip_view.rs)
welcome-tips-command-palette-title = Command Palette
welcome-tips-command-palette-description = Easily discover everything you can do in Warp without your hands leaving the keyboard.
welcome-tips-split-pane-title = Split Pane
welcome-tips-split-pane-description = Split tabs into multiple panes to make your ideal layout.
welcome-tips-history-search-title = History Search
welcome-tips-history-search-description = Find, edit and re-run previously executed commands.
welcome-tips-ai-command-search-title = AI Command Search
welcome-tips-ai-command-search-description = Generate shell commands with natural language.
welcome-tips-theme-picker-title = Theme Picker
welcome-tips-theme-picker-description = Make Warp your own by choosing a built-in theme. Or create your own.
welcome-tips-shortcut-label = Shortcut
welcome-tips-skip = Skip Welcome Tips
welcome-tips-complete-title = Complete!
welcome-tips-complete-description = Nice work on finishing the welcome tips!
welcome-tips-close = Close Welcome Tips

# --- ANCHOR-SUB-SMALL-DIALOGS ---
# Rewind confirmation dialog (app/src/workspace/rewind_confirmation_dialog.rs)
rewind-dialog-title = Rewind
rewind-dialog-body = Are you sure you want to rewind? This will restore your code and conversation to before this point, and cancel any commands the agent is currently running. A copy of the original conversation will be saved in your conversation history.
rewind-dialog-info = Rewinding does not affect files edited manually or via shell commands.
rewind-dialog-cancel = Cancel
rewind-dialog-confirm = Rewind

# --- ANCHOR-SUB-SEARCH-PALETTES ---
# Search palettes (app/src/search/command_palette/view.rs, app/src/search/welcome_palette/view.rs)
command-palette-search-placeholder = Search for a command
command-palette-no-results = No results found
command-palette-toast-cannot-switch-conversations = Cannot switch conversations while agent is monitoring a command.
command-palette-toast-cannot-start-new-conversation = Cannot start a new conversation while agent is monitoring a command.
command-palette-zero-state-recent = Recent
command-palette-zero-state-suggested = Suggested
welcome-palette-search-placeholder = Code, build, or search for anything...
welcome-palette-no-results = No results found
search-filter-placeholder-history = Search history
search-filter-placeholder-workflows = Search workflows
search-filter-placeholder-agent-mode-workflows = Search prompts
search-filter-placeholder-notebooks = Search notebooks
search-filter-placeholder-plans = Search plans
search-filter-placeholder-natural-language = e.g. replace string in file
search-filter-placeholder-actions = Search actions
search-filter-placeholder-sessions = Search sessions
search-filter-placeholder-conversations = Search conversations
search-filter-placeholder-historical-conversations = Search historical conversations
search-filter-placeholder-launch-configurations = Search launch configurations
search-filter-placeholder-drive = Search objects in drive
search-filter-placeholder-environment-variables = Search environment variables
search-filter-placeholder-prompt-history = Search prompt history
search-filter-placeholder-files = Search files
search-filter-placeholder-commands = Search commands
search-filter-placeholder-blocks = Search blocks
search-filter-placeholder-code = Search code symbols
search-filter-placeholder-rules = Search AI rules
search-filter-placeholder-repos = Search code repos
search-filter-placeholder-diff-sets = Search diff sets
search-filter-placeholder-static-slash-commands = Search static slash commands
search-filter-placeholder-skills = Search skills
search-filter-placeholder-base-models = Search base models
search-filter-placeholder-full-terminal-use-models = Search full terminal use models
search-filter-placeholder-current-directory-conversations = Search conversations in current directory
search-filter-display-history = history
search-filter-display-workflows = workflows
search-filter-display-agent-mode-workflows = prompts
search-filter-display-notebooks = notebooks
search-filter-display-plans = plans
search-filter-display-natural-language = AI command suggestions
search-filter-display-actions = actions
search-filter-display-sessions = sessions
search-filter-display-conversations = conversations
search-filter-display-launch-configurations = launch configurations
search-filter-display-drive = Warp Drive
search-filter-display-environment-variables = environment variables
search-filter-display-prompt-history = prompt history
search-filter-display-files = files
search-filter-display-commands = commands
search-filter-display-blocks = blocks
search-filter-display-code = code
search-filter-display-rules = rules
search-filter-display-repos = repos
search-filter-display-diff-sets = diff sets
search-filter-display-static-slash-commands = slash commands
search-filter-display-historical-conversations = historical conversations
search-filter-display-skills = skills
search-filter-display-base-models = base models
search-filter-display-full-terminal-use-models = full terminal use models
search-filter-display-current-directory-conversations = current directory conversations
search-results-menu-no-results = No results found
search-results-menu-prompts-title = Prompts
ai-context-diffset-uncommitted-changes = Uncommitted changes
ai-context-diffset-changes-vs-main-branch = Changes vs. main branch
ai-context-diffset-changes-vs-branch = Changes vs. { $branch }
ai-context-diffset-uncommitted-changes-description = All uncommitted changes in the working directory
ai-context-diffset-changes-vs-main-branch-description = All changes compared to the main branch
ai-context-diffset-changes-vs-branch-description = All changes compared to { $branch }
ai-context-code-search-failed = Code search failed
ai-context-files-directory-accessibility-label = Directory: { $path }
ai-context-files-file-accessibility-label = File: { $path }
ai-context-blocks-just-now = Just now
ai-context-blocks-minutes-ago = { $count ->
        [one] 1 minute ago
       *[other] { $count } minutes ago
    }
ai-context-blocks-hours-ago = { $count ->
        [one] 1 hour ago
       *[other] { $count } hours ago
    }
ai-context-blocks-days-ago = { $count ->
        [one] 1 day ago
       *[other] { $count } days ago
    }
ai-context-blocks-no-output = No output
ai-context-blocks-accessibility-label = Block: { $command }

# --- ANCHOR-SUB-DRIVE-NAMING-IMPORT ---
# Drive naming dialog (app/src/drive/cloud_object_naming_dialog.rs)
drive-naming-notebook-name = Notebook name
drive-naming-folder-name = Folder name
drive-naming-collection-name = Collection name
drive-naming-create = Create
drive-naming-cancel = Cancel
drive-naming-rename = Rename

# Drive import modal (app/src/drive/import/modal.rs, app/src/drive/import/modal_body.rs)
drive-import-title = Import
drive-import-close = Close
drive-import-cancel = Cancel
drive-import-preparing = Preparing...
drive-import-choose-files = Choose files...
drive-import-learn-file-support = Learn about file support and formatting
drive-import-file-upload-error = Failed to upload file to server
drive-import-folder-upload-error = Failed to upload folder to server

# Drive main panel and workflow editor (app/src/drive/index.rs, app/src/drive/workflows/*)
drive-title = Drive
drive-environment-variables = Environment variables
drive-folder = Folder
drive-notebook = Notebook
drive-workflow = Workflow
drive-prompt = Prompt
drive-import = Import
drive-remove = Remove
drive-new-folder = New folder
drive-new-notebook = New notebook
drive-new-workflow = New workflow
drive-new-prompt = New prompt
drive-new-environment-variables = New environment variables
drive-offline-banner = You are offline. Some files will be read only.
drive-sort-by = Sort by
drive-retry-sync = Retry sync
drive-empty-trash = Empty trash
drive-trash-section-title = TRASH
drive-trash-title = Trash
drive-trash-deletion-warning = Items in the trash will be deleted forever after 30 days.
drive-team-space-zero-state = Team spaces are unavailable in local builds. Manage workflows and notebooks in Personal.
drive-sign-up-storage-limit = Local storage limits are enforced on this device.
drive-local-storage-limit-description = Local storage limits are enforced on this device. Remove unused items to create space for new Warp Drive objects.
drive-sign-up = Manage locally
drive-copy-link = Copy link
drive-collapse-all = Collapse all
drive-revert-to-server = Revert to server
drive-attach-to-active-session = Attach to active session
drive-copy-prompt = Copy prompt
drive-copy-workflow-text = Copy workflow text
drive-copy-id = Copy id
drive-copy-variables = Copy variables
drive-load-in-subshell = Load in subshell
drive-delete-forever = Delete forever
drive-rename = Rename
drive-retry = Retry
drive-move-to-space = Move to { $space }
drive-open-on-desktop = Open on Desktop
drive-duplicate = Duplicate
drive-export = Export
drive-trash-menu = Trash
drive-open = Open
drive-edit = Edit
drive-restore = Restore
drive-compare-plans = Compare plans
drive-manage-billing = Manage billing
drive-object-type-notebook-plural = notebook
drive-object-type-workflow-plural = workflow
drive-object-type-folder-plural = folder
drive-object-type-env-var-collection-plural = environment variable collection
drive-object-type-object-plural = object
drive-object-type-notebooks = Notebooks
drive-object-type-workflows = Workflows
drive-object-type-environment-variables = Environment Variables
drive-object-type-folders = Folders
drive-object-type-agent-workflows = Agent Workflows
drive-object-type-ai-fact = AI Fact
drive-object-type-rules = Rules
drive-object-type-mcp-server = MCP Server
drive-object-type-mcp-servers = MCP Servers
drive-shared-object-limit-hit-banner-prefix = You've reached the local { $object_type } limit.
drive-shared-object-limit-hit-banner = You've reached the local { $object_type } limit.
drive-payment-issue-banner-prefix = Shared objects have been restricted due to a subscription payment issue.
drive-payment-issue-banner-admin = Shared objects have been restricted due to a subscription payment issue. Please update your payment information to restore access.
drive-payment-issue-banner-admin-enterprise = Shared objects have been restricted due to a subscription payment issue. Please contact support@warp.dev to restore access.
drive-payment-issue-banner-nonadmin = Shared objects have been restricted due to a subscription payment issue. Please contact a team admin to restore access.
drive-empty-trash-title = Are you sure you want to empty the trash?
drive-empty-trash-body = This action cannot be undone.
drive-empty-trash-confirm = Yes, empty trash
drive-empty-trash-cancel = Cancel
workflow-title-placeholder = Untitled workflow
workflow-description-placeholder = Add a description
workflow-title-input-placeholder = Add a title
workflow-description-input-placeholder = Add a description
workflow-new-argument = New argument
workflow-arguments-label = Arguments
workflow-argument-description-placeholder = Description
workflow-argument-value-placeholder = Value (optional)
workflow-default-value-placeholder = Default value (optional)
workflow-agent-mode-query-placeholder = Enter your prompt here... (e.g., 'Create a function to sort an array of objects by date' or 'Help me debug this React component').
workflow-save = Save workflow
workflow-unsaved-changes = You have unsaved changes.
workflow-keep-editing = Keep editing
workflow-discard-changes = Discard changes
workflow-ai-assist-autofill = Autofill
workflow-ai-assist-loading = Loading
workflow-ai-assist-tooltip = Generate a title, descriptions, or parameters with Warp AI
workflow-tooltip-restore-from-trash = Restore workflow from trash
workflow-ai-assist-error-byop-required = Autofill requires a BYOP model. Configure a provider and model in Settings → AI.
workflow-ai-assist-error-bad-command = Failed to generate metadata. Please try again with a different command.
workflow-ai-assist-error-generic = Something went wrong. Please try again.
workflow-ai-assist-error-rate-limited = Looks like you're out of AI credits. Please try again later.
workflow-enum-new = New
workflow-alias-name-placeholder = alias name
workflow-add-argument-tooltip = Add a workflow argument

# --- ANCHOR-SUB-SETTINGS-PRIVACY-ADD-REGEX ---
# Privacy settings add regex modal (app/src/settings_view/privacy/add_regex_modal.rs)
settings-privacy-add-regex-name-placeholder = e.g. "Google API Key"
settings-privacy-add-regex-name-label = Name (optional)
settings-privacy-add-regex-pattern-label = Regex pattern
settings-privacy-add-regex-invalid = Invalid regex
settings-privacy-add-regex-cancel = Cancel

# Workspace panels (app/src/workspace/view/*)
workspace-conversation-list-search = Search
workspace-conversation-list-active = ACTIVE
workspace-conversation-list-past = PAST
workspace-conversation-list-view-all = View all
workspace-conversation-list-show-less = Show less
workspace-conversation-list-empty-title = No conversations yet
workspace-conversation-list-empty-description = Your active and past conversations with local and ambient agents will appear here.
workspace-conversation-list-new-conversation = New conversation
conversation-untitled = Untitled conversation
conversation-deleted = Deleted conversation
workspace-conversation-list-no-matching = No matching conversations
workspace-conversation-list-delete = Delete
workspace-conversation-list-delete-in-progress-error = Conversations cannot be deleted while in progress.
workspace-conversation-list-delete-ambient-tooltip = Ambient agent conversations cannot be deleted
workspace-conversation-list-fork-new-pane = Fork in new pane
workspace-conversation-list-fork-new-tab = Fork in new tab
workspace-conversation-list-fallback-title = Conversation
command-palette-conversations-active-pane = Active pane conversations
command-palette-conversations-other-active = Other active conversations
command-palette-conversations-past = Past conversations
command-palette-conversations-fork-current = Fork current conversation
command-palette-conversations-fork-current-with-title = Fork current conversation ({ $title })
command-palette-conversations-a11y-navigate = Press enter to navigate to conversation
command-palette-conversations-a11y-fork = Press enter to fork the current conversation into a new conversation.
command-palette-conversations-a11y-new = Press enter to create a new conversation.
workspace-left-panel-project-explorer = Project explorer
project-explorer-unavailable-title = Project explorer unavailable
project-explorer-unavailable-disabled-description = The Project Explorer requires access to your local workspace. Open a new session or navigate to an active session to view.
project-explorer-unavailable-remote-description = The Project Explorer requires access to your local workspace, which isn’t supported in remote sessions.
project-explorer-unavailable-wsl-description = The Project Explorer doesn't currently work in WSL.
workspace-left-panel-global-search = Global search
workspace-left-panel-warp-drive = Warp Drive
workspace-left-panel-agent-conversations = Agent conversations
workspace-left-panel-ssh-manager = SSH Manager
workspace-left-panel-skill-manager = Skill Manager
skill-manager-search-placeholder = Search skills
skill-manager-filter-all = All
skill-manager-filter-provider = Source
skill-manager-meta-default = Default
skill-manager-meta-duplicate = Duplicate
skill-manager-empty = No skills match the current filters.
skill-manager-preview-empty = Select a skill to preview SKILL.md.
workspace-left-panel-ssh-manager-placeholder = SSH Manager — coming soon
workspace-left-panel-ssh-manager-detail-empty = Select a server to see its details.
workspace-left-panel-ssh-manager-detail-host = Host
workspace-left-panel-ssh-manager-detail-port = Port
workspace-left-panel-ssh-manager-detail-user = User
workspace-left-panel-ssh-manager-detail-auth = Auth
workspace-left-panel-ssh-manager-detail-key-path = Key path
workspace-left-panel-ssh-manager-auth-password = Password
workspace-left-panel-ssh-manager-auth-key = Private key
workspace-left-panel-ssh-manager-menu-new-folder = New folder
workspace-left-panel-ssh-manager-menu-new-server = New SSH server
workspace-left-panel-ssh-manager-menu-edit = Edit
workspace-left-panel-ssh-manager-menu-connect = Connect
workspace-left-panel-ssh-manager-menu-delete = Delete
workspace-left-panel-ssh-manager-pane-hint = Editing fields and "Connect" will arrive in the next iteration. For now this pane shows the saved configuration; tweak it via the SQLite store or the upcoming editor.
workspace-left-panel-ssh-manager-pane-folder-body = Folder. Select a server inside this folder to view its details, or right-click the folder for create / delete actions.
workspace-left-panel-ssh-manager-server-missing = Server not found. It may have been deleted from another window.
workspace-left-panel-ssh-manager-field-name = Name
workspace-left-panel-ssh-manager-passphrase = Passphrase
workspace-left-panel-ssh-manager-save = Save
workspace-left-panel-ssh-manager-status-saved = Saved.
workspace-left-panel-ssh-manager-error-name-required = Name cannot be empty.
workspace-left-panel-ssh-manager-error-port-invalid = Port must be a number between 1 and 65535.
workspace-left-panel-ssh-manager-error-host-required = Host cannot be empty.
workspace-left-panel-ssh-manager-connect = Connect
search-filter-placeholder-ssh-servers = Search SSH servers...
search-filter-display-ssh-servers = SSH Servers
workspace-left-panel-ssh-manager-menu-rename = Rename
workspace-left-panel-ssh-manager-tree-empty = No SSH servers yet. Click 📁 to add a folder, + to add a server.
workspace-left-panel-close-panel = Close panel
workspace-tabs-panel-tooltip = Tabs panel
workspace-tools-panel-tooltip = Tools panel
workspace-agent-management-panel-tooltip = Agent management panel
workspace-code-review-panel-tooltip = Code review panel
workspace-notifications-tooltip = Notifications
workspace-new-tab-tooltip = New Tab
workspace-tab-configs-tooltip = Tab configs
workspace-offline-tooltip = Some features may be unavailable offline
workspace-right-panel-open-repository = Open repository
workspace-right-panel-open-repository-tooltip = Navigate to a repo and initialize it for coding
workspace-right-panel-close-panel = Close panel
workspace-right-panel-code-review = Code review
workspace-right-panel-minimize = Minimize
workspace-right-panel-maximize = Maximize
terminal-pane-new-agent-conversation-title = New agent conversation
vertical-tabs-no-tabs-open = No tabs open
vertical-tabs-untitled-tab = Untitled tab
vertical-tabs-view-options-tooltip = View options
vertical-tabs-new-session = New session
vertical-tabs-terminal-kind-oz = Oz
vertical-tabs-pane-kind-terminal = Terminal
vertical-tabs-pane-kind-code = Code
vertical-tabs-pane-kind-code-diff = Code Diff
vertical-tabs-pane-kind-file = File
vertical-tabs-pane-kind-notebook = Notebook
vertical-tabs-pane-kind-workflow = Workflow
vertical-tabs-pane-kind-environment-variables = Environment Variables
vertical-tabs-pane-kind-environments = Environments
vertical-tabs-pane-kind-rules = Rules
vertical-tabs-pane-kind-plan = Plan
vertical-tabs-pane-kind-execution-profile = Execution Profile
vertical-tabs-pane-kind-other = Other
vertical-tabs-setting-view-as = View as
vertical-tabs-setting-panes = Panes
vertical-tabs-setting-tabs = Tabs
vertical-tabs-setting-tab-item = Tab item
vertical-tabs-setting-focused-session = Focused session
vertical-tabs-setting-summary = Summary
vertical-tabs-setting-density = Density
vertical-tabs-setting-pane-title-as = Pane title as
vertical-tabs-setting-command-conversation = Command / Conversation
vertical-tabs-setting-working-directory = Working Directory
vertical-tabs-setting-branch = Branch
vertical-tabs-setting-additional-metadata = Additional metadata
vertical-tabs-setting-show = Show
vertical-tabs-setting-pr-link-requires-gh = Requires the GitHub CLI to be installed and authenticated
vertical-tabs-setting-pr-link = PR link
vertical-tabs-setting-diff-stats = Diff stats
vertical-tabs-setting-show-details-on-hover = Show details on hover
workspace-right-panel-unknown = Unknown
global-search-placeholder = Search in files
global-search-toggle-case-sensitivity = Toggle Case Sensitivity
global-search-toggle-regex = Toggle Regex
global-search-label = Search
global-search-no-results-gitignore = No results found. Review your gitignore files.
global-search-result-count-one = 1 result in { $files } { $files ->
        [one] file
       *[other] files
    }
global-search-result-count-many = { $n } results in { $files } { $files ->
        [one] file
       *[other] files
    }
global-search-subset-warning = The result set only contains a subset of all matches. Be more specific in your search to narrow down results.
global-search-title = Global search
global-search-description = Search in files across your current directories.
global-search-unavailable-title = Global search unavailable
global-search-unavailable-description = Global search requires access to your local workspace. Open a new session or navigate to an active session to view.
global-search-remote-description = Global search requires access to your local workspace, which isn't supported in remote sessions
global-search-unsupported-session-description = Global search doesn't currently work in Git Bash or WSL.
global-search-failed = Global search failed.

# Wasm NUX dialog (app/src/wasm_nux_dialog.rs)
wasm-nux-open-desktop-title = Open in Warp Desktop?
wasm-nux-open-desktop-detail = Future links will automatically open on desktop.
wasm-nux-open-desktop-confirm = Open in Warp
wasm-nux-download-title = Download Warp Desktop?
wasm-nux-download-description = Warp is the intelligent terminal with AI and your dev team's knowledge built-in.
wasm-nux-learn-more = Learn more
wasm-nux-download-confirm = Download
wasm-nux-object-kind-drive-objects = Warp Drive objects
wasm-nux-object-kind-warp-links = Warp links
wasm-nux-always-open-on-web-title = Always open { $object_kind } on the web?
wasm-nux-always-open-on-web-detail = You can change this at any time in settings.
wasm-nux-yes = Yes

# Auth override warning (app/src/auth/auth_override_warning_body.rs)
auth-override-warning-title = New login detected
auth-override-warning-confirm-title = Delete personal Warp Drive objects and preferences?
auth-override-warning-description = It looks like you logged into a Warp account through a web browser. If you continue, any personal Warp Drive objects and preferences from this anonymous session will be permanently deleted.
auth-override-warning-cannot-undo = This cannot be undone.
auth-override-warning-export = Export your data
auth-override-warning-export-description =  to import later.
auth-override-warning-cancel = Cancel
auth-override-warning-continue = Continue
auth-override-warning-accessibility-help = Warp has detected a new login from a web browser. Press escape to cancel and continue using Warp without login.

# Auth SSO link/login failures/paste token/logout/offline/privacy
auth-needs-sso-link-button = Link SSO
auth-needs-sso-link-title = Your organization has enabled SSO for your account
auth-needs-sso-link-detail = Click the button below to link your Warp account to your SSO provider.
auth-login-failure-troubleshooting-prefix =  Not the first time? See our
auth-login-failure-troubleshooting-link =  troubleshooting docs
auth-login-failure-troubleshooting-suffix = .
auth-login-failure-invalid-token = An invalid auth token was entered into the modal.
auth-login-failure-copy-token-manually = Failed to log in. Try manually copying the auth token from the authentication web page and pasting into the modal.
auth-login-failure-login-request = Request to log in failed.
auth-login-failure-signup-request = Request to sign up failed.
auth-login-failure-wrong-redirect-url = The redirect URL pasted did not originate from this app. Please click the button below to try again.
auth-paste-token-placeholder = Enter auth token
auth-paste-token-title = Paste your auth token below
auth-paste-token-detail = Paste your auth token from the browser to get complete login.
auth-paste-token-cancel = Cancel
auth-paste-token-continue = Continue
auth-offline-first-use-description = You are currently offline. An internet connection is required to use Warp for the first time.
auth-offline-first-use-learn-more = Learn more
auth-offline-overlay-title = Using Warp Offline
auth-offline-overlay-paragraph-1 = Warp can be used offline for local terminal and agent workflows.
auth-offline-overlay-paragraph-2 = Some setup flows may still need an internet connection when they depend on external providers.
auth-offline-overlay-paragraph-3 = Logged-out usage keeps local workflows on this machine.
auth-offline-overlay-dismiss = Dismiss
auth-privacy-settings-title = Privacy Settings
auth-privacy-settings-done = Done
auth-privacy-settings-help-improve = Help improve Warp
auth-privacy-settings-help-improve-description = High-level feature usage data helps Warp's product team prioritize the roadmap.
auth-privacy-settings-learn-more = Learn more
auth-privacy-settings-send-crash-reports = Send crash reports
auth-privacy-settings-crash-reports-description = Crash reporting helps Warp's engineering team understand stability and improve performance.
auth-logout-confirm = Yes, log out
auth-logout-show-running-processes = Show running processes
auth-logout-cancel = Cancel
auth-logout-title = Log out?
auth-logout-running-processes-warning = You have { $count } { $count ->
        [one] process
       *[other] processes
    } running.
auth-logout-shared-sessions-warning = You have { $count } remote { $count ->
        [one] session
       *[other] sessions
    }.
auth-logout-unsynced-drive-objects-warning = You have { $count } unsynced Warp Drive { $count ->
        [one] object
       *[other] objects
    }. Logging out will cause you to lose the { $count ->
        [one] object
       *[other] objects
    }.
auth-logout-unsaved-files-warning = You have { $count } unsaved { $count ->
        [one] file
       *[other] files
    }. Logging out will cause you to lose the { $count ->
        [one] file
       *[other] files
    }.

# CLI agent plugin instructions
cli-agent-plugin-run-on-remote = Be sure to run these commands on your remote machine.
cli-agent-plugin-codex-install-title = Enable Warp Notifications for Codex
cli-agent-plugin-codex-install-subtitle = Update Codex to the latest version, then enable in-focus notifications so Warp can display them while you work.
cli-agent-plugin-codex-update-step = Update Codex to the latest version.
cli-agent-plugin-codex-notification-step = Set the notification condition to "always" in your Codex config. Open or create ~/.codex/config.toml and add:
cli-agent-plugin-codex-restart-note = Restart Codex to apply the changes.
cli-agent-plugin-deepseek-install-title = Enable Warp Notifications for DeepSeek
cli-agent-plugin-deepseek-install-subtitle = Add the following to your DeepSeek config file (~/.deepseek/config.toml) to enable turn-completion notifications.
cli-agent-plugin-deepseek-notification-step = Set the notification condition to "always" in ~/.deepseek/config.toml:
cli-agent-plugin-deepseek-restart-note = Restart DeepSeek to apply the changes.
cli-agent-plugin-claude-install-title = Install Warp Plugin for Claude Code
cli-agent-plugin-claude-install-subtitle = Ensure that jq is installed on your machine. Then, run these commands.
cli-agent-plugin-claude-add-marketplace-step = Add the Warp plugin marketplace repository
cli-agent-plugin-install-warp-plugin-step = Install the Warp plugin
cli-agent-plugin-claude-restart-note = Restart Claude Code to activate the plugin.
cli-agent-plugin-claude-known-issues-note = There are some known issues with Claude Code's plugin system. If the plugin is not found after step 1, you can try manually adding an "extraKnownMarketplaces" entry to ~/.claude/settings.json.
cli-agent-plugin-claude-update-title = Update Warp Plugin for Claude Code
cli-agent-plugin-run-following-commands = Run the following commands.
cli-agent-plugin-remove-existing-marketplace-step = Remove the existing marketplace (if present)
cli-agent-plugin-readd-marketplace-step = Re-add the marketplace
cli-agent-plugin-install-latest-version-step = Install the latest plugin version
cli-agent-plugin-claude-restart-update-note = Restart Claude Code to activate the update.
cli-agent-plugin-gemini-install-title = Install Warp Plugin for Gemini CLI
cli-agent-plugin-gemini-run-command-restart = Run the following command, then restart Gemini CLI.
cli-agent-plugin-install-warp-extension-step = Install the Warp extension
cli-agent-plugin-gemini-restart-note = Restart Gemini CLI to activate the plugin.
cli-agent-plugin-gemini-update-title = Update Warp Plugin for Gemini CLI
cli-agent-plugin-update-warp-extension-step = Update the Warp extension
cli-agent-plugin-gemini-restart-update-note = Restart Gemini CLI to activate the update.
cli-agent-plugin-opencode-install-title = Install Warp Plugin for OpenCode
cli-agent-plugin-opencode-install-subtitle = Add the Warp plugin to your OpenCode configuration, then restart OpenCode.
cli-agent-plugin-opencode-open-config-step = Open or create your opencode.json. This can be in your project root, or the global config path:
cli-agent-plugin-opencode-add-plugin-step = Add "@warp-dot-dev/opencode-warp" to the "plugin" array in the top-level JSON object:
cli-agent-plugin-opencode-restart-note = Restart OpenCode to activate the plugin.
cli-agent-plugin-opencode-update-title = Update Warp Plugin for OpenCode
cli-agent-plugin-opencode-update-subtitle = Pin the plugin to the latest version in your opencode.json. OpenCode caches plugins per version spec, so changing the pin forces it to re-fetch on restart.
cli-agent-plugin-opencode-replace-plugin-step = Replace the existing "@warp-dot-dev/opencode-warp" entry in the "plugin" array with the explicit version:
cli-agent-plugin-opencode-restart-update-note = Restart OpenCode to load the updated plugin.

# Remaining visible UI strings
ai-ask-user-questions-unavailable = Questions unavailable
ai-ask-user-questions-skipped-auto-approve = Questions skipped due to auto-approve
terminal-bootstrapping-checking = Checking...
terminal-bootstrapping-installing-progress = Installing... ({ $p }%)
terminal-bootstrapping-installing = Installing...
terminal-bootstrapping-updating = Updating...
terminal-bootstrapping-initializing = Initializing...
terminal-bootstrapping-installing-warp-ssh-extension-progress = Installing Warp SSH Extension... ({ $p }%)
terminal-bootstrapping-installing-warp-ssh-extension = Installing Warp SSH Extension...
terminal-bootstrapping-updating-warp-ssh-extension = Updating Warp SSH Extension...
terminal-bootstrapping-starting-shell-name = Starting { $shell }...
agent-tip-prefix = Tip:
agent-tip-slash-menu = `/` to open the slash-command menu and access quick agent actions.
agent-tip-toggle-input-mode = <keybinding> to toggle natural language detection and switch between agent and terminal input.
agent-tip-plan = `/plan` <prompt> to create a plan for the agent before executing.
agent-tip-command-palette = <keybinding> to open the Command Palette and access Warp actions and shortcuts.
agent-tip-warp-drive = Store reusable workflows, notebooks, and prompts in your
agent-tip-redirect-running-agent = Enter a new prompt to redirect the agent while it's running.
agent-tip-add-context = `@` to add context from files, blocks, or Warp Drive objects to your prompt.
agent-tip-attach-prior-output = <keybinding> to attach the prior command output as agent context.
agent-tip-init-index = `/init` to index the repo so the agent can understand your codebase.
agent-tip-agent-profiles = Add agent profiles to customize permissions and models per session.
agent-tip-fork-block = Right-click a block to fork the conversation from that point.
agent-tip-copy-output = Right-click a block to copy a conversation's output.
agent-tip-drag-image = Drag an image into the pane to attach it as agent context.
agent-tip-interactive-tools = Prompt the agent to control interactive tools like node, python, postgres, gdb, or vim.
agent-tip-code-review-panel = <keybinding> to open the code review panel and review the agent's changes.
agent-tip-add-mcp = `/add-mcp` to add an MCP server to your workspace.
agent-tip-open-mcp-servers = `/open-mcp-servers` to view and manage local MCP servers.
agent-tip-add-prompt = `/add-prompt` to create a reusable prompt for repeatable workflows.
agent-tip-add-rule = `/add-rule` to create a global agent rule.
agent-tip-fork = `/fork` to create a fresh copy of the current conversation, optionally with a new prompt.
agent-tip-open-code-review = `/open-code-review` to open the code review panel and inspect agent-generated diffs.
agent-tip-new-conversation = `/new` to start a new agent conversation with clean context.
agent-tip-compact = `/compact` to summarize the current conversation and free up space in the context window.
agent-tip-usage = `/usage` to show your current AI credits usage.
agent-tip-oz-headless = Use the `oz` command to run an Oz agent in headless mode, useful for remote machines.
agent-tip-selected-text-context = Right-click selected text to attach it as agent context.
agent-tip-project-rules = Use `AGENTS.md` or `CLAUDE.md` to apply project-scoped rules.
agent-tip-url-context = Paste a URL to attach that webpage as context for the agent.
agent-tip-warpify-ssh = Warpify a remote SSH session to enable Oz inside that environment.
agent-tip-switch-profiles = Switch agent profiles to quickly change models and agent permissions.
agent-tip-init-rules = `/init` to generate a `WARP.md` file and define project rules for the agent.
agent-tip-auto-approve = <keybinding> to auto-approve the agent's commands and diffs for the rest of the session.
agent-tip-desktop-notifications = Enable desktop notifications to get an alert when an agent needs your attention.
agent-tip-cancel-task = <keybinding> to cancel the current agent task.
agent-tip-action-open-palette = Open palette
agent-tip-action-warp-drive = Warp Drive.
agent-tip-action-show-diff-view = Show diff view
agent-tip-voice-input = Hold <keybinding> to speak your prompt directly to the agent.
hoa-welcome-banner-title = Introducing universal agent support: level up any coding agent with Warp
hoa-feature-vertical-tabs-title = Vertical tabs
hoa-feature-vertical-tabs-description = Rich tab titles and metadata like git branch, worktree, and PR. Fully customizable.
hoa-feature-tab-configs-title = Tab configs
hoa-feature-tab-configs-description = Tab-level schema to set your directory, startup commands, theme, and worktree with one click
hoa-feature-agent-inbox-title = Agent inbox
hoa-feature-agent-inbox-description = Notifications when any agent needs your attention, also accessible in a central inbox
hoa-feature-native-code-review-title = Native code review
hoa-feature-native-code-review-description = Send inline comments from Warp's code review directly to Claude Code, Codex, or OpenCode
resource-center-whats-new-section = What's New?
resource-center-getting-started-section = Getting Started
resource-center-maximize-warp-section = Maximize Warp
resource-center-advanced-setup-section = Advanced Setup
resource-center-create-first-block-title = Create your first block
resource-center-create-first-block-description = Run a command to see your command and output grouped.
resource-center-navigate-blocks-title = Navigate blocks
resource-center-navigate-blocks-description = Click to select a block and navigate with arrow keys.
resource-center-block-action-title = Take an action on block
resource-center-block-action-description = Right click on a block to copy/paste, share, more.
resource-center-command-palette-title = Open command palette
resource-center-command-palette-description = Access all of Warp via the keyboard.
resource-center-set-theme-title = Set your theme
resource-center-set-theme-description = Make Warp your own by choosing a theme.
resource-center-custom-prompt-title = Use your custom prompt
resource-center-custom-prompt-description = Set up Warp to honor your PS1 setting
resource-center-view-documentation = View documentation
resource-center-integrate-ide-title = Integrate Warp with your IDE
resource-center-integrate-ide-description = Configure Warp to launch from your most used development tools
resource-center-how-warp-uses-warp-title = How Warp uses Warp
resource-center-how-warp-uses-warp-description = Learn how Warp's engineering team uses their favorite features
resource-center-read-article = Read article
resource-center-command-search-title = Command search
resource-center-command-search-description = Find and run previously executed commands, workflows, and more.
resource-center-ai-command-search-title = AI command search
resource-center-ai-command-search-description = Generate shell commands with natural language.
resource-center-split-panes-title = Split panes
resource-center-split-panes-description = Split tabs into multiple panes to make your ideal layout.
resource-center-launch-configuration-title = Launch configuration
resource-center-launch-configuration-description = Save your current configuration of windows, tabs, and panes.
notebook-link-new-session = New session
notebook-link-new-session-tooltip = Open a new terminal session in this directory
notebook-link-open-terminal-session = Open in terminal session
notebook-link-open-in-editor = Open in editor
notebook-link-edit-markdown-file = Edit Markdown file
auth-token-placeholder = Auth Token
sharing-inherited-from-prefix = Inherited from {" "}
sharing-inherited-permission-label = Inherited permission
sharing-inherited-permissions-edit-parent-tooltip = Edit inherited permissions on the parent folder
sharing-inherited-permissions-cannot-edit-tooltip = Cannot edit inherited permissions
command-palette-navigation-running = Running...
command-palette-navigation-completed-over-hour = Completed over 1 hour ago
command-palette-navigation-completed-minute-ago = Completed { $mins } minute ago
command-palette-navigation-completed-minutes-ago = Completed { $mins } minutes ago
command-palette-navigation-no-timestamp = No timestamp found
command-palette-navigation-completed = Completed
command-palette-navigation-empty-session = Empty Session
terminal-history-tab-commands = Commands
terminal-history-tab-prompts = Prompts
common-current = Current
auth-browser-token-placeholder = Browser auth token
requested-script-expand-to-show = Expand to show script
common-hide = Hide
terminal-message-new-conversation = {" "}new conversation
agent-message-bar-again-send-to-agent = again to send to agent

# =============================================================================
# SECTION: remaining-ui-surfaces (Owner: codex-i18n-remaining-ui-surfaces)
# Files: onboarding slides, auth modal, voice, launch configs, notebook file state,
#        resource center, theme picker, terminal banners, AI footer/tool output
# =============================================================================

onboarding-intention-title = Welcome to Warp
onboarding-intention-subtitle = How do you want to work?
onboarding-intention-agent-title = Build faster with AI agents
onboarding-intention-agent-description = An agent-first experience with best in class terminal support. Get terminal and agent driven development AI features like:
onboarding-intention-terminal-title = Just use the terminal
onboarding-intention-terminal-badge = No AI features
onboarding-intention-terminal-description = A modern terminal optimized for speed, context, and control without AI.
onboarding-ai-feature-warp-agents = Warp agents
onboarding-ai-feature-oz-cloud-agents-platform = Oz local agents platform
onboarding-ai-feature-next-command-predictions = Next command predictions
onboarding-ai-feature-prompt-suggestions = Prompt suggestions
onboarding-ai-feature-remote-control-agents = Remote control with Claude Code, Codex, and other agents
onboarding-ai-feature-agents-over-ssh = Agents over SSH
onboarding-agent-title = Customize your Warp Agent
onboarding-agent-subtitle = Select your in-app agent's defaults.
onboarding-agent-default-model = Default model
onboarding-agent-autonomy = Autonomy
onboarding-agent-set-by-team-workspace = Managed by local workspace policy
onboarding-agent-team-workspace-autonomy-description = Autonomy settings are configured by the local workspace policy.
onboarding-agent-autonomy-full-title = Full
onboarding-agent-autonomy-full-subtitle = Runs commands, writes code, and reads files without asking.
onboarding-agent-autonomy-partial-title = Partial
onboarding-agent-autonomy-partial-subtitle = Can plan, read files, and execute low-risk commands. Asks before making any changes or executing sensitive commands.
onboarding-agent-autonomy-none-title = None
onboarding-agent-autonomy-none-subtitle = Takes no actions without your approval.
onboarding-agent-disable-warp-agent = Disable Warp Agent
onboarding-project-title = Open a project
onboarding-project-subtitle = Set up a project to optimize it for coding in Warp.
onboarding-project-open-local-folder = Open local folder
onboarding-project-initialize-automatically = Initialize project automatically
onboarding-project-initialize-description = Prepares the project environment, builds an index of your code, and generates project rules—giving the agent deeper understanding and better performance.
onboarding-intro-already-have-account = Already have an account?{" "}
onboarding-intro-subtitle = A modern terminal with state of the art agents built in.
onboarding-get-started = Get started
onboarding-theme-title = Choose a theme
onboarding-theme-subtitle = Click or use arrow keys to select, Enter to confirm.
onboarding-theme-sync-with-os = Sync light/dark theme with OS
onboarding-third-party-title = Customize third party agents
onboarding-third-party-subtitle = Select defaults for using agents like Claude Code, Codex, and Gemini.
onboarding-third-party-cli-toolbar = CLI agent toolbar
onboarding-third-party-notifications = Notifications
onboarding-customize-title = Customize your Warp
onboarding-customize-subtitle = Tailor your features and UI to your working style.
onboarding-customize-tab-styling = Tab styling
onboarding-customize-vertical = Vertical
onboarding-customize-horizontal = Horizontal
onboarding-customize-conversation-history = Conversation history
onboarding-customize-file-explorer = File explorer
onboarding-customize-global-file-search = Global file search
onboarding-customize-warp-drive = Warp Drive
onboarding-customize-tools-panel = Tools panel
onboarding-customize-code-review = Code review

auth-opt-out-line-1 = OpenWarp stores onboarding choices locally.
auth-opt-out-line-2-prefix = You can adjust your{" "}
auth-privacy-settings-prefix = You can adjust your{" "}
auth-privacy-settings-ai-prefix = You can adjust your local AI preferences in{" "}
auth-privacy-settings = Privacy Settings
auth-local-privacy-note = OpenWarp stores onboarding choices locally on this device.
auth-terms-prefix = Continuing keeps this setup on your device.{" "}
auth-terms-of-service = Local setup
auth-log-in = Log in
auth-paste-token-from-browser = Click here to paste your token from the browser
auth-login-slide-title-warp-drive = Get started with Warp Drive
auth-login-slide-title-ai = Get started with AI
auth-login-slide-subtitle-warp-drive = Connect your account to save and share notebooks, workflows, and more across devices.
auth-login-slide-subtitle-ai = Connect your account to enable AI-powered planning, coding, and automation.
auth-disable-warp-drive = Disable Warp Drive
auth-disable-ai-features = Disable AI features
auth-enable-warp-drive = Enable Warp Drive
auth-enable-ai-features = Enable AI features
auth-browser-sign-in-one-line-title = Sign in on your browser to continue
auth-open-page-manually-line-prefix = {" "}and open
auth-open-page-manually-line-suffix = the page manually.
auth-disable-warp-drive-confirm-title = Are you sure you want to disable Warp Drive?
auth-disable-ai-features-confirm-title = Are you sure you want to disable AI features?
auth-disable-warp-drive-confirm-body = Warp Drive lets you save workflows and knowledge across devices and share them with your team. By continuing, you won't have access to the following features:
auth-disable-ai-features-confirm-body = Warp is better with AI. By continuing, you won't have access to any of the following features:
auth-feature-session-sharing = Session Sharing
auth-sign-up = Continue locally
auth-sign-in = Sign in
auth-already-have-account = Already have an account?{" "}
auth-dont-want-sign-in-now = Don't want to sign in right now?{" "}
auth-skip-for-now = Skip for now
auth-skip-login-confirm-title = Are you sure you want to skip login?
auth-skip-login-confirm-line-1 = You can sign up later, but some features, such as AI,
auth-skip-login-confirm-line-2-prefix = are only available to logged-in users.{" "}
auth-yes-skip-login = Yes, skip login
auth-require-login-ai-collaboration = Local AI features do not require a Warp account.
auth-require-login-drive-limit = Warp Drive objects are stored locally in OpenWarp.
auth-require-login-share = Sharing is unavailable in local OpenWarp builds.
auth-welcome-title = Welcome to Warp!
auth-sign-up-for-warp = Continue in OpenWarp
auth-browser-sign-in-title = Sign in on your browser\nto continue
auth-open-page-manually-suffix = and open the page manually.

voice-try-input = Try Voice Input
voice-input-enabled-toast = Voice input is enabled. You can also press and hold the `{ $key }` key to activate voice input (configure in Settings > AI > Voice)
voice-input-microphone-access-error = Failed to start voice input (you may need to enable Microphone access)
voice-transcription-disabled-microphone = Voice transcription is disabled because Microphone access was not granted.
voice-transcription = Voice transcription
voice-transcription-hold-key = Voice transcription (hold `{ $key }` key)

get-started-welcome-title = Welcome to Warp
get-started-subtitle = The Agentic Development Environment
theme-creator-theme-name = Theme name
theme-creator-background-color = Background color
theme-creator-image-subheader = Automatically generate a theme based on extracted colors from an image (.png, .jpg).
theme-creator-select-image = Select an image
theme-creator-selecting-image = Selecting image...
theme-creator-select-new-image = Select a new image
theme-creator-create-theme = Create theme
theme-creator-process-image-failed = Failed to process selected image. Please try again with a different image.
theme-chooser-current-description = Change your current theme.
theme-chooser-light-description = Pick a theme for when your system is in light mode.
theme-chooser-dark-description = Pick a theme for when your system is in dark mode.
theme-chooser-no-matching-themes = No matching themes!
resource-center-keyboard-shortcuts = Keyboard Shortcuts
resource-center-keybindings-essentials = Essentials
resource-center-keybindings-blocks = Blocks
resource-center-keybindings-input-editor = Input Editor
resource-center-keybindings-terminal = Terminal
resource-center-keybindings-fundamentals = Fundamentals

launch-config-save-success-prefix = Saved successfully to{" "}
launch-config-save-failure-already-exists = Failed to save. A launch configuration with the same name already exists.
launch-config-save-failure-other = An issue was encountered while saving.
launch-config-save-configuration = Save Configuration
launch-config-open-yaml-file = Open YAML File
launch-config-save-current-configuration = Save Current Configuration
launch-config-link-to-documentation = Link to Documentation
launch-config-save-modal-a11y-title = Save Config Modal
launch-config-save-modal-a11y-description = Type the name of the file to which you want to save your current configuration of windows, tabs, and panes. Use enter to save the launch configuration, esc to quit the save configuration modal.
launch-config-save-description-no-keybinding = This will save your current configuration of windows, tabs and panes to a file so you can easily open it again.
launch-config-save-description-with-keybinding = This will save your current configuration of windows, tabs and panes to a file so you can easily open it again with { $keybinding }.
launch-config-yaml-saved-to-prefix = \nThe YAML file is saved to{" "}
notebook-file-could-not-read = Could not read { $name }
notebook-file-loading = Loading { $name }...
notebook-file-missing-source = Missing source file

terminal-shared-session-reconnecting = Offline, trying to reconnect...
terminal-banner-p10k-supported = Powerlevel10k now supports Warp!{"  "}
terminal-banner-p10k-older-version-prefix = You seem to be running an older (unsupported) version, please follow{" "}
terminal-banner-these-instructions = these instructions
terminal-banner-update-latest-suffix = {" "}to update to the latest version.
terminal-banner-pure-unsupported = Pure is not yet supported in Warp. You might consider one of the supported prompts as an alternative.{"  "}
terminal-loading-session = Loading session...

ai-footer-hide-rich-input = Hide Rich Input
ai-footer-choose-environment = Choose an environment
ai-footer-agent-environment = Agent environment
ai-footer-enable-terminal-command-autodetection = Enable terminal command autodetection
ai-footer-disable-terminal-command-autodetection = Disable terminal command autodetection
ai-footer-turn-off-auto-approve-agent-actions = Turn off auto-approve all agent actions
ai-footer-auto-approve-agent-actions-for-task = Auto-approve all agent actions for this task
ai-footer-start-remote-control = Start remote control
ai-footer-login-required-remote-control = Log in to use /remote-control
ai-footer-see-logs-for-details = See logs for details
ai-footer-plugin-installed-restart-session = Warp plugin installed. Please restart the session to activate.
ai-footer-installing-warp-plugin = Installing Warp plugin...
ai-footer-failed-install-warp-plugin = Failed to install Warp plugin
ai-footer-plugin-updated-restart-session = Warp plugin updated. Please restart the session to activate.
ai-footer-updating-warp-plugin = Updating Warp plugin...
ai-footer-failed-update-warp-plugin = Failed to update Warp plugin
voice-input-limit-reached = Voice input limit reached
voice-input-transcription-failed = Failed to transcribe voice input
ai-toolbar-context-chip = Context Chip
ai-toolbar-model-selector = Model Selector
ai-toolbar-autodetection = Autodetection
ai-toolbar-voice-input = Voice Input
ai-toolbar-attach-file = Attach File
ai-toolbar-context-usage = Context Usage
ai-toolbar-file-explorer = File Explorer
ai-toolbar-rich-input = Rich Input
ai-toolbar-fast-forward = Fast Forward
ai-tool-output-grep-for = Grep for{" "}
ai-tool-output-grepping-for = Grepping for{" "}
ai-tool-output-in-path-cancelled = {" "}in { $path } cancelled
ai-tool-output-in-path = {" "}in { $path }
ai-tool-output-grep-patterns-cancelled = Cancelled grep for the following patterns in { $path }
ai-tool-output-grep-patterns-queued = Grep for the following patterns in { $path }
ai-tool-output-grep-patterns-running = Grepping for the following patterns in { $path }
ai-tool-output-search-files-match = Search for files that match{" "}
ai-tool-output-finding-files-match = Finding files that match{" "}
ai-tool-output-file-patterns-cancelled = Cancelled search for files that match the following patterns in { $path }
ai-tool-output-file-patterns-queued = Find files that match the following patterns in { $path }
ai-tool-output-file-patterns-running = Finding files that match the following patterns in { $path }
ai-tool-output-listing-messages = Listing messages
ai-tool-output-grepping-patterns = Grepping for patterns
ai-tool-output-grepping-patterns-with-query = Grepping for patterns: { $query }
ai-tool-output-reading-messages = Reading { $count } messages

code-review-discard-uncommitted-changes-title = Discard uncommitted changes?
code-review-discard-file-uncommitted-changes-title = Discard all uncommitted changes to file?
code-review-discard-all-changes-title = Discard all changes?
code-review-discard-file-changes-title = Discard all changes to file?
code-review-discard-uncommitted-changes-description = You're about to discard all local changes that haven't been committed.
code-review-discard-file-uncommitted-changes-description = This will restore this file to the last committed version and discard local edits.
code-review-discard-all-changes-description = You're about to discard all committed and uncommitted changes.
code-review-discard-file-main-branch-description = This will restore this file to the main branch version and discard all committed and uncommitted edits.
code-review-discard-file-branch-description = This will reset this file to the { $branch } branch version and discard all committed and uncommitted edits.
code-review-stash-changes = Stash changes
code-review-no-changes-to-commit = No changes to commit
code-review-no-git-actions-available = No git actions available
command-search-out-of-credits-contact-admin = Looks like you're out of credits. Contact a team admin to upgrade for more credits.
command-search-out-of-credits-prefix = Looks like you're out of credits.{" "}
command-search-for-more-credits-suffix = {" "}for more credits.
search-not-visible-to-other-users = Not visible to other users
sharing-invite = Invite
sharing-who-has-access = Who has access
terminal-shared-session-cancel-request = Cancel request
terminal-shared-session-continue-sharing = Continue sharing
settings-import-reset-to-warp-defaults = Reset to Warp defaults
settings-import-type-theme = Theme
settings-import-type-theme-with-comma = Theme,
settings-import-type-option-as-meta = Option as Meta
settings-import-type-mouse-scroll-reporting = Mouse/Scroll Reporting
settings-import-type-font = Font
settings-import-type-default-shell = Default Shell
settings-import-type-working-directory = Working Directory
settings-import-type-global-hotkey = Global hotkey
settings-import-type-window-dimensions = Window Dimensions
settings-import-type-copy-on-select = Copy On Select
settings-import-type-window-opacity = Window Opacity
settings-import-type-cursor-blinking = Cursor Blinking
settings-import-one-other-setting = 1 other setting
settings-import-other-settings = { $count } other settings
workflow-argument-editor-helper = Fill out the arguments in this workflow and copy it to run in your terminal session
workflow-add-environment-variables = Add environment variables
workflow-environment-variables = Environment variables
workflow-new-environment-variables = New environment variables
ai-history-completed-successfully = Completed successfully
ai-history-pending = Pending
ai-history-cancelled-by-user = Cancelled by user
ai-block-always-allow = Always allow
ai-cancel-summarization = Cancel summarization
ai-continue-summarization = Continue summarization
ai-dont-show-suggested-code-banners-again = Don't show me suggested code banners again
ai-inline-code-diff-no-file-name = No file name
ai-tool-call-cancelled = Tool call was cancelled
ai-agent-view-open-in-different-pane = Open in different pane
passive-suggestion-feature-or-bug-label = Code a feature or fix a bug in {1}
passive-suggestion-help-feature-or-bug-label = Help me code a feature or fix a bug in {1}
passive-suggestion-implement-feature-or-bug-query = Implement a feature or fix a bug in {1}. Ask me for all the details you need.
passive-suggestion-create-pull-request-query = Help me create a pull request.
passive-suggestion-start-new-project-label = Help me start a new project
passive-suggestion-start-new-project-query = Help me start a new project. Ask me for all the details you need.
passive-suggestion-node-project-label = Help me start a Node.js project
passive-suggestion-node-project-query = Help me start a Node.js project. Ask me for all the details you need.
passive-suggestion-react-app-label = Help me create a new React app
passive-suggestion-react-app-query = Help me create a new React app called {1}. Ask me for all the details you need.
passive-suggestion-next-app-label = Help me create a new Next.js app
passive-suggestion-next-app-query = Help me create a new Next.js app called {1}. Ask me for all the details you need.
passive-suggestion-rust-project-label = Help me start a Rust project for {1}
passive-suggestion-rust-project-query = Help me start a Rust project for {1}. Ask me for all the details you need.
passive-suggestion-poetry-project-label = Help me start a Poetry project for {1}
passive-suggestion-poetry-project-query = Help me start a Poetry project for {1}. Ask me for all the details you need.
passive-suggestion-django-project-label = Help me start a Django project for {1}
passive-suggestion-django-project-query = Help me start a Django project for {1}. Ask me for all the details you need.
passive-suggestion-rails-app-label = Help me start a Rails app for {1}
passive-suggestion-rails-app-query = Help me start a Rails app for {1}. Ask me for all the details you need.
passive-suggestion-gradle-maven-project-label = Help me start a Gradle/Maven project
passive-suggestion-gradle-maven-project-query = Help me start a Gradle/Maven project. Ask me for all the details you need.
passive-suggestion-go-project-label = Help me start a Go project for {1}
passive-suggestion-go-project-query = Help me start a Go project for {1}. Ask me for all the details you need.
passive-suggestion-swift-project-label = Help me start a Swift project
passive-suggestion-swift-project-query = Help me start a Swift project. Ask me for all the details you need.
passive-suggestion-terraform-config-label = Help me start a Terraform configuration
passive-suggestion-terraform-config-query = Help me start a Terraform configuration. Ask me for all the details you need.
passive-suggestion-prisma-setup-label = Help me set up Prisma in this project
passive-suggestion-prisma-setup-query = Help me set up Prisma in this project.
passive-suggestion-install-dependencies-query = Help me install dependencies for {1}.
passive-suggestion-ruby-project-label = Help me set up a new Ruby project
passive-suggestion-ruby-project-query = Help me set up a new Ruby project. Ask me for all the details you need.
passive-suggestion-modelfile-query = Help me set up a Modelfile for {1}.
passive-suggestion-kubernetes-utilization-query = Help me understand resource utilization in my cluster.
passive-suggestion-kubernetes-inspect-query = Help me inspect Kubernetes resources.
passive-suggestion-docker-containers-query = Help me manage running containers.
passive-suggestion-docker-images-query = Help me manage Docker images.
passive-suggestion-docker-compose-label = Help me manage or troubleshoot {1} with Docker Compose
passive-suggestion-docker-compose-query = Help me manage or troubleshoot {1} with Docker Compose.
passive-suggestion-docker-network-query = Help me configure containers to use {1}.
passive-suggestion-vagrant-box-query = Help me set up or customize a Vagrant box {1}.
passive-suggestion-vagrant-up-query = Help me provision my environment or troubleshoot Vagrant startup.
passive-suggestion-grep-search-query = Help me search code across files for {1}.
passive-suggestion-find-search-query = Help me search code across files with {1}.
passive-suggestion-ssh-keygen-query = Walk me through generating an SSH key.

# =============================================================================
# SECTION: remaining-ui-surfaces (Owner: agent-i18n-remaining)
# Files: app/src/workspace, app/src/terminal, app/src/code, app/src/notebooks,
#        app/src/ai, app/src/settings_view, app/src/workflows, app/src/view_components
# =============================================================================

common-update = Update
common-reject = Reject
common-open-link = Open link
common-open-file = Open file
common-open-folder = Open folder
common-name = Name
common-rule = Rule
common-skip-for-now = Skip for now
common-never = Never
common-save-changes = Save changes
common-do-not-show-again = Do not show again
common-dont-show-again-with-period = Don't show again.
common-refresh = Refresh
common-resource-not-found-or-access-denied = Resource not found or access denied
workspace-close-session = Close session
workspace-auto-reload = Auto-reload
workspace-add-new-repo = {" "}+ Add new repo
workspace-notification-permission-denied-toast = Warp doesn't have permission to send desktop notifications.
workspace-troubleshoot-notifications-link = Troubleshoot notifications
workspace-plan-synced-to-warp-drive-toast = Plan synced to your Warp Drive
workspace-remote-control-link-copied-toast = Remote control link copied.
workspace-update-now = Update now
workspace-update-warp = Update Warp
workspace-app-out-of-date-needs-update = Your app is out of date and needs to update.
workspace-restart-app-and-update-now = Restart app and update now
workspace-sampling-process-toast = Sampling process for 3 seconds...
workspace-version-deprecation-banner = Your app is out of date and some features may not work as expected. Please update immediately.
workspace-version-deprecation-without-permissions-banner = Some Warp features may not work as expected without updating immediately, but Warp is unable to perform the update.
workspace-new-version-unable-to-update-banner = A new version is available but Warp is unable to perform the update.
workspace-unable-to-launch-new-installed-version = Warp was unable to launch the new installed version.
tab-config-session-type = Session type
terminal-copy-error = Copy error
terminal-authenticate-with-github = Authenticate with GitHub
terminal-create-environment = Create an environment
terminal-regenerate-agents-file = Re-generate AGENTS.md file
terminal-view-index-status = View index status
terminal-shared-session-request-edit-access = Request edit access
terminal-create-team = Create team
terminal-warpify-without-tmux = Warpify without TMUX
terminal-continue-without-warpification = Continue without Warpification
terminal-always-install = Always install
terminal-never-install = Never install
terminal-ssh-report-issue-prefix = We are actively working on improving the stability of SSH in Warp. Please consider{" "}
terminal-ssh-report-issue-link = filing an issue
terminal-ssh-report-issue-suffix = {" "}on GitHub so we can better identify the problem.
terminal-ssh-why-need-tmux = Why do I need tmux?
terminal-ssh-file-uploads-title = File Uploads
terminal-ssh-close-upload-session = Close upload session
terminal-ssh-view-upload-session = View upload session
terminal-reveal-secret = Reveal secret
terminal-hide-secret = Hide secret
terminal-copy-secret = Copy secret
terminal-tag-agent-for-assistance = Tag agent for assistance
terminal-save-as-workflow-secrets-tooltip = Blocks containing secrets cannot be saved.
terminal-agent-mode-setup-title = Optimize Warp for this codebase?
terminal-agent-mode-setup-description = Unlock smarter, more consistent responses by letting the Agent understand your codebase and generate rules for it. You can also do this at any point by running /init
terminal-agent-mode-setup-optimize = Optimize
terminal-no-active-conversation-to-export = No active conversation to export
terminal-slow-shell-startup-banner-prefix = Seems like your shell is taking a while to start...{"  "}
terminal-more-info = More info
terminal-show-initialization-block = Show initialization block
terminal-shell-process-exited = Shell process exited
terminal-shell-process-could-not-start = Shell process could not start!
terminal-shell-process-exited-prematurely = Shell process exited prematurely!
terminal-shell-premature-subtext = Something went wrong while starting { $shell_detail } and Warpifying it, causing the process to terminate. Warpify script output is displayed here, which may point at a cause.
terminal-file-issue = File issue
notifications-banner-troubleshoot = Troubleshoot
notifications-banner-dismissed-title = We won't show this banner again, but you can always go to Settings to enable notifications.
notifications-banner-disabled-title = Notifications were turned off, but you can always go to Settings to enable notifications.
notifications-banner-enable = Enable
notifications-banner-permissions-accepted-title = Success! You are now ready to receive desktop notifications.
notifications-banner-permissions-denied-title = Warp was denied permissions to send you notifications.
notifications-banner-permissions-error-title = Something went wrong while requesting permissions.
notifications-banner-allow-permissions-title = Don't forget to 'Allow' the permissions request to finish setting up notifications.
notifications-banner-configure-notifications = Configure notifications
notifications-banner-set-permissions = Set permissions
ai-edit-api-keys = Edit API Keys
ai-manage-privacy-settings = Manage privacy settings
ai-block-manage-agent-permissions = Manage Agent permissions
agent-zero-state-visit-docs = Visit docs
ai-execution-profile-agent-decides = Agent decides
ai-execution-profile-always-ask = Always ask
ai-execution-profile-ask-on-first-write = Ask on first write
ai-execution-profile-never-ask = Never ask
ai-execution-profile-ask-unless-auto-approve = Ask unless auto-approve
code-accept-and-save = Accept and save
code-hunk-label = Hunk:
code-discard-this-version = Discard this version
code-overwrite = Overwrite
code-review-send-to-agent = Send to Agent
code-review-open-pr = Open PR
code-review-pr-created-toast = PR successfully created.
code-review-comments-sent-to-agent = Comments sent to agent
code-review-could-not-submit-comments = Could not submit comments to the agent
code-review-tooltip-view-changes = View changes
code-review-diffs-local-workspaces-only = Diffs only work for local workspaces.
code-review-diffs-git-repositories-only = Diffs only work for git repositories.
code-review-diffs-wsl-unsupported = Diffs don't currently work in WSL.
code-review-generating-commit-message-placeholder = Generating commit message…
code-review-type-commit-message-placeholder = Type a commit message
code-review-committing-loading = Committing…
code-review-commit-message-label = Commit message
code-review-no-non-outdated-comments-to-send = No non-outdated comments to send
code-review-send-diff-comments-to = Send diff comments to { $label }
code-review-ai-must-be-enabled-to-send-comments = AI must be enabled to send comments to Agent
code-review-agent-code-review-requires-ai-credits = Agent code review requires AI credits
code-review-all-terminals-are-busy = All terminals are busy
code-review-send-diff-comments-to-agent = Send diff comments to Agent
code-failed-to-load-file-toast = Failed to load file.
code-failed-to-save-file-toast = Failed to save file.
code-file-saved-toast = File saved.
notebook-apply-link = Apply link
notebook-sync-conflict-resolution-message = This notebook could not be saved because changes were made while you were editing. Please copy your work and refresh.
notebook-sync-feature-not-available-message = This notebook could not be saved to the server because the feature is temporarily unavailable. The changes are saved locally. Please retry later.
notebook-link-copied-toast = Link copied
settings-share-with-team = Save locally
tooltip-secrets-not-sent-to-warp-server = *Secrets are not sent to Warp's server.
editor-voice-limit-hit-toast = You have hit the limit for Voice requests. Your limit will be refreshed as a part of your next cycle.
editor-voice-error-toast = An error occurred while processing your voice input.
ai-copied-branch-name-toast = Copied branch name
workflow-new-enum = New enum
workflow-edit-enum = Edit enum
workflow-enum-variant-placeholder = Variant
workflow-enum-variants = Variants
quit-warning-dont-save = Don't Save
quit-warning-show-running-processes = Show running processes
quit-warning-save-changes-title = Save changes?
