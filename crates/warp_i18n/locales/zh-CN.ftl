# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

### Application
app-name = Warp

### Generic actions
generic-cancel = 取消
generic-delete = 删除
generic-close = 关闭
generic-save = 保存
generic-remove = 移除
generic-upgrade = 升级
generic-compare-plans = 比较方案
generic-contact-support = 联系支持

### Settings sections
settings-section-appearance = 外观
settings-section-features = 功能
settings-section-keybindings = 键盘快捷键
settings-section-about = 关于
settings-section-ai = AI
settings-section-account = 账户
settings-section-privacy = 隐私
settings-section-teams = 团队
settings-section-warp-drive = Warp Drive
settings-section-warp-agent = Warp 智能体
settings-section-profiles = 配置文件
settings-section-knowledge = 知识库
settings-section-mcp-servers = MCP 服务器
settings-section-billing-and-usage = 账单与用量
settings-section-shared-blocks = 共享块
settings-section-mcp-servers-lower = MCP 服务器
settings-section-third-party-cli-agents = 第三方 CLI 智能体
settings-section-indexing-and-projects = 索引与项目
settings-section-editor-and-code-review = 编辑器和代码审查
settings-section-environments = 环境
settings-section-oz-cloud-api-keys = Oz Cloud API 密钥
settings-section-referrals = 推荐
settings-section-warpify = Warpify
settings-section-code = 代码

### Appearance page
appearance-category-themes = 主题
appearance-category-icon = 图标
appearance-category-window = 窗口
appearance-category-input = 输入
appearance-category-panes = 窗格
appearance-category-blocks = 块
appearance-category-text = 文本
appearance-category-cursor = 光标
appearance-category-tabs = 标签页
appearance-category-full-screen-apps = 全屏应用
appearance-create-custom-theme = 创建自定义主题
appearance-theme-mode-light = 浅色
appearance-theme-mode-dark = 深色
appearance-theme-mode-current = 当前主题
appearance-sync-with-os = 跟随系统
appearance-open-new-windows-custom-size = 使用自定义尺寸打开新窗口
appearance-columns = 列数
appearance-rows = 行数
appearance-window-opacity = 窗口不透明度：{ $value }
appearance-window-blur-radius = 窗口模糊半径：{ $value }
appearance-use-window-blur-acrylic = 使用窗口模糊（亚克力纹理）
appearance-input-type = 输入类型
appearance-input-position = 输入位置
appearance-dim-inactive-panes = 调暗非活动窗格
appearance-focus-follows-mouse = 焦点跟随鼠标
appearance-compact-mode = 紧凑模式
appearance-line-height = 行高
appearance-agent-font = 智能体字体
appearance-terminal-font = 终端字体
appearance-cursor-type = 光标类型
appearance-input-type-warp = Warp
appearance-input-type-shell-ps1 = Shell (PS1)
appearance-opacity-not-supported = 你的显卡驱动不支持透明度设置。
appearance-transparency-warning = 当前选择的图形设置可能不支持渲染透明窗口。
appearance-try-changing-graphics = 请尝试在「功能 > 系统」中更改图形后端或集成 GPU 的设置。

### Settings UI
settings-info-tooltip = 点击了解文档详情
settings-local-only-tooltip = 此设置不会同步到其他设备
settings-reset-to-default = 重置为默认

### AI settings - Section headers
ai-agent-name = Warp 智能体
ai-active-ai-section = 主动 AI
ai-input-section = 输入
ai-knowledge-section = 知识库
ai-other-section = 其他
ai-voice-input-label = 语音输入

### AI settings - Toggle labels
ai-show-input-hint-text = 显示输入提示文本
ai-show-agent-tips = 显示智能体提示
ai-include-agent-commands-history = 将智能体执行的命令包含在历史记录中
ai-context-window-tokens = 上下文窗口（Token 数）
ai-voice-input-enabled = 语音输入
ai-show-oz-changelog = 在新对话视图中显示 Oz 更新日志
ai-show-use-agent-footer = 显示「使用智能体」底栏
ai-show-conversation-history = 在工具面板中显示对话历史
ai-orchestration = 多智能体编排

### AI settings - Toggle descriptions
ai-next-command-description = 让 AI 根据你的命令历史、输出和常见工作流来建议下一条要运行的命令。
ai-prompt-suggestions-description = 让 AI 根据最近的命令及其输出，在输入框中以内联横幅的形式建议自然语言提示。
ai-suggested-code-banners-description = 让 AI 根据最近的命令及其输出，在块列表中以横幅形式建议代码 diff 和查询。
ai-natural-language-autosuggestions-description = 让 AI 根据最近的命令及其输出，建议自然语言自动补全。
ai-shared-block-title-generation-description = 让 AI 根据命令和输出为共享块自动生成标题。
ai-git-operations-autogen-description = 让 AI 在代码审查对话框中自动生成提交信息和 PR 标题/描述。
ai-agent-footer-tooltip-description = 在长时间运行的命令中，提示使用具有「完整终端使用」权限的智能体。
ai-voice-input-description = 语音输入让你可以直接对终端说话来控制 Warp（由
ai-manage-rules = 管理规则
ai-remote-session-disabled = 当活动窗格包含来自远程会话的内容时，你的组织禁止使用 AI
ai-placeholder-read-allowlist = 例如 ~/code-repos/repo
ai-placeholder-commands-comma-separated = 命令，以逗号分隔
ai-placeholder-command-allowlist = 例如 ls .*
ai-placeholder-command-denylist = 例如 rm .*
ai-placeholder-command-regex = 命令（支持正则表达式）
ai-for-more-ai-usage = 以获取更多 AI 使用量。
ai-to-get-more-ai-usage = 以获取更多 AI 使用量。

### Voice input key names
voice-key-none = 无
voice-key-fn = Fn
voice-key-alt-left = Option/Alt (左)
voice-key-alt-right = Option/Alt (右)
voice-key-control-left = Control (左)
voice-key-control-right = Control (右)
voice-key-super-left = Command/Windows/Super (左)
voice-key-super-right = Command/Windows/Super (右)
voice-key-shift-left = Shift (左)
voice-key-shift-right = Shift (右)
voice-input-tooltip-with-key = 语音输入（按住 { $side } { $key } 键）
voice-input-tooltip-no-key = 语音输入

### Default session mode
default-session-mode-terminal = 终端
default-session-mode-agent = 智能体
default-session-mode-cloud-agent = Cloud Oz
default-session-mode-tab-config = 标签配置
default-session-mode-docker-sandbox = 本地 Docker 沙箱

### Thinking display mode
thinking-display-show-and-collapse = 显示并折叠
thinking-display-always-show = 始终显示
thinking-display-never-show = 从不显示

### Dialogs
dialog-delete-conversation-title = 删除「{ $title }」？
dialog-delete-conversation-title-fallback = 删除对话？
dialog-delete-conversation-body = 此对话将被永久删除。此操作无法撤销。
dialog-close-session-title = 关闭会话？
dialog-close-session-body = 你正在关闭一个正在共享的会话。关闭将终止所有人的共享。
dialog-dont-show-again = 不再显示。
dialog-button-close-session = 关闭会话
dialog-unsaved-changes-title = 你有未保存的更改。
dialog-button-keep-editing = 继续编辑
dialog-button-discard-changes = 放弃更改
dialog-remove-tab-config-title = 移除「{ $name }」？
dialog-remove-tab-config-body = 此标签配置将被永久删除。此操作无法撤销。
dialog-button-remove = 移除

### Quit warning
quit-warning-title-close-pane = 关闭窗格？
quit-warning-title-close-tab = 关闭标签页？
quit-warning-title-close-tabs = 关闭标签页？
quit-warning-title-close-window = 关闭窗口？
quit-warning-title-quit-warp = 退出 Warp？
quit-warning-title-save-changes = 保存更改？
quit-warning-button-yes-close = 是，关闭
quit-warning-button-yes-quit = 是，退出
quit-warning-button-save = 保存
quit-warning-button-dont-save = 不保存
quit-warning-button-show-processes = 显示正在运行的进程
quit-warning-suffix-tab = 在此标签页中。
quit-warning-suffix-window = 在此窗口中。
quit-warning-suffix-pane = 在此窗格中。
quit-warning-suffix-default = 。
quit-warning-processes-running = 你有 { $count } 个{ $noun }正在运行
quit-warning-processes-in-windows = 在 { $count } 个窗口中
quit-warning-processes-in-tabs = 在 { $count } 个标签页中
quit-warning-sharing-sessions = 你正在共享 { $count } 个{ $noun }
quit-warning-unsaved-file-changes = 你有未保存的文件更改
quit-warning-unsaved-file-editor = 是否要保存对 { $filename } 的更改？如果不保存，更改将丢失。

### Alerts
alert-no-internet = 无网络连接
alert-telemetry-disabled = 要使用 AI 功能，
alert-enable-analytics = 启用分析
alert-upgrade-to-build = 升级
alert-at-limit = 已达上限 -
alert-payment-issue = 因支付问题受限
alert-out-of-credits = 信用额度已用完
alert-sign-up-credits = 注册获取更多 AI 信用额度
alert-manage-billing = 管理账单
alert-enable-overages = 启用超额付费
alert-increase-spend-limit = 增加月度消费上限
alert-upgrade = 升级
alert-compare-plans = 比较方案
alert-contact-support = 联系支持
alert-contact-admin = ，请联系团队管理员
alert-ask-admin-enable-overages = ，请让团队管理员启用超额付费
alert-ask-admin-increase-overages = ，请让团队管理员增加超额上限
alert-or = 或
alert-upgrade-to-build-action = 升级到 Build 方案
alert-or-use-own-api = 使用自己的 API 密钥
alert-add-credits = 添加信用额度

### Setting descriptions
setting-is-any-ai-enabled = 控制是否启用所有 AI 功能。
setting-is-active-ai-enabled = 控制是否启用主动 AI 功能（如建议）。
setting-ai-autodetection-enabled = 控制 AI 是否自动检测自然语言输入。
setting-nld-in-terminal-enabled = 控制是否在终端输入中启用自然语言检测。
setting-autodetection-command-denylist = 要从 AI 自然语言自动检测中排除的命令。
setting-intelligent-autosuggestions-enabled = 控制是否启用 AI 驱动的智能自动建议。
setting-prompt-suggestions-enabled = 控制是否在智能体模式下显示提示建议。
setting-code-suggestions-enabled = 控制是否启用 AI 代码建议。
setting-natural-language-autosuggestions-enabled = 控制是否对 AI 输入查询显示虚影文本自动建议。
setting-shared-block-title-generation-enabled = 控制共享块时是否自动生成标题。
setting-git-operations-autogen-enabled = 控制 AI 是否在代码审查对话框中自动生成提交信息和 PR 标题/描述。
setting-rule-suggestions-enabled = 控制智能体是否在回复后建议保存规则。
setting-voice-input-enabled = 控制是否为 AI 交互启用语音输入。
setting-agent-mode-command-execution-allowlist = 智能体可以无需明确许可执行的命令。
setting-agent-mode-command-execution-denylist = 智能体执行前必须征求许可的命令。
setting-agent-mode-execute-readonly-commands = 控制智能体是否可以自动执行只读命令而无需询问。
setting-agent-mode-coding-permissions = 智能体的文件读取权限级别。
setting-agent-mode-coding-file-read-allowlist = 智能体可以无需许可读取的文件路径。
setting-aws-bedrock-credentials-enabled = 控制 Warp 是否应使用本地 AWS 凭证进行 Bedrock 请求。
setting-aws-bedrock-auto-login = 控制在 Bedrock 凭证过期时是否自动运行 AWS 登录命令。
setting-aws-bedrock-auth-refresh-command = 用于刷新 Bedrock AWS 凭证的命令。
setting-aws-bedrock-profile = 用于 Bedrock 凭证的 AWS 配置文件名称。
setting-memory-enabled = 控制智能体在请求期间是否使用你保存的规则。
setting-warp-drive-context-enabled = 控制 AI 请求中是否包含 Warp Drive 上下文。
setting-should-show-oz-updates = 控制是否在智能体视图中显示「新功能」部分。
setting-can-use-warp-credits-with-byok = 控制即使提供自己的 API 密钥时是否仍可使用 Warp 信用额度。
setting-should-render-use-agent-footer = 控制是否为终端命令显示「使用智能体」底栏。
setting-should-render-cli-agent-footer = 控制是否为编码智能体命令显示 CLI 智能体底栏。
setting-auto-toggle-rich-input = 控制 CLI 智能体富输入是否根据智能体的阻塞状态自动关闭和重新打开。
setting-auto-open-rich-input = 控制 CLI 智能体富输入是否在 CLI 智能体会话启动时自动打开。
setting-auto-dismiss-rich-input = 控制 CLI 智能体富输入是否在用户提交提示后自动关闭。
setting-cli-agent-footer-enabled-commands = 将自定义工具栏命令模式映射到特定 CLI 智能体。
setting-cloud-agent-computer-use-enabled = 控制是否为云智能体对话启用计算机使用。
setting-orchestration-enabled = 控制是否启用多智能体编排。
setting-file-based-mcp-enabled = 控制是否自动检测第三方基于文件的 MCP 服务器。
setting-thinking-display-mode = 控制智能体推理痕迹在流式传输后如何显示。
setting-default-session-mode = 新终端会话的默认模式。
setting-voice-input-toggle-key = 用于切换语音输入的按键。
setting-include-agent-commands-in-history = 控制智能体执行的命令是否包含在命令历史中。
setting-show-conversation-history = 控制对话历史是否显示在工具面板中。
setting-show-agent-notifications = 控制是否显示智能体通知。
setting-agent-attribution-enabled = 控制 Warp 智能体是否在创建的提交消息和 PR 中添加署名合著者行。

### Command palette
cmd-palette-compact-mode = 紧凑模式
cmd-palette-sync-with-os = 主题：跟随系统
cmd-palette-cursor-blink = 光标闪烁
cmd-palette-jump-to-bottom = 跳转到块按钮底部
cmd-palette-block-dividers = 块分隔线
cmd-palette-dim-inactive-panes = 调暗非活动窗格
cmd-palette-start-input-top = 在顶部开始输入
cmd-palette-pin-input-top = 将输入固定到顶部
cmd-palette-pin-input-bottom = 将输入固定到底部
cmd-palette-toggle-input-mode = 切换输入模式（Warp/经典）
cmd-palette-tab-indicators = 标签页指示器
cmd-palette-show-code-review-button = 在标签栏中显示代码审查按钮
cmd-palette-hide-code-review-button = 在标签栏中隐藏代码审查按钮
cmd-palette-focus-follows-mouse = 焦点跟随鼠标
cmd-palette-always-show-tab-bar = 始终显示标签栏
cmd-palette-hide-tab-bar-fullscreen = 全屏时隐藏标签栏
cmd-palette-show-tab-bar-on-hover = 悬停时显示标签栏
cmd-palette-zen-mode = 极简模式
cmd-palette-vertical-tab-layout = 垂直标签布局
cmd-palette-vertical-tabs-restored = 在恢复的窗口中显示垂直标签面板
cmd-palette-ligature-rendering = 连字渲染
cmd-palette-ai-toggle = AI
cmd-palette-active-ai-toggle = 主动 AI
cmd-palette-autodetection = 智能体输入中的终端命令自动检测 / 自然语言检测
cmd-palette-nld-terminal = 终端输入中的智能体提示自动检测
cmd-palette-next-command = 下一条命令
cmd-palette-prompt-suggestions = 提示建议
cmd-palette-code-suggestions = 代码建议
cmd-palette-show-agent-tips = 显示智能体提示
cmd-palette-hide-agent-tips = 隐藏智能体提示
cmd-palette-show-oz-changelog = 在新的智能体对话视图中显示 Oz 更新日志
cmd-palette-hide-oz-changelog = 在新的智能体对话视图中隐藏 Oz 更新日志
cmd-palette-natural-language-autosuggestions = 自然语言自动建议
cmd-palette-shared-block-title-generation = 共享块标题生成
cmd-palette-voice-input = 语音输入
cmd-palette-show-use-agent-footer = 显示「使用智能体」底栏
cmd-palette-hide-use-agent-footer = 隐藏「使用智能体」底栏
cmd-palette-codebase-index = 代码库索引
cmd-palette-thinking-show-collapse = 设置智能体推理显示：显示并折叠
cmd-palette-thinking-always-show = 设置智能体推理显示：始终显示
cmd-palette-thinking-never-show = 设置智能体推理显示：从不显示

### Locale
locale-label = 语言
locale-en-us = English
locale-zh-cn = 简体中文

### Language/locale setting description
setting-locale = The display language for the Warp UI.
