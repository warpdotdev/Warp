# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

### Application
app-name = Warp

### Generic actions
generic-cancel = Cancel
generic-delete = Delete
generic-close = Close
generic-save = Save
generic-remove = Remove
generic-upgrade = Upgrade
generic-compare-plans = Compare plans
generic-contact-support = Contact support

### Settings sections
settings-section-appearance = Appearance
settings-section-features = Features
settings-section-keybindings = Keyboard shortcuts
settings-section-about = About
settings-section-ai = AI
settings-section-account = Account
settings-section-privacy = Privacy
settings-section-teams = Teams
settings-section-warp-drive = Warp Drive
settings-section-warp-agent = Warp Agent
settings-section-profiles = Profiles
settings-section-knowledge = Knowledge
settings-section-mcp-servers = MCP Servers
settings-section-billing-and-usage = Billing and usage
settings-section-shared-blocks = Shared blocks
settings-section-mcp-servers-lower = MCP servers
settings-section-third-party-cli-agents = Third party CLI agents
settings-section-indexing-and-projects = Indexing and projects
settings-section-editor-and-code-review = Editor and Code Review
settings-section-environments = Environments
settings-section-oz-cloud-api-keys = Oz Cloud API Keys
settings-section-referrals = Referrals
settings-section-warpify = Warpify
settings-section-code = Code

### Appearance page
appearance-category-themes = Themes
appearance-category-icon = Icon
appearance-category-window = Window
appearance-category-input = Input
appearance-category-panes = Panes
appearance-category-blocks = Blocks
appearance-category-text = Text
appearance-category-cursor = Cursor
appearance-category-tabs = Tabs
appearance-category-full-screen-apps = Full-screen Apps
appearance-create-custom-theme = Create your own custom theme
appearance-theme-mode-light = Light
appearance-theme-mode-dark = Dark
appearance-theme-mode-current = Current theme
appearance-sync-with-os = Sync with OS
appearance-open-new-windows-custom-size = Open new windows with custom size
appearance-columns = Columns
appearance-rows = Rows
appearance-window-opacity = Window Opacity: { $value }
appearance-window-blur-radius = Window Blur Radius: { $value }
appearance-use-window-blur-acrylic = Use Window Blur (Acrylic texture)
appearance-input-type = Input type
appearance-input-position = Input position
appearance-dim-inactive-panes = Dim inactive panes
appearance-focus-follows-mouse = Focus follows mouse
appearance-compact-mode = Compact mode
appearance-line-height = Line height
appearance-agent-font = Agent font
appearance-terminal-font = Terminal font
appearance-cursor-type = Cursor type
appearance-input-type-warp = Warp
appearance-input-type-shell-ps1 = Shell (PS1)
appearance-opacity-not-supported = Transparency is not supported with your graphics drivers.
appearance-transparency-warning = The selected graphics settings may not support rendering transparent windows.
appearance-try-changing-graphics = Try changing the settings for the graphics backend or integrated GPU in Features > System.

### Settings UI
settings-info-tooltip = Click to learn more in docs
settings-local-only-tooltip = This setting is not synced to your other devices
settings-reset-to-default = Reset to default

### AI settings - Section headers
ai-agent-name = Warp Agent
ai-active-ai-section = Active AI
ai-input-section = Input
ai-knowledge-section = Knowledge
ai-other-section = Other
ai-voice-input-label = Voice Input

### AI settings - Toggle labels
ai-show-input-hint-text = Show input hint text
ai-show-agent-tips = Show agent tips
ai-include-agent-commands-history = Include agent-executed commands in history
ai-context-window-tokens = Context window (tokens)
ai-voice-input-enabled = Voice Input
ai-show-oz-changelog = Show Oz changelog in new conversation view
ai-show-use-agent-footer = Show "Use Agent" footer
ai-show-conversation-history = Show conversation history in tools panel
ai-orchestration = Orchestration

### AI settings - Toggle descriptions
ai-next-command-description = Let AI suggest the next command to run based on your command history, outputs, and common workflows.
ai-prompt-suggestions-description = Let AI suggest natural language prompts, as inline banners in the input, based on recent commands and their outputs.
ai-suggested-code-banners-description = Let AI suggest code diffs and queries as inline banners in the blocklist, based on recent commands and their outputs.
ai-natural-language-autosuggestions-description = Let AI suggest natural language autosuggestions, based on recent commands and their outputs.
ai-shared-block-title-generation-description = Let AI generate a title for your shared block based on the command and output.
ai-git-operations-autogen-description = Let AI generate commit messages and pull request titles and descriptions.
ai-agent-footer-tooltip-description = Shows hint to use the "Full Terminal Use"-enabled agent in long running commands.
ai-voice-input-description = Voice input allows you to control Warp by speaking directly to your terminal (powered by
ai-manage-rules = Manage rules
ai-remote-session-disabled = Your organization disallows AI when the active pane contains content from a remote session
ai-placeholder-read-allowlist = e.g. ~/code-repos/repo
ai-placeholder-commands-comma-separated = Commands, comma separated
ai-placeholder-command-allowlist = e.g. ls .*
ai-placeholder-command-denylist = e.g. rm .*
ai-placeholder-command-regex = command (supports regex)
ai-for-more-ai-usage = for more AI usage.
ai-to-get-more-ai-usage = to get more AI usage.

### Voice input key names
voice-key-none = None
voice-key-fn = Fn
voice-key-alt-left = Option/Alt (Left)
voice-key-alt-right = Option/Alt (Right)
voice-key-control-left = Control (Left)
voice-key-control-right = Control (Right)
voice-key-super-left = Command/Windows/Super (Left)
voice-key-super-right = Command/Windows/Super (Right)
voice-key-shift-left = Shift (Left)
voice-key-shift-right = Shift (Right)
voice-input-tooltip-with-key = Voice input (hold { $side } { $key } key)
voice-input-tooltip-no-key = Voice input

### Default session mode
default-session-mode-terminal = Terminal
default-session-mode-agent = Agent
default-session-mode-cloud-agent = Cloud Oz
default-session-mode-tab-config = Tab Config
default-session-mode-docker-sandbox = Local Docker Sandbox

### Thinking display mode
thinking-display-show-and-collapse = Show & collapse
thinking-display-always-show = Always show
thinking-display-never-show = Never show

### Dialogs
dialog-delete-conversation-title = Delete '{ $title }'?
dialog-delete-conversation-title-fallback = Delete conversation?
dialog-delete-conversation-body = This conversation will be permanently deleted. This action cannot be undone.
dialog-close-session-title = Close session?
dialog-close-session-body = You are about to close a session that is currently being shared. Closing it will end sharing for everyone.
dialog-dont-show-again = Don't show again.
dialog-button-close-session = Close session
dialog-unsaved-changes-title = You have unsaved changes.
dialog-button-keep-editing = Keep editing
dialog-button-discard-changes = Discard changes
dialog-remove-tab-config-title = Remove '{ $name }'?
dialog-remove-tab-config-body = This tab config will be permanently deleted. This action cannot be undone.
dialog-button-remove = Remove

### Quit warning
quit-warning-title-close-pane = Close pane?
quit-warning-title-close-tab = Close tab?
quit-warning-title-close-tabs = Close tabs?
quit-warning-title-close-window = Close window?
quit-warning-title-quit-warp = Quit Warp?
quit-warning-title-save-changes = Save changes?
quit-warning-button-yes-close = Yes, close
quit-warning-button-yes-quit = Yes, quit
quit-warning-button-save = Save
quit-warning-button-dont-save = Don't Save
quit-warning-button-show-processes = Show running processes
quit-warning-suffix-tab = in this tab.
quit-warning-suffix-window = in this window.
quit-warning-suffix-pane = in this pane.
quit-warning-suffix-default = .
quit-warning-processes-running = You have { $count } { $noun } running
quit-warning-processes-in-windows = in { $count } windows
quit-warning-processes-in-tabs = in { $count } tabs
quit-warning-sharing-sessions = You are sharing { $count } { $noun }
quit-warning-unsaved-file-changes = You have unsaved file changes
quit-warning-unsaved-file-editor = Do you want to save the changes you made to { $filename }? Your changes will be discarded if you don't save them.

### Alerts
alert-no-internet = No internet connection
alert-telemetry-disabled = To use AI features,
alert-enable-analytics = enable analytics
alert-upgrade-to-build = upgrade
alert-at-limit = At Limit -
alert-payment-issue = Restricted due to payment issue
alert-out-of-credits = Out of credits
alert-sign-up-credits = Sign up for more AI credits
alert-manage-billing = Manage billing
alert-enable-overages = Enable premium overages
alert-increase-spend-limit = Increase monthly spend limit
alert-upgrade = Upgrade
alert-compare-plans = Compare plans
alert-contact-support = Contact support
alert-contact-admin = , contact a team admin
alert-ask-admin-enable-overages = , ask a team admin to enable overages
alert-ask-admin-increase-overages = , ask a team admin to increase overages
alert-or = or
alert-upgrade-to-build-action = Upgrade to Build
alert-or-use-own-api = use your own API keys
alert-add-credits = Add credits

### Setting descriptions
setting-is-any-ai-enabled = Controls whether all AI features are enabled.
setting-is-active-ai-enabled = Controls whether proactive AI features like suggestions are enabled.
setting-ai-autodetection-enabled = Controls whether AI automatically detects natural language input.
setting-nld-in-terminal-enabled = Controls whether natural language detection is enabled in the terminal input.
setting-autodetection-command-denylist = Commands to exclude from AI natural language autodetection.
setting-intelligent-autosuggestions-enabled = Controls whether AI-powered intelligent autosuggestions are enabled.
setting-prompt-suggestions-enabled = Controls whether prompt suggestions are shown in agent mode.
setting-code-suggestions-enabled = Controls whether AI code suggestions are enabled.
setting-natural-language-autosuggestions-enabled = Controls whether ghosted text autosuggestions are shown for AI input queries.
setting-shared-block-title-generation-enabled = Controls whether titles are auto-generated when sharing blocks.
setting-git-operations-autogen-enabled = Controls whether AI auto-generates commit messages and PR title/body in the code review dialogs.
setting-rule-suggestions-enabled = Controls whether the agent suggests rules to save after responses.
setting-voice-input-enabled = Controls whether voice input is enabled for AI interactions.
setting-agent-mode-command-execution-allowlist = Commands that the agent can execute without explicit permission.
setting-agent-mode-command-execution-denylist = Commands that the agent must always ask before executing.
setting-agent-mode-execute-readonly-commands = Whether the agent can auto-execute read-only commands without asking.
setting-agent-mode-coding-permissions = The file read permission level for the agent.
setting-agent-mode-coding-file-read-allowlist = File paths the agent can read without asking for permission.
setting-aws-bedrock-credentials-enabled = Whether Warp should use your local AWS credentials for Bedrock-enabled requests.
setting-aws-bedrock-auto-login = Whether to automatically run the AWS login command when Bedrock credentials expire.
setting-aws-bedrock-auth-refresh-command = The command to run to refresh AWS credentials for Bedrock.
setting-aws-bedrock-profile = The AWS profile name to use for Bedrock credentials.
setting-memory-enabled = Whether the agent uses your saved rules during requests.
setting-warp-drive-context-enabled = Whether Warp Drive context is included in AI requests.
setting-should-show-oz-updates = Whether the "What's new" section is shown in the agent view.
setting-can-use-warp-credits-with-byok = Whether Warp credits can be used even when providing your own API key.
setting-should-render-use-agent-footer = Whether to show the "Use Agent" footer for terminal commands.
setting-should-render-cli-agent-footer = Whether to show the CLI agent footer for coding agent commands.
setting-auto-toggle-rich-input = Whether CLI agent Rich Input automatically closes and reopens based on the agent's blocked state.
setting-auto-open-rich-input = Whether CLI agent Rich Input automatically opens when a CLI agent session starts.
setting-auto-dismiss-rich-input = Whether CLI agent Rich Input automatically closes after the user submits a prompt.
setting-cli-agent-footer-enabled-commands = Maps custom toolbar command patterns to specific CLI agents.
setting-cloud-agent-computer-use-enabled = Whether computer use is enabled for cloud agent conversations.
setting-orchestration-enabled = Whether multi-agent orchestration is enabled.
setting-file-based-mcp-enabled = Whether third-party file-based MCP servers are automatically detected.
setting-thinking-display-mode = Controls how agent thinking traces are displayed after streaming.
setting-default-session-mode = The default mode for new terminal sessions.
setting-voice-input-toggle-key = The key used to toggle voice input.
setting-include-agent-commands-in-history = Whether agent-executed commands are included in command history.
setting-show-conversation-history = Whether conversation history appears in the tools panel.
setting-show-agent-notifications = Whether agent notifications are shown.
setting-agent-attribution-enabled = Whether the Warp Agent adds an attribution co-author line to commit messages and pull requests it creates.

### Command palette
cmd-palette-compact-mode = compact mode
cmd-palette-sync-with-os = themes: sync with OS
cmd-palette-cursor-blink = cursor blink
cmd-palette-jump-to-bottom = jump to bottom of block button
cmd-palette-block-dividers = block dividers
cmd-palette-dim-inactive-panes = dim inactive panes
cmd-palette-start-input-top = Start Input at the Top
cmd-palette-pin-input-top = Pin Input to the Top
cmd-palette-pin-input-bottom = Pin Input to the Bottom
cmd-palette-toggle-input-mode = Toggle Input Mode (Warp/Classic)
cmd-palette-tab-indicators = tab indicators
cmd-palette-show-code-review-button = Show code review button in tab bar
cmd-palette-hide-code-review-button = Hide code review button in tab bar
cmd-palette-focus-follows-mouse = focus follows mouse
cmd-palette-always-show-tab-bar = Always show tab bar
cmd-palette-hide-tab-bar-fullscreen = Hide tab bar if fullscreen
cmd-palette-show-tab-bar-on-hover = Only show tab bar on hover
cmd-palette-zen-mode = zen mode
cmd-palette-vertical-tab-layout = vertical tab layout
cmd-palette-vertical-tabs-restored = show vertical tabs panel in restored windows
cmd-palette-ligature-rendering = ligature rendering
cmd-palette-ai-toggle = AI
cmd-palette-active-ai-toggle = Active AI
cmd-palette-autodetection = terminal command autodetection in agent input / natural language detection
cmd-palette-nld-terminal = agent prompt autodetection in terminal input
cmd-palette-next-command = Next Command
cmd-palette-prompt-suggestions = prompt suggestions
cmd-palette-code-suggestions = code suggestions
cmd-palette-show-agent-tips = Show agent tips
cmd-palette-hide-agent-tips = Hide agent tips
cmd-palette-show-oz-changelog = Show Oz changelog in new agent conversation view
cmd-palette-hide-oz-changelog = Hide Oz changelog in new agent conversation view
cmd-palette-natural-language-autosuggestions = natural language autosuggestions
cmd-palette-shared-block-title-generation = shared block title generation
cmd-palette-voice-input = voice input
cmd-palette-show-use-agent-footer = Show "Use Agent" footer
cmd-palette-hide-use-agent-footer = Hide "Use Agent" footer
cmd-palette-codebase-index = codebase index
cmd-palette-thinking-show-collapse = Set agent thinking display: show & collapse
cmd-palette-thinking-always-show = Set agent thinking display: always show
cmd-palette-thinking-never-show = Set agent thinking display: never show

### Locale
locale-label = Language
locale-en-us = English
locale-zh-cn = 简体中文

### Language/locale setting description
setting-locale = The display language for the Warp UI.
