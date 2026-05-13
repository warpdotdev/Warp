# Warp 桌面端 — 简体中文
# 缺失的 key 会自动 fallback 到 en/warp.ftl，所以可以分批补译。
# 术语统一：Agent → 智能体 / Block → 命令块 / Drive → 云盘 / Workflow → 工作流 / Profile → 配置

# =============================================================================
# SECTION: common (Owner: foundation)
# =============================================================================

app-name = Warp
app-tagline = 面向个人与团队的云端终端

common-ok = 确定
common-cancel = 取消
common-apply = 应用
common-save = 保存
common-delete = 删除
common-confirm = 确认
common-close = 关闭
common-reset = 重置
common-back = 返回
common-next = 下一步
common-yes = 是
common-no = 否
common-continue = 继续
common-approve = 批准
common-deny = 拒绝
common-import = 导入
common-upgrade = 升级
common-default = 默认
common-editing = 正在编辑
common-viewing = 正在查看
common-restored = 已恢复
common-continued = 已继续
common-deleted = 已删除
common-send-feedback = 发送反馈
common-something-went-wrong = 出了点问题
common-no-results-found = 未找到结果。
common-edit = 编辑
common-add = 添加
common-remove = 移除
common-rename = 重命名
common-copy = 复制
common-paste = 粘贴
common-search = 搜索
common-view = 查看
common-loading = 加载中…
common-error = 错误
common-warning = 警告
common-info = 提示
common-success = 成功
common-all = 全部
common-none = 无
common-unknown = 未知
common-open = 打开
common-restore = 恢复
common-duplicate = 复制一份
common-export = 导出
common-trash = 移到废纸篓
common-copy-link = 复制链接
common-untitled = 未命名
common-retry = 重试
common-maximize = 最大化
common-discard = 放弃
common-undo = 撤销
common-commit = 提交
common-push = 推送
common-publish = 发布
common-create = 创建
common-configure = 配置
common-dismiss = 忽略
common-manage = 管理
common-failed = 失败
common-done = 已完成
common-working = 进行中
common-cut = 剪切
common-previous = 上一个
common-suggested = 推荐
common-copied-to-clipboard = 已复制到剪贴板
common-new = 新增
common-no-results = 无结果
common-learn-more = 了解更多
common-skip = 跳过
common-get-warping = 开始使用 Warp
common-try-again = 重试
common-settings = 设置
common-premium = 高级版
common-recommended = 推荐
common-enabled = 启用
common-disabled = 停用
common-free = 免费
common-subscribe = 订阅
common-list-prefix = {" - "}
common-current-directory = 当前目录

# =============================================================================
# SECTION: agent-management (Owner: agent-i18n-remaining)
# =============================================================================

agent-management-filter-all-tooltip = 查看你的智能体任务以及所有共享的团队任务
agent-management-filter-personal = 个人
agent-management-filter-personal-tooltip = 查看你创建的智能体任务
agent-management-get-started = 开始使用
agent-management-view-agents = 查看智能体
agent-management-clear-filters = 清除筛选
agent-management-clear-all = 全部清除
agent-management-new-agent = 新建智能体
agent-management-status = 状态
agent-management-source = 来源
agent-management-created-on = 创建时间
agent-management-has-artifact = 包含产物
agent-management-harness = 执行器
agent-management-environment = 环境
agent-management-created-by = 创建者
agent-management-last-24-hours = 最近 24 小时
agent-management-past-3-days = 最近 3 天
agent-management-last-week = 最近一周
agent-management-artifact-pull-request = Pull Request
agent-management-artifact-plan = 计划
agent-management-artifact-screenshot = 截图
agent-management-artifact-file = 文件
agent-management-source-scheduled = 定时
agent-management-source-local-agent = Warp（本地智能体）
agent-management-source-cloud-agent = Warp（本地智能体）
agent-management-source-oz-web = Oz Web
agent-management-source-github-action = GitHub Action
agent-management-no-session-available = 无可用会话
agent-management-session-expired = 会话已过期
agent-management-session-expired-tooltip = 会话会在一周后过期，且无法再打开。
agent-management-metadata-source = 来源：{ $source }
agent-management-metadata-harness = 执行器：{ $harness }
agent-management-metadata-run-time = 运行时长：{ $run_time }
agent-management-metadata-credits-used = 已用额度：{ $usage }
agent-management-environment-selected = 环境：{ $environment }
agent-management-loading-cloud-runs = 正在加载智能体运行

# =============================================================================
# SECTION: workspace-runtime (Owner: agent-i18n-remaining)
# =============================================================================

workspace-menu-update-warp-manually = 手动更新 Warp
workspace-menu-whats-new = 最新变化
workspace-menu-settings = 设置
workspace-menu-keyboard-shortcuts = 快捷键
workspace-menu-documentation = 文档
workspace-menu-feedback = 反馈
workspace-menu-view-warp-logs = 查看 Warp 日志
workspace-menu-slack = Slack
workspace-toast-failed-load-conversation = 加载对话失败。
workspace-toast-failed-load-conversation-for-forking = 加载要 fork 的对话失败。
workspace-toast-conversation-forking-failed = 对话 fork 失败。
workspace-toast-no-terminal-pane-for-context = 没有打开的终端窗格。请打开新窗格后再附加为上下文。
workspace-toast-plan-already-in-context = 此计划已在上下文中。
workspace-toast-command-still-running = 此会话中仍有命令正在运行。
workspace-toast-cannot-open-terminal-session = 无法打开新的终端会话
workspace-toast-out-of-ai-credits = 你的 AI 额度似乎已用完。
workspace-toast-upgrade-more-credits = 升级以获取更多额度。
workspace-toast-disabled-synchronized-inputs = 已停用所有同步输入。
workspace-toast-conversation-deleted = 对话已删除
workspace-search-repos-placeholder = 搜索仓库
workspace-search-tabs-placeholder = 搜索标签页...
workspace-rearrange-toolbar-items = 重新排列工具栏项目
workspace-new-session-agent = 智能体
workspace-new-session-terminal = 终端
workspace-new-session-cloud-oz = 云端 Oz
workspace-new-session-local-docker-sandbox = 本地 Docker 沙箱
workspace-new-worktree-config = 新建 worktree 配置
workspace-new-tab-config = 新建标签页配置
workspace-reopen-closed-session = 重新打开已关闭的会话
app-menu-new-window = 新建窗口
app-menu-save-new = 另存为新配置...
app-menu-launch-configurations = 启动配置
app-menu-warp = Warp
app-menu-preferences = 偏好设置
app-menu-privacy-policy = 隐私政策...
app-menu-debug = 调试
app-menu-set-default-terminal = 将 Warp 设为默认终端
app-menu-file = 文件
app-menu-edit = 编辑
app-menu-use-warp-prompt = 使用 Warp 提示符
app-menu-copy-on-select-terminal = 在终端中选中即复制
app-menu-synchronize-inputs = 同步输入
app-menu-view = 视图
app-menu-toggle-mouse-reporting = 切换鼠标上报
app-menu-toggle-scroll-reporting = 切换滚动上报
app-menu-toggle-focus-reporting = 切换焦点上报
app-menu-compact-mode = 紧凑模式
app-menu-tab = 标签页
app-menu-ai = AI
app-menu-blocks = 命令块
app-menu-drive = Drive
app-menu-show-in-band-command-blocks = 显示内嵌命令块
app-menu-hide-in-band-command-blocks = 隐藏内嵌命令块
app-menu-show-warpified-ssh-blocks = 显示 Warp 化 SSH 块
app-menu-hide-warpified-ssh-blocks = 隐藏 Warp 化 SSH 块
app-menu-show-initialization-block = 显示初始化命令块
app-menu-hide-initialization-block = 隐藏初始化命令块
app-menu-window = 窗口
app-menu-enable-shell-debug-mode = 为新会话启用 Shell 调试模式 (-x)
app-menu-disable-shell-debug-mode = 为新会话停用 Shell 调试模式 (-x)
app-menu-enable-pty-recording = 启用 PTY 录制模式 (warp.pty.recording)
app-menu-disable-pty-recording = 停用 PTY 录制模式 (warp.pty.recording)
app-menu-enable-in-band-generators = 为新会话启用内嵌生成器
app-menu-disable-in-band-generators = 为新会话停用内嵌生成器
app-menu-manually-toggle-network-status = 手动切换网络状态
app-menu-export-default-settings-csv = 将默认设置导出为 CSV 到主目录
app-menu-create-anonymous-user = 创建匿名用户
app-menu-send-feedback = 发送反馈...
app-menu-help = 帮助
app-menu-warp-documentation = Warp 文档...
app-menu-github-issues = GitHub Issues...
app-menu-warp-slack-community = Warp Slack 社区...
workspace-update-and-relaunch-warp = 更新并重启 Warp
workspace-updating-to-version = 正在更新到（{ $version }）
workspace-update-warp-manually = 手动更新 Warp
workspace-view-all-cloud-runs = 查看所有云端运行
pane-get-started-title = 开始使用
pane-new-tab-title = 新建标签页

# =============================================================================
# SECTION: terminal-runtime (Owner: agent-i18n-remaining)
# =============================================================================

terminal-banner-completions-not-working-prefix = 你的补全看起来没有正常工作（
terminal-banner-more-info-lower = 更多信息
terminal-banner-more-info = 更多信息
terminal-banner-completions-not-working-middle = ）。在{" "}
terminal-banner-settings = 设置
terminal-banner-completions-not-working-suffix =  中启用 tmux warpification 可能会解决此问题。
terminal-banner-shell-config-incompatible = 你的 shell 配置与 Warp 不兼容...{"  "}
terminal-banner-did-you-intend = 你是否想用{" "}
terminal-banner-move-cursor =  移动光标？
terminal-toast-powershell-subshells-not-supported = 不支持 PowerShell 子 shell
terminal-dont-ask-again = 不再询问
terminal-clear-upload = 清除上传
terminal-manage-defaults = 管理默认值
terminal-free-credits = 免费额度
terminal-cloud-agent-run = 智能体运行
terminal-agent-header-for-terminal = 用于终端
ssh-remote-choice-title = 为此远程会话选择体验：
ssh-remote-choice-install-extension = 安装 Warp 的 SSH 扩展
ssh-remote-choice-install-extension-desc = 安装 Warp 扩展以在此会话中启用文件浏览、代码评审和智能命令补全等智能体功能。
ssh-remote-choice-continue-without-installing = 不安装并继续
ssh-remote-choice-continue-without-installing-desc = 你仍可获得 Warp 化体验，但不包含智能体功能。
ssh-remote-choice-manage-warpify-settings = 管理 Warpify 设置
ai-document-show-version-history = 显示版本历史
ai-document-update-agent = 更新智能体
ai-document-save-and-sync-tooltip = 保存并将此计划自动同步到你的 Warp Drive
ai-document-show-in-warp-drive = 在 Warp Drive 中查看
ai-document-save-as-markdown-file = 另存为 markdown 文件
ai-document-attach-to-active-session = 附加到当前会话
ai-document-copy-plan-id = 复制计划 ID
ai-document-plan-id-copied = 计划 ID 已复制到剪贴板
ai-conversation-view-in-oz = 在 Oz 中查看
ai-conversation-view-in-oz-tooltip = 在 Oz Web 应用中查看此运行
ai-artifact-prepare-download-failed = 准备文件下载失败。
ai-block-open-in-github = 在 GitHub 中打开
ai-block-open-in-code-review = 在代码评审中打开
ai-block-manage-rules = 管理规则
ai-block-review-changes = 查看更改
ai-block-open-all-in-code-review = 在代码评审中全部打开
ai-block-dont-show-again = 不再显示
ai-block-rewind = 回退
ai-block-rewind-tooltip = 回退到此命令块之前
ai-block-remove-queued-prompt = 移除排队提示
ai-block-send-now = 立即发送
ai-block-check-now =  · 立即检查
ai-block-check-now-tooltip = 让智能体现在检查此命令，跳过计时器。
ai-block-resume-conversation = 继续对话
ai-block-continue-conversation = 继续对话
ai-block-fork-conversation = Fork 对话
ai-block-show-credit-usage-details = 显示额度使用详情
ai-block-follow-up-existing-conversation = 跟进已有对话
ai-block-accept = 接受
ai-block-auto-approve = 自动批准
ai-rule-add-rule = 添加规则
ai-rule-edit-rule = 编辑规则
ai-rule-delete-rule = 删除规则
ai-aws-refresh-credentials = 刷新 AWS 凭据
ai-footer-enable-notifications = 启用通知
ai-footer-enable-notifications-tooltip = 安装 Warp 插件以在 Warp 中启用丰富的智能体通知
ai-footer-notifications-setup-instructions = 通知设置说明
ai-footer-install-plugin-instructions-tooltip = 查看安装 Warp 插件的说明
ai-footer-update-warp-plugin = 更新 Warp 插件
ai-footer-plugin-update-available-tooltip = Warp 插件有新版本可用
ai-footer-plugin-update-instructions = 插件更新说明
ai-footer-plugin-update-instructions-tooltip = 查看更新 Warp 插件的说明
ai-footer-context-window-usage-tooltip = 上下文窗口使用情况
ai-footer-choose-environment-tooltip = 选择环境
ai-footer-reasoning-depth-tooltip = 推理深度
ai-footer-file-explorer = 文件浏览器
ai-footer-open-file-explorer = 打开文件浏览器
ai-footer-rich-input = 富输入
ai-footer-open-rich-input = 打开富输入
ai-footer-open-coding-agent-settings = 打开编码智能体设置
ai-ask-user-question-placeholder = 输入答案后按 Enter
ai-ask-user-questions-skipped = 已跳过问题
ai-ask-user-answered-question = 已回答问题
ai-ask-user-answered-all-questions = 已回答全部 { $total } 个问题
ai-ask-user-answered-count = 已回答 { $answered_count } / { $total } 个问题
ai-code-diff-requested-edit-title = 请求的编辑
ai-cloud-setup-visit-oz = 访问 Oz
ai-inline-code-diff-review-changes = 查看更改
ai-execution-profile-name-placeholder = 例如 "YOLO code"
ai-execution-profile-delete-profile = 删除配置
ai-notifications-mark-all-as-read = 全部标为已读
ai-assistant-copy-transcript-tooltip = 复制转录内容到剪贴板
code-comment = 评论
code-copy-file-path = 复制文件路径
code-select-all = 全选
code-replace-all = 全部替换
code-goto-line-placeholder = 行号:列号
code-open-file-unavailable-remote-tooltip = 远程会话无法打开文件
code-view-markdown-preview = 查看 Markdown 预览
markdown-display-mode-rendered = 预览
markdown-display-mode-raw = 源码
code-review-commit-and-create-pr = 提交并创建 PR
notebook-link-text-placeholder = 文本
notebook-link-url-placeholder = 链接（网页或文件）
notebook-block-embed = 嵌入
notebook-block-divider = 分割线
notebook-insert-block-tooltip = 插入块
notebook-refresh-notebook = 刷新笔记
notebook-refresh-file = 刷新文件
notebook-open-in-editor = 在编辑器中打开
notebook-sign-in-to-edit = 登录后编辑
editor-custom-keybinding = 自定义...
editor-change-keybinding = 更改快捷键
autosuggestion-ignore-this-suggestion = 忽略此建议
codex-use-latest-model = 使用最新 Codex 模型
openwarp-launch-visit-repo = 访问仓库
openwarp-launch-title = Warp 现已开源
openwarp-launch-description = 你和我们的社区现在可以使用智能体优先的工作流参与构建 Warp。
openwarp-launch-contribute-title = 参与贡献
openwarp-launch-contribute-description = Warp 客户端代码现已开源。你可以先使用 /feedback skill 创建 issue，并阅读这里的贡献指南。
openwarp-launch-contribute-link-text = 这里
openwarp-launch-oad-title = 开放自动化开发
openwarp-launch-oad-description = Warp 仓库由智能体优先的本地工作流管理，并由 Oz 本地智能体体验提供支持。
openwarp-launch-auto-model-title = 介绍「auto（开放权重）」
openwarp-launch-auto-model-description = 我们新增了一个 auto 模型，可为任务选择最佳开放权重模型，例如 Kimi 或 MiniMax。
hoa-see-whats-new = 查看新变化
hoa-finish = 完成
session-config-get-warping = 开始使用 Warp
uri-custom-uri-invalid = 自定义 URI 无效。
context-node-install-nvm = 安装 nvm
context-node-install-node = nvm install node
context-node-installed = 已安装
context-chip-change-git-branch = 切换 Git 分支
context-chip-view-pull-request = 查看 Pull Request
context-chip-change-working-directory = 更改工作目录
context-chip-working-directory = 工作目录
settings-ai-repo-placeholder = 例如 ~/code-repos/repo
settings-ai-commands-comma-separated-placeholder = 命令，英文逗号分隔
settings-ai-regex-example-placeholder = 例如 ls .*
settings-ai-command-supports-regex-placeholder = 命令（支持正则）
settings-ai-aws-login-placeholder = aws login
settings-ai-default-placeholder = default
settings-working-directory-path-placeholder = 路径
settings-startup-shell-executable-path-placeholder = 可执行文件路径
settings-agent-providers-base-url-placeholder = https://api.deepseek.com/v1
drive-sharing-only-people-invited = 仅受邀者
drive-sharing-anyone-with-link = 拥有链接的任何人
drive-sharing-only-invited-teammates = 仅受邀团队成员
drive-sharing-teammates-with-link = 拥有链接的团队成员
terminal-warpify-subshell = Warpify subshell
terminal-warpify-subshell-tooltip = 在此会话中启用 Warp shell 集成
terminal-use-agent = 使用智能体
terminal-use-agent-tooltip = 让 Warp 智能体协助
terminal-give-control-back-to-agent = 将控制权交还给智能体
terminal-resume-agent-tooltip = 让 Warp 智能体继续
terminal-voice-input-tooltip = 语音输入
terminal-attach-file-tooltip = 附加文件
terminal-slash-commands-tooltip = 斜杠命令
terminal-manage-api-keys-tooltip = 管理 API 密钥
terminal-profiles = 配置
terminal-manage-profiles = 管理配置
terminal-continue-locally = 在本地继续
terminal-fork-conversation-locally-tooltip = 将此对话 fork 到本地
terminal-open-in-warp = 在 Warp 中打开
terminal-open-conversation-in-warp-tooltip = 在 Warp 桌面端打开此对话
terminal-share-session = 分享会话...
terminal-stop-sharing = 停止分享
terminal-copy-session-sharing-link = 复制会话分享链接
terminal-shared-session-make-editor = 设为编辑者
terminal-shared-session-make-viewer = 设为查看者
terminal-shared-session-change-role = 更改角色
terminal-choose-execution-profile-tooltip = 选择 AI 执行配置
terminal-choose-agent-model-tooltip = 选择智能体模型
terminal-input-cli-agent-rich-input-hint = 告诉智能体要构建什么...
terminal-input-enter-prompt-for-agent = 输入给 { $agent } 的提示...
terminal-input-cloud-agent-hint = 启动智能体
terminal-input-a11y-label = 命令输入。
terminal-input-a11y-helper = 输入 shell 命令，按 Enter 执行。按 cmd-up 导航到之前执行命令的输出。按 cmd-l 重新聚焦命令输入框。
terminal-input-ai-command-search-hint = 输入 '#' 获取 AI 命令建议
terminal-input-run-commands-hint = 运行命令
terminal-input-agent-hint-deploy-react-vercel = 让 Warp 处理任何任务，例如将 React 应用部署到 Vercel 并配置环境变量
terminal-input-agent-hint-debug-python-ci = 让 Warp 处理任何任务，例如帮我调试 Python 测试在 CI 中失败的原因
terminal-input-agent-hint-setup-microservice = 让 Warp 处理任何任务，例如用 Docker 搭建新的微服务并创建部署流水线
terminal-input-agent-hint-fix-node-memory-leak = 让 Warp 处理任何任务，例如查找并修复 Node.js 应用中的内存泄漏
terminal-input-agent-hint-backup-postgres = 让 Warp 处理任何任务，例如为 PostgreSQL 数据库创建备份脚本并设置定时任务
terminal-input-agent-hint-migrate-mysql-postgres = 让 Warp 处理任何任务，例如帮我把数据从 MySQL 迁移到 PostgreSQL
terminal-input-agent-hint-monitor-aws = 让 Warp 处理任何任务，例如为 AWS 基础设施设置监控和告警
terminal-input-agent-hint-build-fastapi = 让 Warp 处理任何任务，例如用 FastAPI 为移动应用构建 REST API
terminal-input-agent-hint-optimize-sql = 让 Warp 处理任何任务，例如帮我优化运行缓慢的 SQL 查询
terminal-input-agent-hint-github-actions = 让 Warp 处理任何任务，例如创建 GitHub Actions 工作流，在合并后自动部署
terminal-input-agent-hint-redis-cache = 让 Warp 处理任何任务，例如为 Web 应用设置 Redis 缓存
terminal-input-agent-hint-kubernetes-pods = 让 Warp 处理任何任务，例如帮我排查 Kubernetes Pod 持续崩溃的原因
terminal-input-agent-hint-bigquery-pipeline = 让 Warp 处理任何任务，例如构建数据流水线来处理 CSV 文件并加载到 BigQuery
terminal-input-agent-hint-ssl-https = 让 Warp 处理任何任务，例如设置 SSL 证书并为域名配置 HTTPS
terminal-input-agent-hint-refactor-legacy-code = 让 Warp 处理任何任务，例如帮我将这段遗留代码重构为现代设计模式
terminal-input-agent-hint-unit-tests = 让 Warp 处理任何任务，例如为认证服务创建单元测试
terminal-input-agent-hint-elk-logs = 让 Warp 处理任何任务，例如为分布式系统设置 ELK 日志聚合
terminal-input-agent-hint-oauth-express = 让 Warp 处理任何任务，例如帮我在 Express.js 应用中实现 OAuth2 认证
terminal-input-agent-hint-optimize-docker = 让 Warp 处理任何任务，例如优化 Docker 镜像以减少构建时间和体积
terminal-input-agent-hint-ab-testing = 让 Warp 处理任何任务，例如为 Web 应用搭建 A/B 测试基础设施
terminal-input-steer-agent-hint = 指导正在运行的智能体
terminal-input-steer-agent-backspace-hint = 指导正在运行的智能体，或按 Backspace 退出
terminal-input-follow-up-hint = 继续追问
terminal-input-follow-up-backspace-hint = 继续追问，或按 Backspace 退出
terminal-input-search-queries = 搜索查询
terminal-input-search-queries-rewind = 搜索要回退到的查询
terminal-input-search-conversations = 搜索对话
terminal-input-search-skills = 搜索技能
terminal-input-search-models = 搜索模型
terminal-input-search-profiles = 搜索配置
terminal-input-search-commands = 搜索命令
terminal-input-search-prompts = 搜索提示
terminal-input-search-indexed-repos = 搜索已索引仓库
terminal-input-search-plans = 搜索计划
terminal-input-choose-agent-model = 选择智能体模型
terminal-message-new-agent-conversation = {" "}新建 /agent 对话
terminal-message-agent-for-new-conversation = /agent 用于新建对话
terminal-message-selected-text-attached = 已将所选文本附加为上下文
terminal-message-to-remove = {" "}以移除
terminal-message-to-dismiss = {" "}以关闭
terminal-message-plan-with-agent = {" "}让智能体规划
terminal-message-continue-conversation = {" "}继续对话
terminal-message-to-execute = {" "}执行
terminal-message-to-send = {" "}发送
terminal-message-open-conversation-title = {" "}打开「{ $title }」
terminal-message-autodetected = {" "}（已自动识别）{" "}
terminal-message-to-override = {" "}覆盖
terminal-message-to-navigate = {" "}导航
terminal-message-to-cycle-tabs = {" "}切换标签页
terminal-message-to-select = {" "}选择
terminal-message-select-save-profile = {" "}选择并保存到配置
terminal-message-open-plan = {" "}打开计划
terminal-starting-shell = 正在启动 shell...
terminal-input-no-skills-found = 未找到技能
terminal-model-specs-title = 模型规格
terminal-model-specs-description = Warp 对模型在执行器中的表现、额度消耗速率和任务速度的基准测试。
terminal-model-specs-reasoning-level-title = 推理级别
terminal-model-specs-reasoning-level-description = 更高推理级别会消耗更多额度并带来更高延迟，但在复杂任务上表现更好。
terminal-model-auto-mode-title = 自动模式
terminal-model-auto-mode-description = 自动模式会为任务选择最佳模型。Cost-efficiency 会优化成本，Responsiveness 会优化响应速度。
terminal-model-banner-base-agent = 你正在使用基础智能体。完整终端使用模型仅适用于完整终端使用智能体。
terminal-model-banner-full-terminal-agent = 你正在使用完整终端使用智能体。基础模型仅适用于基础智能体。
terminal-filter-block-output-placeholder = 筛选命令块输出

# =============================================================================
# SECTION: object-surfaces (Owner: agent-i18n-remaining)
# =============================================================================

code-review-tooltip-show-file-navigation = 显示文件导航
code-review-discard-changes = 放弃更改
code-review-create-pr = 创建 PR
code-review-add-diff-set-context = 将 diff 集添加为上下文
code-review-show-saved-comment = 显示已保存评论
code-review-add-comment = 添加评论
code-review-discard-all = 放弃全部
code-review-initialize-codebase = 初始化代码库
code-review-initialize-codebase-tooltip = 启用代码库索引和 WARP.md
code-review-open-repository = 打开仓库
code-review-open-repository-tooltip = 前往仓库并初始化用于编码
code-review-open-file = 打开文件
code-review-add-file-diff-context = 将文件 diff 添加为上下文
code-review-copy-file-path = 复制文件路径
code-review-no-open-changes = 没有打开的变更
code-review-header-reviewing-changes = 正在查看代码变更
code-review-search-diff-placeholder = 搜索要比较的 diff 集或分支…
code-review-one-comment = 1 条评论
code-review-copy-text = 复制文本
code-review-file-level-comment-cannot-edit = 暂不支持编辑文件级评论。
code-review-outdated-comment-cannot-edit = 无法编辑已过期评论。
code-review-view-in-github = 在 GitHub 中查看
notebook-menu-attach-active-session = 附加到当前会话
object-menu-open-on-desktop = 在桌面端打开
notebook-tooltip-restore-from-trash = 从废纸篓恢复笔记
notebook-tooltip-copy-to-personal = 将笔记内容复制到你的个人工作区
notebook-copy-to-personal = 复制到个人空间
notebook-tooltip-copy-to-clipboard = 将笔记内容复制到剪贴板
notebook-copy-all = 全部复制
object-toast-link-copied = 链接已复制到剪贴板
drive-toast-finished-exporting = 对象导出完成

# =============================================================================
# SECTION: remaining-settings-tabs-env (Owner: agent-i18n-remaining)
# =============================================================================

settings-environment-delete-button = 删除环境
settings-language-system-default = 系统默认
settings-language-english = 英文
tab-config-open-tab = 打开标签页
tab-config-make-default = 设为默认
tab-config-already-default = 已是默认
tab-config-edit-config = 编辑配置
env-vars-restore-tooltip = 从废纸篓恢复环境变量
env-vars-variables-label = 变量

# =============================================================================
# SECTION: onboarding-callout (Owner: agent-i18n-remaining)
# =============================================================================

onboarding-callout-meet-input-title = 认识 Warp 输入框
onboarding-callout-meet-input-text-prefix = 你的终端输入框同时支持终端命令和智能体提示，并会自动识别你的输入类型。使用
onboarding-callout-meet-input-text-suffix = 可将输入锁定为智能体模式（自然语言）或终端模式（命令）。
onboarding-callout-talk-agent-title = 与智能体对话
onboarding-callout-talk-agent-text = 你可以直接输入自然语言来使用智能体。提交下方查询即可开始：这个仓库里有哪些测试？它们是如何组织的？覆盖了哪些内容？
onboarding-callout-skip = 跳过
onboarding-callout-submit = 提交
onboarding-callout-finish = 完成
onboarding-callout-meet-terminal-title = 认识你的终端输入框
onboarding-callout-meet-updated-terminal-title = 认识升级后的终端输入框
onboarding-callout-meet-terminal-text-prefix = 你可以在终端中运行命令，也可以使用
onboarding-callout-meet-terminal-text-suffix = 来启动智能体或发送给智能体。
onboarding-callout-nl-overrides-title = 自然语言覆盖
onboarding-callout-nl-overrides-text-prefix = 你始终可以使用
onboarding-callout-nl-support-title = 自然语言支持
onboarding-callout-nl-support-text-prefix = 默认情况下自然语言输入处于关闭状态。启用后，你可以直接用自然语言输入请求，Warp 会为智能体自动识别查询。你也始终可以使用
onboarding-callout-enable-nl-detection = 启用自然语言识别
onboarding-callout-new-agent-title = 认识 Warp 全新的智能体体验
onboarding-callout-new-agent-text = 智能体对话现在会作为终端之外的独立视图存在。你可以随时按 ESC 返回终端。
onboarding-callout-updated-agent-input-title = 升级后的智能体输入框
onboarding-callout-updated-agent-input-project-text = 默认情况下，你的智能体输入框会同时识别自然语言和命令。使用 ! 可将输入锁定为 bash 模式以编写命令。\n\n提交下方查询即可让智能体初始化此项目，或按 ⊗ 清空输入后自行开始！
onboarding-callout-skip-initialization = 跳过初始化
onboarding-callout-initialize = 初始化
onboarding-callout-updated-agent-input-text = 默认情况下，你的智能体输入框会同时识别自然语言和命令。使用 ! 可将输入锁定为 bash 模式以编写命令。
onboarding-callout-back-terminal = 返回终端

# =============================================================================
# SECTION: language (Owner: foundation)
# =============================================================================

language-widget-label = 语言
language-widget-secondary = 重启 Warp 以让此更改完全生效。
language-restart-required-title = 语言已切换
language-restart-required-body = Warp 的界面语言已更新。部分文字会立即切换，完整生效需要重启 Warp。

# =============================================================================
# SECTION: settings (Owner: agent-settings)
# =============================================================================

# --- ANCHOR-SUB-MOD-NAV (agent-settings-mod) ---
# settings_view/mod.rs SettingsSection 标签 + 上下文菜单分屏/关闭操作

# 侧边栏 / SettingsSection 标签（Display impl）
settings-section-about = 关于
settings-section-account = 账户
settings-section-mcp-servers = MCP 服务器
settings-section-billing-and-usage = 账单与用量
settings-section-appearance = 外观
settings-section-features = 功能
settings-section-keybindings = 快捷键
settings-section-privacy = 隐私
settings-section-referrals = 推荐
settings-section-shared-blocks = 共享命令块
settings-section-teams = 团队
settings-section-warp-drive = Warp Drive
settings-section-warpify = Warpify
settings-section-ai = AI
settings-section-warp-agent = Warp 智能体
settings-section-agent-profiles = 配置
settings-section-agent-mcp-servers = MCP 服务器
settings-section-agent-providers = 提供商
settings-section-knowledge = Rules
settings-section-third-party-cli-agents = 第三方 CLI 智能体
settings-section-code = 代码
settings-section-editor-and-code-review = 编辑器与代码评审
settings-section-cloud-environments = 环境
settings-section-oz-cloud-api-keys = Oz Cloud API 密钥
settings-title = 设置

# 上下文菜单项（分屏 / 关闭窗格）
settings-pane-split-right = 向右拆分窗格
settings-pane-split-left = 向左拆分窗格
settings-pane-split-down = 向下拆分窗格
settings-pane-split-up = 向上拆分窗格
settings-pane-close = 关闭窗格

# 调试开关设置描述（命令面板）
settings-debug-show-init-block = 显示初始化命令块
settings-debug-hide-init-block = 隐藏初始化命令块
settings-debug-show-inband-blocks = 显示行内命令块
settings-debug-hide-inband-blocks = 隐藏行内命令块

# --- ANCHOR-SUB-ABOUT (agent-settings-about) ---

# about_page.rs
settings-about-copyright = 版权所有 2026 Warp
settings-about-automatic-updates-label = 自动检查更新
settings-about-automatic-updates-description = 开启后，OpenWarp 会在后台定期检查是否有新版本;发现新版本后会在上方提示并提供 GitHub 下载链接。OpenWarp 不会自动下载或安装更新。
settings-about-update-checking = 正在检查更新…
settings-about-update-up-to-date = 已是最新版本。
settings-about-update-available = 发现新版本 { $version }。
settings-about-update-check-now = 检查更新
settings-about-update-open-release = 前往 GitHub 下载

# main_page.rs — referral / account
settings-main-referral-cta = 与朋友和同事分享 Warp，获得奖励
settings-main-refer-a-friend = 推荐朋友
settings-main-sign-up = 注册
settings-main-plan-free = 免费版
settings-main-compare-plans = 对比方案
settings-main-contact-support = 联系支持
settings-main-manage-billing = 管理账单
settings-main-upgrade-to-turbo = 升级到 Turbo 方案
settings-main-upgrade-to-lightspeed = 升级到 Lightspeed 方案

# main_page.rs — settings sync
settings-main-settings-sync-label = 设置同步

# main_page.rs — version / autoupdate
settings-main-version-label = 版本
settings-main-status-up-to-date = 已是最新
settings-main-cta-check-for-updates = 检查更新
settings-main-status-checking = 正在检查更新...
settings-main-status-downloading = 正在下载更新...
settings-main-status-update-available = 有可用更新
settings-main-cta-relaunch-warp = 重启 Warp
settings-main-status-updating = 正在更新...
settings-main-status-installed-update = 更新已安装
settings-main-status-cant-install = 有新版本的 Warp 可用，但无法安装
settings-main-status-cant-launch = 新版本的 Warp 已安装，但无法启动。
settings-main-cta-update-manually = 手动更新 Warp

# --- ANCHOR-SUB-MCP (agent-settings-mcp) ---
settings-mcp-page-title = MCP 服务器
settings-mcp-logout-success-named = 已成功登出 {$name} MCP 服务器
settings-mcp-logout-success = 已成功登出 MCP 服务器
settings-mcp-install-modal-busy = 请先完成当前 MCP 安装，然后再打开另一个安装链接。
settings-mcp-unknown-server = 未知的 MCP 服务器 '{$name}'
settings-mcp-install-from-link-failed = MCP 服务器 '{$name}' 无法通过此链接安装。

# ---- destructive_mcp_confirmation_dialog.rs ----
settings-mcp-confirm-delete-local-title = 删除 MCP 服务器？
settings-mcp-confirm-delete-local-description = 这将从你所有的设备上卸载并移除此 MCP 服务器。
settings-mcp-confirm-delete-shared-title = 删除共享的 MCP 服务器？
settings-mcp-confirm-delete-shared-description = 此操作不仅会为你自己删除此 MCP 服务器，还会从 Warp 以及所有团队成员的设备上卸载并移除此 MCP 服务器。
settings-mcp-confirm-unshare-title = 从团队中移除共享的 MCP 服务器？
settings-mcp-confirm-unshare-description = 这将从 Warp 以及所有团队成员的设备上卸载并移除此 MCP 服务器。
settings-mcp-confirm-delete-button = 删除 MCP
settings-mcp-confirm-remove-from-team-button = 从团队移除
settings-mcp-confirm-cancel-button = 取消

# ---- edit_page.rs ----
settings-mcp-edit-save = 保存
settings-mcp-edit-edit-variables = 编辑变量
settings-mcp-edit-delete = 删除 MCP
settings-mcp-edit-remove-from-team = 从团队移除
settings-mcp-edit-editing-disabled-banner = 仅团队管理员和 MCP 服务器的创建者可以编辑此 MCP 服务器。
settings-mcp-edit-add-new-title = 添加新 MCP 服务器
settings-mcp-edit-edit-named-title = 编辑 { $name } MCP 服务器
settings-mcp-edit-edit-title = 编辑 MCP 服务器
settings-mcp-edit-logout-tooltip = 登出
settings-mcp-edit-secrets-error = 此 MCP 服务器包含敏感信息。请前往 设置 > 隐私 修改你的敏感信息脱敏设置。
settings-mcp-edit-no-server-error = 未指定 MCP 服务器。
settings-mcp-edit-multiple-servers-error = 编辑单个服务器时无法添加多个 MCP 服务器。

# ---- installation_modal.rs ----
settings-mcp-install-modal-title = 安装 { $name }
settings-mcp-install-modal-source-shared = 团队共享
settings-mcp-install-modal-source-other-device = 来自其他设备
settings-mcp-install-modal-cancel = 取消
settings-mcp-install-modal-install = 安装
settings-mcp-install-modal-no-server = 未选择 MCP 服务器

# ---- list_page.rs ----
settings-mcp-list-description = 添加 MCP 服务器以扩展 Warp Agent 的能力。MCP 服务器通过标准化接口向 agent 暴露数据源或工具，本质上类似插件。你可以添加自定义服务器，或使用预设快速开始使用流行的服务器。你也可以在此找到团队共享给你的服务器。
settings-mcp-list-learn-more = 了解更多。
settings-mcp-list-empty-state = 添加 MCP 服务器后，它将显示在此处。
settings-mcp-list-no-search-results = 未找到搜索结果
settings-mcp-list-search-placeholder = 搜索 MCP 服务器
settings-mcp-list-add-button = 添加
settings-mcp-list-file-based-toggle-label = 自动启动来自第三方 agent 的服务器
settings-mcp-list-file-based-description = 自动检测并启动来自全局范围的第三方 AI agent 配置文件（例如位于你的主目录）中的 MCP 服务器。在仓库内部检测到的服务器永远不会自动启动，必须在下方「检测自」分组中单独启用。
settings-mcp-list-file-based-supported-providers = 查看支持的 provider。
settings-mcp-list-template-available-to-install = 可安装
settings-mcp-list-file-based-detected = 来自配置文件的检测
settings-mcp-list-toast-server-updated = MCP 服务器已更新
settings-mcp-list-section-my-mcps = 我的 MCP
settings-mcp-list-section-shared-by-warp-and-team = 由 Warp 和 { $name } 共享
settings-mcp-list-section-shared-by-warp-and-other-devices = 由 Warp 和其他设备共享
settings-mcp-list-section-shared-from-warp = 来自 Warp 的共享
settings-mcp-list-section-detected-from = 检测自 { $provider }
settings-mcp-list-chip-global = 全局
settings-mcp-list-chip-shared-by-creator = 由 { $creator } 共享
settings-mcp-list-chip-shared-by-team-member = 由团队成员共享
settings-mcp-list-chip-from-another-device = 来自其他设备

# ---- server_card.rs ----
settings-mcp-card-tooltip-show-logs = 查看日志
settings-mcp-card-tooltip-log-out = 登出
settings-mcp-card-tooltip-share-server = 共享服务器
settings-mcp-card-tooltip-edit = 编辑
settings-mcp-card-tooltip-update-available = 有可用的服务器更新
settings-mcp-card-button-view-logs = 查看日志
settings-mcp-card-button-edit-config = 编辑配置
settings-mcp-card-button-set-up = 设置
settings-mcp-card-tools-none = 暂无可用工具
settings-mcp-card-tools-available = { $count } 个可用工具
settings-mcp-card-status-offline = 离线
settings-mcp-card-status-starting = 正在启动服务器…
settings-mcp-card-status-authenticating = 正在认证…
settings-mcp-card-status-shutting-down = 正在关闭…

# ---- update_modal.rs ----
settings-mcp-update-modal-default-name = 服务器
settings-mcp-update-modal-title = 更新 { $name }
settings-mcp-update-modal-description = 此服务器有 { $count } 个可用更新，你想使用哪一个？
settings-mcp-update-modal-publisher-another-device = 其他设备
settings-mcp-update-modal-publisher-team-member = 团队成员
settings-mcp-update-modal-update-from = 来自 { $publisher } 的更新
settings-mcp-update-modal-version = 版本 { $version }
settings-mcp-update-modal-cancel = 取消
settings-mcp-update-modal-update = 更新
settings-mcp-update-modal-no-updates = 暂无可用更新

# --- ANCHOR-SUB-PLATFORM (agent-settings-platform) ---
settings-platform-section-title = Oz Cloud API 密钥
settings-platform-description = 创建并管理 API 密钥，允许其他 Oz 云端 agent 访问你的 Warp 账户。
    了解更多请访问
settings-platform-documentation-link = 文档。
settings-platform-create-button = + 创建 API 密钥
settings-platform-modal-title-new = 新建 API 密钥
settings-platform-modal-title-save = 保存你的密钥
settings-platform-toast-deleted = API 密钥已删除
settings-platform-column-name = 名称
settings-platform-column-key = 密钥
settings-platform-column-scope = 范围
settings-platform-column-created = 创建时间
settings-platform-column-last-used = 最近使用
settings-platform-column-expires-at = 过期时间
settings-platform-value-never = 从未
settings-platform-scope-personal = 个人
settings-platform-scope-team = 团队
settings-platform-zero-state-title = 暂无 API 密钥
settings-platform-zero-state-description = 创建密钥以管理对 Warp 的外部访问
settings-platform-create-api-key-description-personal = 此 API 密钥与你的用户绑定，可用于向你的 Warp 账户发起请求。
settings-platform-create-api-key-description-team = 此 API 密钥与你的团队绑定，可用于代表你的团队发起请求。
settings-platform-create-api-key-name-placeholder = Warp API 密钥
settings-platform-create-api-key-expiration-one-day = 1 天
settings-platform-create-api-key-expiration-thirty-days = 30 天
settings-platform-create-api-key-expiration-ninety-days = 90 天
settings-platform-create-api-key-label-type = 类型
settings-platform-create-api-key-label-expiration = 过期时间
settings-platform-create-api-key-error-no-current-team = 无法创建团队 API 密钥，因为当前没有团队。
settings-platform-create-api-key-error-create-failed = API 密钥创建失败。请重试。
settings-platform-create-api-key-secret-once = 此密钥只会显示一次。请复制并安全保存。
settings-platform-create-api-key-copied = 已复制
settings-platform-create-api-key-done = 完成
settings-platform-create-api-key-creating = 正在创建…
settings-platform-create-api-key-create = 创建密钥
settings-platform-create-api-key-toast-secret-copied = 密钥已复制。

# --- ANCHOR-SUB-KEYBINDINGS (agent-settings-keybindings) ---
settings-keybindings-search-placeholder = 按名称或按键搜索（例如 "cmd d"）
settings-keybindings-conflict-warning = 此快捷键与其他快捷键冲突
settings-keybindings-button-default = 默认
settings-keybindings-button-cancel = 取消
settings-keybindings-button-clear = 清除
settings-keybindings-button-save = 保存
settings-keybindings-press-new-shortcut = 按下新的快捷键
settings-keybindings-description = 在下方为已有操作添加你自己的自定义快捷键。
settings-keybindings-use-prefix = 使用
settings-keybindings-use-suffix = 可随时在侧边栏中参考这些快捷键。
settings-keybindings-not-synced-tooltip = 快捷键不会同步到云端
settings-keybindings-subheader = 配置键盘快捷键
settings-keybindings-command-column = 命令

# --- ANCHOR-SUB-REFERRALS (agent-settings-referrals) ---
settings-referrals-page-title = 邀请朋友加入 Warp
settings-referrals-anonymous-header = 注册以参与 Warp 推荐计划
settings-referrals-sign-up = 注册
settings-referrals-link-label = 链接
settings-referrals-email-label = 邮箱
settings-referrals-link-error = 加载邀请码失败。
settings-referrals-loading = 加载中...
settings-referrals-copy-link-button = 复制链接
settings-referrals-email-send-button = 发送
settings-referrals-email-sending-button = 发送中...
settings-referrals-link-copied-toast = 链接已复制。
settings-referrals-email-success-toast = 邮件发送成功。
settings-referrals-email-failure-toast = 邮件发送失败，请重试。
settings-referrals-email-empty-error = 请输入邮箱。
settings-referrals-email-invalid-error = 请确认下列邮箱有效：{ $email }
settings-referrals-reward-intro = 推荐朋友即可获得 Warp 专属周边*
settings-referrals-claimed-count-singular = 当前推荐
settings-referrals-claimed-count-plural = 当前推荐
settings-referrals-terms-link = 部分条款适用。
settings-referrals-terms-contact = { " " }如对推荐计划有任何疑问，请联系 referrals@warp.dev。
settings-referrals-reward-theme = 专属主题
settings-referrals-reward-keycaps = 键帽 + 贴纸
settings-referrals-reward-tshirt = T 恤
settings-referrals-reward-notebook = 笔记本
settings-referrals-reward-cap = 棒球帽
settings-referrals-reward-hoodie = 连帽衫
settings-referrals-reward-hydroflask = 高级 Hydro Flask 水壶
settings-referrals-reward-backpack = 双肩包

# --- ANCHOR-SUB-WARPIFY (agent-settings-warpify) ---
settings-warpify-page-title = Warpify
settings-warpify-description-prefix = 配置 Warp 是否尝试对特定 Shell 执行 "Warpify"（为其添加命令块、输入模式等支持）。
settings-warpify-learn-more = 了解更多
settings-warpify-section-subshells = 子 Shell
settings-warpify-section-subshells-subtitle = 支持的子 Shell:bash、zsh、fish。
settings-warpify-section-ssh = SSH
settings-warpify-section-ssh-subtitle = 对交互式 SSH 会话启用 Warpify。
settings-warpify-added-commands = 已添加的命令
settings-warpify-denylisted-commands = 拒绝列表中的命令
settings-warpify-denylisted-hosts = 拒绝列表中的主机
settings-warpify-command-placeholder = 命令（支持正则）
settings-warpify-host-placeholder = 主机（支持正则）
settings-warpify-enable-ssh = 对 SSH 会话启用 Warpify
settings-warpify-install-ssh-extension = 安装 SSH 扩展
settings-warpify-install-ssh-extension-description = 控制远程主机未安装 Warp 的 SSH 扩展时的安装行为。
settings-warpify-use-tmux = 使用 Tmux Warpify
settings-warpify-tmux-description = tmux ssh 包装器在许多默认方式无效的场景下能正常工作，但可能需要你手动点击按钮才能 Warpify。在新标签页中生效。
settings-warpify-ssh-tmux-toggle-binding-label = 用于 Warpify 的 SSH 会话检测

# --- ANCHOR-SUB-AI-PAGE (agent-settings-ai-page) ---
# 章节 / 副标题
settings-ai-warp-agent-header = Warp 智能体
settings-ai-active-ai-section = 主动 AI
settings-ai-input-section = 输入
settings-ai-mcp-servers-section = MCP 服务器
settings-ai-knowledge-section = Rules
settings-ai-voice-section = 语音
settings-ai-other-section = 其他
settings-ai-third-party-cli-section = 第三方 CLI 智能体
settings-ai-experimental-section = 实验性
settings-ai-aws-bedrock-section = AWS Bedrock
settings-ai-agents-header = 智能体
settings-ai-profiles-header = 配置
settings-ai-models-subheader = 模型
settings-ai-permissions-subheader = 权限
settings-ai-usage-header = 使用量
settings-ai-credits-label = 额度

# 主动 AI 开关标签
settings-ai-next-command-label = 下一条命令
settings-ai-prompt-suggestions-label = 提示建议
settings-ai-suggested-code-banners-label = 代码建议横幅
settings-ai-natural-language-autosuggestions-label = 自然语言自动建议
settings-ai-git-operations-autogen-label = 提交与 Pull Request 生成

# 权限下拉项
settings-ai-permission-agent-decides = 智能体决定
settings-ai-permission-always-allow = 始终允许
settings-ai-permission-always-ask = 始终询问
settings-ai-permission-ask-on-first-write = 首次写入时询问
settings-ai-permission-read-only = 只读
settings-ai-permission-supervised = 受监督
settings-ai-permission-allow-specific-dirs = 在指定目录中允许

# 权限行标签
settings-ai-apply-code-diffs = 应用代码 diff
settings-ai-read-files = 读取文件
settings-ai-execute-commands = 执行命令
settings-ai-interact-running-commands = 与运行中的命令交互
settings-ai-call-mcp-servers = 调用 MCP 服务器
settings-ai-command-denylist = 命令拒绝列表
settings-ai-command-denylist-description = 匹配命令的正则表达式，Warp 智能体执行这些命令前必须征得许可。
settings-ai-command-allowlist = 命令允许列表
settings-ai-command-allowlist-description = 匹配命令的正则表达式，Warp 智能体可自动执行这些命令。
settings-ai-directory-allowlist = 目录允许列表
settings-ai-directory-allowlist-description = 授予智能体对指定目录的文件访问权限。
settings-ai-mcp-allowlist = MCP 允许列表
settings-ai-mcp-allowlist-description = 允许 Warp 智能体调用这些 MCP 服务器。
settings-ai-mcp-denylist = MCP 拒绝列表
settings-ai-mcp-denylist-description = Warp 智能体调用此列表中的任何 MCP 服务器前都必须征得许可。
settings-ai-info-banner-managed-by-workspace = 你的部分权限由工作区管理。

# 模型 / 配置
settings-ai-base-model = 基础模型
settings-ai-base-model-description = 此模型作为 Warp 智能体背后的主要引擎，驱动大部分交互，并在需要时调用其他模型完成规划或代码生成等任务。Warp 可能根据模型可用性自动切换备用模型，或将其用于会话摘要等辅助任务。
settings-ai-show-model-picker-in-prompt = 在提示中显示模型选择器
settings-ai-codebase-context = 代码库上下文
settings-ai-codebase-context-description = 允许 Warp 智能体生成代码库的概要作为上下文。代码从不存储到我们的服务器。
settings-ai-add-profile = 新建配置
settings-ai-agents-description = 设定智能体的运行边界：它能访问什么、拥有多少自主权、以及何时必须征得你的同意。你也可以微调自然语言输入、代码库感知等行为。
settings-ai-profiles-description = 配置让你定义智能体的运行方式 —— 包括它可执行的动作、何时需要审批，以及编码、规划等任务使用的模型。你也可以将其作用于具体项目。

# 匿名 / 组织限制
settings-ai-sign-up = 注册
settings-ai-anonymous-create-account = 要使用 AI 功能，请先创建账户。
settings-ai-org-disallows-remote-session = 当活动窗格包含来自远程会话的内容时，你的组织禁止使用 AI
settings-ai-org-enforced-tooltip = 此选项由你所在组织的设置强制启用，无法自定义。
settings-ai-restricted-billing = 因账单问题受限
settings-ai-unlimited = 不限量

# AI 输入区段
settings-ai-show-input-hint-text = 显示输入提示文本
settings-ai-show-agent-tips = 显示智能体提示
settings-ai-include-agent-commands-in-history = 将智能体执行的命令纳入历史
settings-ai-autodetect-agent-prompts = 在终端输入中自动检测智能体提示
settings-ai-autodetect-terminal-commands = 在智能体输入中自动检测终端命令
settings-ai-natural-language-detection = 自然语言检测
settings-ai-natural-language-denylist = 自然语言拒绝列表
settings-ai-natural-language-denylist-description = 列出的命令永远不会触发自然语言检测。
settings-ai-let-us-know = 告诉我们

# MCP 服务器
settings-ai-learn-more = 了解更多
settings-ai-add-server = 添加服务器
settings-ai-manage-mcp-servers = 管理 MCP 服务器
settings-ai-file-based-mcp-toggle = 从第三方智能体自动启动服务器
settings-ai-file-based-mcp-supported-providers = 查看支持的提供商。
settings-ai-mcp-dropdown-header = 选择 MCP 服务器

# 知识库 / 规则
settings-ai-rules-label = 规则
settings-ai-suggested-rules-label = 规则建议
settings-ai-suggested-rules-description = 让 AI 根据你的交互建议要保存的规则。
settings-ai-manage-rules = 管理规则
settings-ai-rules-description = 规则帮助 Warp 智能体遵循你的约定，无论是针对代码库还是特定工作流。

# 语音
settings-ai-voice-input-label = 语音输入
settings-ai-voice-key = 激活语音输入的按键
settings-ai-voice-key-hint = 按住以激活。

# 其他区段
settings-ai-show-use-agent-footer = 显示「使用智能体」页脚
settings-ai-use-agent-footer-description = 在长时间运行的命令中提示使用启用了「完整终端使用」的智能体。
settings-ai-show-conversation-history = 在工具面板中显示会话历史
settings-ai-thinking-display = 智能体思考显示
settings-ai-thinking-display-description = 控制推理/思考过程的显示方式。
settings-ai-conversation-layout-label = 打开已有智能体会话时的首选布局
settings-ai-conversation-layout-newtab = 新标签页
settings-ai-conversation-layout-splitpane = 拆分窗格
settings-ai-toolbar-layout = 工具栏布局

# 第三方 CLI 智能体
settings-ai-show-coding-agent-toolbar = 显示编码智能体工具栏
settings-ai-auto-show-rich-input = 根据智能体状态自动显示/隐藏富输入
settings-ai-auto-show-rich-input-tooltip = 需要为你的编码智能体安装 Warp 插件
settings-ai-auto-open-rich-input = 编码智能体会话启动时自动打开富输入
settings-ai-auto-dismiss-rich-input = 提交提示后自动关闭富输入
settings-ai-toolbar-commands-label = 启用工具栏的命令
settings-ai-toolbar-commands-description = 添加正则表达式，匹配的命令将显示编码智能体工具栏。
settings-ai-coding-agent-other = 其他
settings-ai-coding-agent-select-header = 选择编码智能体

# 实验性 / 智能体
settings-ai-cloud-agent-computer-use = 在智能体中启用计算机使用
settings-ai-cloud-agent-computer-use-description = 在 Warp 应用中启动的智能体会话中启用计算机使用。
settings-ai-orchestration-label = 编排
settings-ai-orchestration-description = 启用多智能体编排，允许智能体派生并协调并行的子智能体。

# AWS Bedrock
settings-ai-aws-bedrock-toggle = 使用 AWS Bedrock 凭证
settings-ai-aws-bedrock-description = Warp 加载并发送本地 AWS CLI 凭证以使用 Bedrock 支持的模型。
settings-ai-aws-bedrock-description-managed = Warp 加载并发送本地 AWS CLI 凭证以使用 Bedrock 支持的模型。此设置由你的组织管理。
settings-ai-aws-login-command = 登录命令
settings-ai-aws-profile = AWS Profile
settings-ai-aws-auto-login = 自动运行登录命令
settings-ai-aws-auto-login-description = 启用后，AWS Bedrock 凭证过期时将自动运行登录命令。
settings-ai-refresh = 刷新

# --- ANCHOR-SUB-FEATURES (agent-settings-features) ---
settings-features-category-general = 通用
settings-features-category-session = 会话
settings-features-category-keys = 按键
settings-features-category-text-editing = 文本编辑
settings-features-category-terminal-input = 终端输入
settings-features-category-terminal = 终端
settings-features-category-notifications = 通知
settings-features-category-workflows = 工作流
settings-features-category-system = 系统
settings-features-open-links-in-desktop = 在桌面应用中打开链接
settings-features-open-links-in-desktop-tooltip = 尽可能自动在桌面应用中打开链接。
settings-features-restore-session = 启动时恢复窗口、标签页和面板
settings-features-persist-conversations = 保存智能体对话到本地历史
settings-features-show-sticky-command-header = 显示固定的命令标题栏
settings-features-show-link-tooltip = 点击链接时显示提示
settings-features-show-quit-warning = 退出/登出前显示警告
settings-features-quit-on-last-window-closed = 关闭所有窗口时退出应用
settings-features-show-changelog-after-update = 更新后显示更新日志提示
settings-features-mouse-scroll-multiplier = 鼠标滚轮每次滚动的行数
settings-features-auto-open-code-review = 自动打开代码评审面板
settings-features-max-rows-per-block = 命令块最大行数
settings-features-ssh-wrapper = Warp SSH 包装器
settings-features-receive-desktop-notifications = 接收来自 Warp 的桌面通知
settings-features-show-in-app-agent-notifications = 显示应用内 Agent 通知
settings-features-confirm-close-shared-session = 关闭共享会话前确认
settings-features-global-hotkey-label = 全局快捷键：
settings-features-global-hotkey-not-supported-on-wayland = Wayland 下不支持。
settings-features-autocomplete-symbols = 自动补全引号、圆括号和方括号
settings-features-error-underlining = 命令错误下划线提示
settings-features-syntax-highlighting = 命令语法高亮
settings-features-completions-while-typing = 输入时自动打开补全菜单
settings-features-command-corrections = 建议修正后的命令
settings-features-expand-aliases = 输入时展开别名
settings-features-middle-click-paste = 中键点击粘贴
settings-features-vim-mode = 使用 Vim 快捷键编辑代码和命令
settings-features-at-context-menu = 在终端模式下启用「@」上下文菜单
settings-features-slash-commands-in-terminal = 在终端模式下启用斜杠命令
settings-features-outline-codebase-symbols = 为「@」上下文菜单提取代码库符号大纲
settings-features-show-input-message-bar = 显示终端输入消息行
settings-features-show-autosuggestion-hint = 显示自动建议快捷键提示
settings-features-show-autosuggestion-ignore = 显示自动建议忽略按钮
settings-features-enable-mouse-reporting = 启用鼠标事件上报
settings-features-enable-scroll-reporting = 启用滚动事件上报
settings-features-enable-focus-reporting = 启用焦点事件上报
settings-features-use-audible-bell = 启用响铃
settings-features-double-click-smart-selection = 双击智能选择
settings-features-show-help-block-in-new-sessions = 新会话中显示帮助命令块
settings-features-copy-on-select = 选中即复制
settings-features-show-global-workflows-in-command-search = 在命令搜索（ctrl-r）中显示全局工作流
settings-features-linux-selection-clipboard = 兼容 Linux 选区剪贴板
settings-features-prefer-low-power-gpu = 新窗口优先使用集成 GPU 渲染（低功耗）
settings-features-use-wayland = 使用 Wayland 进行窗口管理
settings-features-use-wayland-tooltip = 启用 Wayland 支持
settings-features-ctrl-tab-behavior-label = Ctrl+Tab 行为：
settings-features-extra-meta-key-left-mac = 左 Option 键作为 Meta
settings-features-extra-meta-key-right-mac = 右 Option 键作为 Meta
settings-features-extra-meta-key-left-other = 左 Alt 键作为 Meta
settings-features-extra-meta-key-right-other = 右 Alt 键作为 Meta
settings-features-default-shell-header = 新会话默认 shell
settings-features-working-directory-header = 新会话工作目录
settings-features-notify-agent-task-completed = Agent 完成任务时通知
settings-features-notify-needs-attention = 命令或 Agent 需要继续操作时通知
settings-features-play-notification-sounds = 播放通知声音
settings-features-default-session-mode = 新会话默认模式
settings-features-block-rows-description = 将上限设置为超过 10 万行可能影响性能。最大支持行数为 { $max_rows }。
settings-features-toast-duration-label = Toast 通知保持显示时长
settings-features-tab-key-behavior = Tab 键行为
settings-features-graphics-backend-label = 首选图形后端
settings-features-graphics-backend-current = 当前后端：{ $backend }
settings-features-working-dir-home = 用户主目录
settings-features-working-dir-previous = 上一个会话的目录
settings-features-working-dir-custom = 自定义目录
settings-features-undo-close-enable = 启用重新打开已关闭的会话
settings-features-undo-close-grace-period = 宽限期（秒）
settings-features-configure-global-hotkey = 配置全局快捷键
settings-features-make-default-terminal = 将 Warp 设为默认终端
settings-features-pin-top = 固定到顶部
settings-features-pin-bottom = 固定到底部
settings-features-pin-left = 固定到左侧
settings-features-pin-right = 固定到右侧
settings-features-default-option = 默认
settings-features-tab-behavior-completions = 打开补全菜单
settings-features-tab-behavior-autosuggestions = 接受自动建议
settings-features-tab-behavior-user-defined = 用户自定义
settings-features-new-tab-placement-all = 在所有标签页之后
settings-features-new-tab-placement-current = 在当前标签页之后
settings-features-width-percent = 宽度 %
settings-features-height-percent = 高度 %
settings-features-autohide-on-focus-loss = 键盘焦点丢失时自动隐藏
settings-features-long-running-prefix = 当命令执行时间超过
settings-features-long-running-suffix = 秒时完成
settings-features-keybinding-label = 快捷键
settings-features-click-set-global-hotkey = 点击设置全局快捷键
settings-features-cancel = 取消
settings-features-save = 保存
settings-features-press-new-shortcut = 按下新的键盘快捷键
settings-features-change-keybinding = 更改快捷键
settings-features-active-screen = 当前屏幕
settings-features-wayland-window-restore-warning = 在 Wayland 下不会恢复窗口位置。
settings-features-see-docs = 查看文档。
settings-features-allowed-values-1-20 = 允许的取值范围：1-20
settings-features-supports-floating-1-20 = 支持 1 到 20 之间的浮点数。
settings-features-auto-open-code-review-description = 打开此设置后，对话中第一次接受 diff 时会自动打开代码评审面板
settings-features-default-terminal-current = Warp 已是默认终端
settings-features-takes-effect-new-sessions = 此更改会在新会话中生效
settings-features-seconds = 秒
settings-features-vim-system-clipboard = 将未命名寄存器设为系统剪贴板
settings-features-vim-status-bar = 显示 Vim 状态栏
settings-features-tab-behavior-right-arrow-accepts = 按右箭头可接受自动建议。
settings-features-tab-behavior-key-accepts = { $keybinding } 可接受自动建议。
settings-features-completions-open-while-typing-sentence = 输入时会自动打开补全菜单。
settings-features-completions-open-while-typing-or-key = 输入时会自动打开补全菜单（或按 { $keybinding }）。
settings-features-open-completions-unbound = 打开补全菜单未绑定快捷键。
settings-features-tab-behavior-key-opens-completions = { $keybinding } 可打开补全菜单。
settings-features-word-characters-label = 视为单词一部分的字符
settings-features-new-tab-placement = 新标签页位置
settings-features-linux-selection-clipboard-tooltip = 是否支持 Linux 主选区剪贴板。
settings-features-changes-apply-new-windows = 更改会应用到新窗口。
settings-features-wayland-description = 启用此设置会禁用全局快捷键支持。禁用时，如果 Wayland 合成器使用分数缩放（例如 125%），文本可能会发虚。
settings-features-restart-warp-to-apply = 重启 Warp 以使更改生效。

# --- ANCHOR-SUB-TEAMS (agent-settings-teams) ---
settings-teams-page-title = 团队
settings-teams-create-page-subtitle = 创建团队
settings-teams-create-description = 创建团队后，您可以通过共享云端 agent 运行、环境、自动化与产物来协作进行 agent 驱动开发，也可以为队友与 agent 共享一个统一的知识库。
settings-teams-create-button = 创建
settings-teams-team-name-placeholder = 团队名称
settings-teams-rename-placeholder = 新的团队名称
settings-teams-leave-team-button = 退出团队
settings-teams-delete-team-button = 删除团队
settings-teams-emails-placeholder = 邮箱（逗号分隔）
settings-teams-domains-placeholder = 域名（逗号分隔）
settings-teams-set-button = 设置
settings-teams-invite-button = 邀请
settings-teams-join-button = 加入
settings-teams-contact-admin-button = 联系管理员申请访问
settings-teams-tab-link = 邀请链接
settings-teams-tab-email = 邮箱邀请
settings-teams-section-team-members = 团队成员
settings-teams-section-team-members-pricing = 团队成员
settings-teams-section-invite-by-link = 通过链接邀请
settings-teams-section-invite-by-email = 通过邮箱邀请
settings-teams-section-restrict-by-domain = 按域名限制
settings-teams-section-make-discoverable = 设为可被发现
settings-teams-section-plan-usage-free = 免费方案用量限制
settings-teams-section-plan-usage = 方案用量限制
settings-teams-shared-notebooks = 共享笔记本
settings-teams-shared-workflows = 共享工作流
settings-teams-reset-links = 重置链接
settings-teams-compare-plans = 对比方案
settings-teams-upgrade-build = 升级到 Build
settings-teams-upgrade-turbo = 升级到 Turbo 方案
settings-teams-upgrade-lightspeed = 升级到 Lightspeed 方案
settings-teams-contact-support = 联系支持
settings-teams-manage-billing = 管理账单
settings-teams-manage-plan = 管理方案
settings-teams-open-admin-panel = 打开管理员面板
settings-teams-or-join-existing = 或加入您公司内已有的团队
settings-teams-discovery-cta = 加入此团队，开始一起协作工作流、笔记本等内容。
settings-teams-discovery-1-teammate = 1 名队友
settings-teams-discovery-n-teammates = { $count } 名队友
settings-teams-transfer-modal-title = 转移团队所有权？
settings-teams-action-cancel-invite = 取消邀请
settings-teams-action-transfer-ownership = 转移所有权
settings-teams-action-demote-from-admin = 取消管理员
settings-teams-action-promote-to-admin = 设为管理员
settings-teams-action-remove-from-team = 从团队移除
settings-teams-action-remove-domain = 移除域名
settings-teams-state-expired = 已过期
settings-teams-state-pending = 待处理
settings-teams-state-owner = 拥有者
settings-teams-state-admin = 管理员
settings-teams-badge-past-due = 已逾期
settings-teams-badge-unpaid = 未付款
settings-teams-offline = 您当前处于离线状态。
settings-teams-failed-load-invite-link = 加载邀请链接失败。
settings-teams-toast-link-copied = 链接已复制到剪贴板！
settings-teams-toast-invite-sent-one = 邀请已发出！
settings-teams-toast-invites-sent = 已发出 { $count } 封邀请！
settings-teams-toast-domain-added = 已添加域名限制：{ $count }
settings-teams-toast-invalid-domains = 无效域名：{ $count }
settings-teams-toast-invalid-emails = 无效邮箱：{ $count }
settings-teams-toast-toggled-invite-links = 已切换邀请链接状态
settings-teams-toast-reset-invite-links = 已重置邀请链接
settings-teams-toast-deleted-invite = 已删除邀请
settings-teams-toast-toggled-discoverability = 已切换团队可发现状态
settings-teams-toast-joined-team = 已成功加入团队
settings-teams-toast-joined-team-named = 已成功加入 { $name }
settings-teams-toast-transferred-ownership = 已成功转移团队所有权
settings-teams-toast-updated-role = 已成功更新团队成员角色
settings-teams-toast-left-team = 已成功退出团队
settings-teams-toast-renamed-team = 已成功重命名团队
settings-teams-error-leave-team = 退出团队失败
settings-teams-error-rename-team = 重命名团队失败
settings-teams-error-send-invite = 发送邀请失败
settings-teams-error-toggle-invite-links = 切换邀请链接失败
settings-teams-error-reset-invite-links = 重置邀请链接失败
settings-teams-error-delete-invite = 删除邀请失败
settings-teams-error-add-domain = 添加域名限制失败
settings-teams-error-delete-domain = 删除域名限制失败
settings-teams-error-upgrade-link = 生成升级链接失败，请联系 feedback@warp.dev
settings-teams-error-billing-link = 生成账单链接失败，请联系 feedback@warp.dev
settings-teams-error-toggle-discoverability = 切换团队可发现状态失败
settings-teams-error-join-team = 加入团队失败
settings-teams-error-transfer-ownership = 转移团队所有权失败
settings-teams-error-update-role = 更新团队成员角色失败

# --- ANCHOR-SUB-SETTINGS-PAGE-NAV (agent-settings-page-nav) ---

# ---- settings_page.rs ----
settings-page-info-icon-tooltip = 点击查看文档详情
settings-page-local-only-icon-tooltip = 此设置不会同步到你的其他设备
settings-page-reset-to-default = 重置为默认值

# ---- delete_environment_confirmation_dialog.rs ----
settings-confirm-cancel = 取消
settings-confirm-delete-environment-button = 删除环境
settings-confirm-delete-environment-title = 删除环境？
settings-confirm-delete-environment-description = 确定要删除 { $name } 环境吗？

# ---- directory_color_add_picker.rs ----
settings-color-picker-add-directory-footer = + 添加目录…
settings-color-picker-add-directory-color = 添加目录颜色

# ---- settings_file_footer.rs ----
settings-footer-open-file = 打开设置文件
settings-footer-alert-open-file = 打开文件
settings-footer-alert-fix-with-oz = 用 Oz 修复

# --- ANCHOR-SUB-CODE (agent-settings-code) ---
settings-code-auto-open-review-panel = 自动打开代码评审面板
settings-code-auto-open-review-panel-desc = 启用后，代码评审面板将在会话首次接受 diff 时自动打开。
settings-code-show-code-review-button = 显示代码评审按钮
settings-code-show-code-review-button-desc = 在窗口右上角显示用于切换代码评审面板的按钮。
settings-code-show-diff-stats = 在代码评审按钮上显示差异统计
settings-code-show-diff-stats-desc = 在代码评审按钮上显示新增与删除行数。
settings-code-project-explorer = 项目浏览器
settings-code-project-explorer-desc = 在左侧工具面板添加 IDE 风格的项目浏览器 / 文件树。
settings-code-global-search = 全局文件搜索
settings-code-global-search-desc = 在左侧工具面板添加全局文件搜索。

# --- ANCHOR-SUB-PRIVACY (agent-settings-privacy) ---
settings-privacy-page-title = 隐私
settings-privacy-modal-add-regex-title = 添加正则表达式
settings-privacy-safe-mode-title = 敏感信息混淆
settings-privacy-safe-mode-description = 启用此设置后，Warp 会扫描 Block、Warp Drive 对象内容以及 Oz 提示词中可能包含的敏感信息，并阻止将这些数据保存或发送到任何服务器。你可以通过正则表达式自定义匹配规则。
settings-privacy-user-secret-regex-title = 自定义敏感信息混淆
settings-privacy-user-secret-regex-description = 使用正则表达式定义你希望额外混淆的敏感信息或数据。规则将在下一条命令执行时生效。可在正则表达式前加 (?i) 标志使其忽略大小写。
settings-privacy-telemetry-title = 帮助改进 Warp
settings-privacy-telemetry-description = 应用分析数据帮助我们为你改进产品。我们可能会采集部分控制台交互数据，用于改进 Warp 的 AI 能力。
settings-privacy-telemetry-description-old = 应用分析数据帮助我们为你改进产品。我们仅采集应用使用元数据，绝不采集控制台输入或输出内容。
settings-privacy-telemetry-free-tier-note = 免费版必须启用分析才能使用 AI 功能。
settings-privacy-telemetry-docs-link = 详细了解 Warp 如何使用数据
settings-privacy-data-management-title = 管理你的数据
settings-privacy-data-management-description = 你可以随时选择永久删除 Warp 账户。删除后将无法继续使用 Warp。
settings-privacy-data-management-link = 访问数据管理页面
settings-privacy-policy-title = 隐私政策
settings-privacy-policy-link = 阅读 Warp 的隐私政策
settings-privacy-tab-personal = 个人
settings-privacy-tab-enterprise = 企业
settings-privacy-enterprise-readonly = 企业版敏感信息混淆规则不可修改。
settings-privacy-enterprise-empty = 你所在组织尚未配置任何企业版正则规则。
settings-privacy-recommended = 推荐
settings-privacy-add-all = 全部添加
settings-privacy-add-regex-button = 添加正则
settings-privacy-enterprise-enabled-by-org = 已由你所在组织启用。
settings-privacy-zdr-badge = ZDR
settings-privacy-zdr-tooltip = 你的管理员已为团队启用零数据保留。用户生成的内容将永远不会被采集。
settings-privacy-secret-display-mode-title = 敏感信息可视化混淆模式
settings-privacy-secret-display-mode-description = 选择敏感信息在 Block 列表中的视觉呈现方式，同时保持可搜索。此设置仅影响 Block 列表中的显示。
settings-privacy-crash-reports-title = 发送崩溃报告
settings-privacy-crash-reports-description = 崩溃报告有助于排查问题并提升稳定性。
settings-privacy-cloud-conv-title = 将 AI 对话存储到云端
settings-privacy-cloud-conv-description-on = 智能体对话可与他人共享，并在你于其它设备登录时保留。此数据仅用于产品功能，Warp 不会将其用于分析。
settings-privacy-cloud-conv-description-off = 智能体对话仅本地存储，登出后丢失，且无法共享。注意：Ambient Agent 的对话数据仍存储在云端。
settings-privacy-org-managed-tooltip = 此设置由你所在组织管理。
settings-privacy-network-log-title = 网络日志控制台
settings-privacy-network-log-description = 我们构建了原生控制台，允许你查看 Warp 与外部服务器的全部通信，让你确信工作始终安全。
settings-privacy-network-log-link = 查看网络日志

# --- ANCHOR-SUB-EXEC-MODAL-BLOCKS (agent-settings-misc) ---
# ---- execution_profile_view ----
settings-exec-profile-edit-button = 编辑
settings-exec-profile-auto = 自动
settings-exec-profile-section-models = 模型
settings-exec-profile-section-permissions = 权限
settings-exec-profile-base-model = 基础模型：
settings-exec-profile-full-terminal-use = 完整终端使用：
settings-exec-profile-title-model = 标题生成：
settings-exec-profile-active-ai-model = 主动式 AI:
settings-exec-profile-next-command-model = Next Command:
settings-exec-profile-computer-use = 电脑使用：
settings-exec-profile-apply-code-diffs = 应用代码 diff:
settings-exec-profile-read-files = 读取文件：
settings-exec-profile-execute-commands = 执行命令：
settings-exec-profile-interact-running-commands = 与运行中命令交互：
settings-exec-profile-ask-questions = 提问：
settings-exec-profile-call-mcp-servers = 调用 MCP 服务器：
settings-exec-profile-call-web-tools = 调用 Web 工具：
settings-exec-profile-chips-none = 无
settings-exec-profile-perm-agent-decides = Agent 自行决定
settings-exec-profile-perm-always-allow = 始终允许
settings-exec-profile-perm-always-ask = 始终询问
settings-exec-profile-perm-unknown = 未知
settings-exec-profile-perm-ask-on-first-write = 首次写入时询问
settings-exec-profile-perm-never = 从不
settings-exec-profile-perm-never-ask = 从不询问
settings-exec-profile-perm-ask-unless-auto-approve = 除非自动批准否则询问
settings-exec-profile-perm-on = 开
settings-exec-profile-perm-off = 关
settings-exec-profile-directory-allowlist = 目录允许列表：
settings-exec-profile-command-allowlist = 命令允许列表：
settings-exec-profile-command-denylist = 命令拒绝列表：
settings-exec-profile-mcp-allowlist = MCP 允许列表：
settings-exec-profile-mcp-denylist = MCP 拒绝列表：

# ---- execution_profile_editor (Profile Editor pane) ----
settings-exec-profile-editor-header = 配置文件编辑器
settings-exec-profile-editor-title = 编辑配置文件
settings-exec-profile-editor-name-label = 名称
settings-exec-profile-editor-default-name-info = 默认配置文件的名称无法修改。
settings-exec-profile-editor-workspace-override-tooltip = 该选项由你所在组织的设置强制指定，无法自定义。
settings-exec-profile-editor-section-models = 模型
settings-exec-profile-editor-section-permissions = 权限
settings-exec-profile-editor-base-model = 基础模型
settings-exec-profile-editor-base-model-desc = 该模型作为智能体的主要引擎，驱动绝大多数交互，并在需要时调用其他模型完成规划或代码生成等任务。Warp 可能基于模型可用性或辅助任务（如对话摘要）自动切换到备选模型。
settings-exec-profile-editor-full-terminal-use-model = 完整终端使用模型
settings-exec-profile-editor-full-terminal-use-model-desc = 智能体在交互式终端应用（如数据库 shell、调试器、REPL、开发服务器）内运行时使用的模型 —— 读取实时输出并向 PTY 写入命令。
settings-exec-profile-editor-title-model = 标题生成模型
settings-exec-profile-editor-title-model-desc = 用于生成简洁对话标题的模型。默认沿用基础模型 —— 在此选择更便宜 / 更快的模型可在不影响智能体主推理的前提下节省标题摘要的 token。
settings-exec-profile-editor-active-ai-model = 主动式 AI 模型
settings-exec-profile-editor-active-ai-model-desc = 主动式 AI 功能使用的模型：命令完成后的提示建议、智能体输入框中的自然语言自动补全，以及代码库相关性排序。默认沿用基础模型 —— 在此选择小型 / 快速模型可让这些功能保持流畅，而不影响智能体的主推理。
settings-exec-profile-editor-next-command-model = Next Command 模型
settings-exec-profile-editor-next-command-model-desc = 用于预测你下一条 shell 命令的模型（灰色行内自动建议 + 块结束后的零状态建议）。对延迟敏感 —— 请选择你拥有的最小 / 最快 BYOP 模型。默认沿用基础模型。
settings-exec-profile-editor-computer-use-model = 电脑使用模型
settings-exec-profile-editor-computer-use-model-desc = 智能体接管你的电脑、通过鼠标移动、点击和键盘输入与图形化应用交互时使用的模型。
settings-exec-profile-editor-apply-code-diffs = 应用代码 diff
settings-exec-profile-editor-read-files = 读取文件
settings-exec-profile-editor-execute-commands = 执行命令
settings-exec-profile-editor-interact-running-commands = 与运行中命令交互
settings-exec-profile-editor-computer-use = 电脑使用
settings-exec-profile-editor-ask-questions = 提问
settings-exec-profile-editor-call-mcp-servers = 调用 MCP 服务器
settings-exec-profile-editor-call-web-tools = 调用 Web 工具
settings-exec-profile-editor-call-web-tools-desc = 智能体可在有助于完成任务时使用 Web 搜索。
settings-exec-profile-editor-directory-allowlist = 目录允许列表
settings-exec-profile-editor-directory-allowlist-desc = 授予智能体对特定目录的文件访问权限。
settings-exec-profile-editor-command-allowlist = 命令允许列表
settings-exec-profile-editor-command-allowlist-desc = 用于匹配可被 Oz 自动执行的命令的正则表达式。
settings-exec-profile-editor-command-denylist = 命令拒绝列表
settings-exec-profile-editor-command-denylist-desc = 用于匹配 Oz 必须每次询问权限才能执行的命令的正则表达式。
settings-exec-profile-editor-mcp-allowlist = MCP 允许列表
settings-exec-profile-editor-mcp-allowlist-desc = 允许被 Oz 调用的 MCP 服务器。
settings-exec-profile-editor-mcp-denylist = MCP 拒绝列表
settings-exec-profile-editor-mcp-denylist-desc = 不允许被 Oz 调用的 MCP 服务器。

# ---- agent_assisted_environment_modal ----
settings-env-modal-add-repo = 添加仓库
settings-env-modal-cancel = 取消
settings-env-modal-create-environment = 创建环境
settings-env-modal-selected-repos = 已选仓库
settings-env-modal-no-repos-selected = 尚未选择仓库
settings-env-modal-available-repos = 可用的已索引仓库
settings-env-modal-loading = 正在加载本地已索引仓库…
settings-env-modal-empty-no-indexed = 尚未发现本地已索引仓库。请先索引一个仓库后再试。
settings-env-modal-unavailable-build = 当前构建不支持选择本地仓库。
settings-env-modal-all-selected = 所有本地已索引仓库均已选中。
settings-env-modal-unknown-repo-name = （未知）
settings-env-modal-not-git-repo = 所选文件夹不是 Git 仓库：{ $path }
settings-env-modal-no-directory-selected = 未选择目录
settings-env-modal-dialog-title = 为你的环境选择仓库
settings-env-modal-dialog-description-indexed = 选择本地已索引仓库，为环境创建 agent 提供上下文。
settings-env-modal-dialog-description-default = 选择仓库，为环境创建 agent 提供上下文。

# ---- show_blocks_view ----
settings-show-blocks-page-title = 共享命令块
settings-show-blocks-unshare-menu-item = 取消共享
settings-show-blocks-copy-link = 复制链接
settings-show-blocks-deleting = 正在删除...
settings-show-blocks-executed-on = 执行于：{ $time }
settings-show-blocks-empty = 你还没有任何共享命令块。
settings-show-blocks-loading = 正在获取命令块...
settings-show-blocks-load-failed = 加载命令块失败，请重试。
settings-show-blocks-link-copied = 链接已复制。
settings-show-blocks-unshare-success = 命令块已成功取消共享。
settings-show-blocks-unshare-failed = 取消共享命令块失败，请重试。
settings-show-blocks-confirm-dialog-title = 取消共享命令块
settings-show-blocks-confirm-dialog-text = 确定要取消共享此命令块吗？

    取消后将无法通过链接访问，并将从 Warp 服务器永久删除。
settings-show-blocks-confirm-cancel = 取消
settings-show-blocks-confirm-unshare = 取消共享

# --- ANCHOR-SUB-APPEARANCE (agent-settings-appearance) ---

# Categories
settings-appearance-category-themes = 主题
settings-appearance-category-language = 语言
settings-appearance-category-icon = 图标
settings-appearance-category-window = 窗口
settings-appearance-category-input = 输入
settings-appearance-category-panes = 窗格
settings-appearance-category-blocks = 命令块
settings-appearance-category-text = 文本
settings-appearance-category-cursor = 光标
settings-appearance-category-tabs = 标签页
settings-appearance-category-fullscreen-apps = 全屏应用

# Theme widget
settings-appearance-theme-create-custom = 创建你自己的自定义主题
settings-appearance-theme-mode-light = 浅色
settings-appearance-theme-mode-dark = 深色
settings-appearance-theme-mode-current = 当前主题
settings-appearance-theme-sync-os-label = 跟随系统
settings-appearance-theme-sync-os-description = 当系统切换浅色/深色时自动跟随。

# Custom App Icon widget
settings-appearance-custom-icon-label = 自定义应用图标
settings-appearance-custom-icon-bundle-warning = 修改应用图标需要应用以 bundle 形式运行。
settings-appearance-custom-icon-restart-warning = 你可能需要重启 Warp 才能让 macOS 应用所选图标样式。

# Window widgets
settings-appearance-window-custom-size-label = 以自定义尺寸打开新窗口
settings-appearance-window-columns-label = 列数
settings-appearance-window-rows-label = 行数
settings-appearance-window-opacity-label = 窗口不透明度：
settings-appearance-window-opacity-value = 窗口不透明度：{ $value }
settings-appearance-window-opacity-not-supported = 当前显卡驱动不支持透明效果。
settings-appearance-window-opacity-graphics-warning = 当前图形设置可能不支持透明窗口渲染。
settings-appearance-window-opacity-graphics-warning-hint = 请尝试在 Features > System 中调整图形后端或集成 GPU 设置。
settings-appearance-window-blur-radius = 窗口模糊半径：{ $value }
settings-appearance-window-blur-texture-label = 启用窗口模糊（Acrylic 纹理）
settings-appearance-window-blur-texture-not-supported = 当前硬件可能不支持透明窗口渲染。
settings-appearance-tools-panel-consistent-label = 工具面板在所有标签页保持一致显示

# Input
settings-appearance-input-type-label = 输入类型
settings-appearance-input-type-warp = Warp
settings-appearance-input-type-shell = Shell (PS1)
settings-appearance-input-position-label = 输入位置
settings-appearance-input-mode-pinned-bottom = 固定在底部（Warp 模式）
settings-appearance-input-mode-pinned-top = 固定在顶部（反转模式）
settings-appearance-input-mode-waterfall = 从顶部开始（经典模式）

# Panes
settings-appearance-pane-dim-inactive-label = 暗化非活动窗格
settings-appearance-pane-focus-follows-mouse-label = 焦点跟随鼠标

# Blocks
settings-appearance-block-compact-label = 紧凑模式
settings-appearance-block-jump-bottom-label = 显示「跳到命令块底部」按钮
settings-appearance-block-show-dividers-label = 显示命令块分隔线

# Text / Fonts
settings-appearance-font-agent-label = Agent 字体
settings-appearance-font-match-terminal = 匹配终端
settings-appearance-font-terminal-label = 终端字体
settings-appearance-font-view-all-system = 查看所有可用系统字体
settings-appearance-font-weight-label = 字重
settings-appearance-font-size-label = 字号（像素）
settings-appearance-font-line-height-label = 行高
settings-appearance-font-reset-default = 恢复默认
settings-appearance-font-notebook-size-label = 笔记本字号
settings-appearance-font-thin-strokes-label = 使用细笔画
settings-appearance-font-thin-strokes-never = 从不
settings-appearance-font-thin-strokes-low-dpi = 仅低 DPI 显示器
settings-appearance-font-thin-strokes-high-dpi = 仅高 DPI 显示器
settings-appearance-font-thin-strokes-always = 始终
settings-appearance-font-min-contrast-label = 强制最低对比度
settings-appearance-font-min-contrast-always = 始终
settings-appearance-font-min-contrast-named-only = 仅命名颜色
settings-appearance-font-min-contrast-never = 从不
settings-appearance-font-ligatures-label = 在终端显示连字
settings-appearance-font-ligatures-perf-tooltip = 连字可能影响性能

# Cursor
settings-appearance-cursor-type-label = 光标类型
settings-appearance-cursor-disabled-vim = Vim 模式下光标类型已禁用
settings-appearance-cursor-blink-label = 闪烁光标

# Tabs
settings-appearance-tab-close-position-label = 标签页关闭按钮位置
settings-appearance-tab-close-position-right = 右侧
settings-appearance-tab-close-position-left = 左侧
settings-appearance-tab-show-indicators-label = 显示标签页指示器
settings-appearance-tab-show-code-review-label = 显示代码审查按钮
settings-appearance-tab-preserve-active-color-label = 新标签页保留当前标签页颜色
settings-appearance-tab-vertical-layout-label = 使用垂直标签页布局
settings-appearance-tab-show-vertical-panel-in-restored-windows-label = 恢复窗口时显示垂直标签页面板
settings-appearance-tab-show-vertical-panel-in-restored-windows-description = 启用后，重新打开或恢复窗口时会展开垂直标签页面板，即使上次保存时该面板是关闭的。
settings-appearance-tab-show-title-bar-search-bar-label = 在标题栏显示搜索框
settings-appearance-tab-show-title-bar-search-bar-description = 在标签栏中央显示「搜索会话、智能体、文件...」搜索框，点击打开命令面板。关闭后该位置留空。仅在垂直标签页布局下生效。
workspace-title-bar-search-placeholder = 搜索会话、智能体、文件...
settings-appearance-tab-use-prompt-as-title-label = 在标签页名称中使用最近的用户提示作为对话标题
settings-appearance-tab-use-prompt-as-title-description = 在垂直标签页中，对内置 AI 与第三方 agent 会话显示最近的用户提示，而不是生成的对话标题。
settings-appearance-tab-toolbar-layout-label = 标题栏工具条布局
settings-appearance-tab-directory-colors-label = 目录标签页颜色
settings-appearance-tab-directory-colors-description = 根据当前目录或仓库自动为标签页着色。
settings-appearance-tab-directory-color-default-tooltip = 默认（无颜色）
settings-appearance-zen-mode-label = 显示标签栏
settings-appearance-zen-decoration-always = 始终
settings-appearance-zen-decoration-windowed = 仅在窗口模式
settings-appearance-zen-decoration-on-hover = 仅悬停时

# Full-screen apps
settings-appearance-alt-screen-padding-label = 在 alt 屏幕中使用自定义内边距
settings-appearance-alt-screen-uniform-padding-label = 统一内边距（像素）

# Zoom
settings-appearance-zoom-label = 缩放
settings-appearance-zoom-secondary = 调整所有窗口的默认缩放级别

# --- ANCHOR-SUB-ENVIRONMENTS (agent-settings-environments) ---
settings-environments-page-title = 环境
settings-environments-page-description = 环境定义了你的 ambient agent 在哪里运行。可通过 GitHub（推荐）、Warp 辅助配置或手动配置在几分钟内创建一个。
settings-environments-search-placeholder = 搜索环境...
settings-environments-no-matches = 没有符合搜索条件的环境。
settings-environments-section-personal = 个人
settings-environments-section-team-default = 由 Warp 和你的团队共享
settings-environments-section-team-named = 由 Warp 和 { $team } 共享
settings-environments-env-id-prefix = 环境 ID:{ $id }
settings-environments-detail-image = 镜像：{ $image }
settings-environments-detail-repos = 仓库：{ $repos }
settings-environments-detail-setup-commands = 初始命令：{ $commands }
settings-environments-last-edited = 最近编辑：{ $time }
settings-environments-last-used = 最近使用：{ $time }
settings-environments-last-used-never = 最近使用：从未
settings-environments-view-my-runs = 查看我的运行记录
settings-environments-tooltip-share = 分享
settings-environments-tooltip-edit = 编辑
settings-environments-empty-header = 你还没有创建任何环境。
settings-environments-empty-subheader = 选择你想要的环境创建方式：
settings-environments-empty-quick-setup-title = 快速创建
settings-environments-empty-suggested-badge = 推荐
settings-environments-empty-quick-setup-subtitle = 选择你想要使用的 GitHub 仓库，我们会为你建议基础镜像与配置
settings-environments-empty-use-agent-title = 使用 agent
settings-environments-empty-use-agent-subtitle = 选择一个本地已配置好的项目，我们会基于它帮你创建环境
settings-environments-button-loading = 加载中...
settings-environments-button-retry = 重试
settings-environments-button-authorize = 授权
settings-environments-button-get-started = 开始使用
settings-environments-button-launch-agent = 启动 agent
settings-environments-toast-update-success = 环境更新成功
settings-environments-toast-create-success = 环境创建成功
settings-environments-toast-delete-success = 环境删除成功
settings-environments-toast-share-success = 环境分享成功
settings-environments-toast-share-failure = 环境分享给团队失败
settings-environments-toast-create-not-logged-in = 无法创建环境：未登录。
settings-environments-toast-save-not-found = 无法保存：环境已不存在。
settings-environments-toast-share-no-team = 无法分享环境：你当前未加入任何团队。
settings-environments-toast-share-not-synced = 无法分享环境：环境尚未同步。
settings-update-environment-name-placeholder = 环境名称
settings-update-environment-docker-image-placeholder = 例如 python:3.11、node:20-alpine
settings-update-environment-repos-placeholder-authed = 输入仓库（owner/repo 格式）
settings-update-environment-repos-placeholder-unauthenticated = 粘贴仓库 URL
settings-update-environment-setup-command-placeholder = 例如 cd my-repo && pip install -r requirements.txt
settings-update-environment-description-placeholder = 例如，这个环境适用于所有偏前端的智能体

# --- ANCHOR-SUB-AGENT-PROVIDERS (agent-settings-agent-providers) ---
settings-agent-providers-title = Agent 提供商
settings-agent-providers-description = 配置自定义 Agent 提供商，支持多协议 —— OpenAI 兼容（DeepSeek、智谱 GLM、Moonshot、通义千问 DashScope、SiliconFlow、OpenRouter 等）、Anthropic、Gemini、本地 Ollama。可以手动添加模型（显示名 + 模型 ID 映射），也可以从 API 自动抓取。提供商元数据存储在本地 settings.toml，API 密钥安全存储在系统密钥库。
settings-agent-providers-empty = 尚未配置任何提供商。点击右上角 [+ 添加提供商] 按钮添加。
settings-agent-providers-add-button = + 添加提供商
settings-agent-providers-search-placeholder = 搜索提供商…
settings-agent-providers-quick-add-title = 快速添加
settings-agent-providers-refresh-catalog = 刷新目录
settings-agent-providers-loading-catalog = 正在拉取 models.dev 目录…（第一次可能需要几秒）
settings-agent-providers-catalog-empty = models.dev 目录为空，点 [刷新目录] 重试。
settings-agent-providers-no-match = 无匹配「{ $query }」
settings-agent-providers-collapse = 收起 ▲
settings-agent-providers-expand-remaining = 展开剩余 { $count } 个 ▼
settings-agent-providers-row-missing = （此提供商还未关联编辑器：{ $id }）
settings-agent-providers-field-name = 名称
settings-agent-providers-field-base-url = 接口地址
settings-agent-providers-field-api-key = API 密钥
settings-agent-providers-field-api-type = API 协议
settings-agent-providers-api-type-hint = （genai 据此显式绑定 adapter，避免按模型名误识别。接口地址留空将使用默认：{ $url }）
settings-agent-providers-name-placeholder = 自定义提供商名称（例如：DeepSeek、本地 Ollama）
settings-agent-providers-api-key-placeholder = sk-...（可选，本地无认证服务如 ollama 留空即可）
settings-agent-providers-models-label = 模型列表（{ $count } 个）
settings-agent-providers-models-empty-hint = 还未配置模型。点 [+ 添加模型] 手动添加，或点 [Fetch from API] 自动抓取。
settings-agent-providers-models-header-name = 显示名
settings-agent-providers-models-header-id = 模型 ID
settings-agent-providers-models-header-context = 上下文（tok）
settings-agent-providers-models-header-output = 输出（tok）
settings-agent-providers-model-name-placeholder = 显示名（例如：DS-V3 通用）
settings-agent-providers-model-id-placeholder = 模型 ID（发给 API 的 model 字段，如：deepseek-chat）
settings-agent-providers-model-context-placeholder = 上下文（tokens）
settings-agent-providers-model-output-placeholder = 输出（tokens）
settings-agent-providers-add-model = + 添加模型
settings-agent-providers-fetch-from-api = 从 API 抓取
settings-agent-providers-sync-models-dev = 从 models.dev 同步
settings-agent-providers-remove = 移除
settings-agent-providers-save = 保存
settings-agent-providers-saved-toast = 已保存

# ---- AI 子页 ----
settings-ai-title = AI
settings-ai-active-ai = 主动 AI
settings-ai-input-autodetection = 智能体输入框中的终端命令自动识别
settings-ai-input-autodetection-legacy = 自然语言识别
settings-ai-next-command-description = 根据你的历史命令、输出与常见工作流，让 AI 推荐下一条要执行的命令。
settings-ai-prompt-suggestions-description = 让 AI 根据最近命令与输出，在输入框中以行内横幅形式建议自然语言提示。
settings-ai-suggested-code-banners-description = 让 AI 根据最近命令与输出，在命令块列表中以行内横幅形式建议代码差异与查询。
settings-ai-natural-language-autosuggestions = 让 AI 根据最近命令与输出，提供自然语言自动建议。
settings-ai-git-operations-autogen-description = 让 AI 自动生成提交信息以及 Pull Request 的标题与描述。

# =============================================================================
# 其余 surface 章节缺失 key 会自动 fallback 到英文，见 en/warp.ftl
# =============================================================================

# =============================================================================
# SECTION: banner (Owner: agent-banner)
# Files: app/src/banner/**
# =============================================================================

banner-dont-show-again = 不再显示


# =============================================================================
# SECTION: quit-warning (Owner: agent-quit-warning)
# Files: app/src/quit_warning/mod.rs
# =============================================================================

# ---- 对话框标题 ----
quit-warning-title-pane = 关闭窗格？
quit-warning-title-tab-singular = 关闭标签页？
quit-warning-title-tab-plural = 关闭标签页？
quit-warning-title-window = 关闭窗口？
quit-warning-title-app = 退出 Warp？
quit-warning-title-editor-tab = 保存更改？

# ---- 按钮 ----
quit-warning-button-confirm-close = 确定关闭
quit-warning-button-confirm-quit = 确定退出
quit-warning-button-save = 保存
quit-warning-button-discard = 不保存
quit-warning-button-show-processes = 查看运行中的进程
quit-warning-button-cancel = 取消

# ---- 提示正文 ----
quit-warning-suffix-tab = { " " }（此标签页）。
quit-warning-suffix-window = { " " }（此窗口）。
quit-warning-suffix-pane = { " " }（此窗格）。
quit-warning-suffix-default = 。

quit-warning-processes-running = 你有 { $count } 个进程正在运行
quit-warning-processes-in-windows = ，分布在 { $count } 个窗口
quit-warning-processes-in-tabs = ，分布在 { $count } 个标签页

quit-warning-shared-sessions = 你正在共享 { $count } 个会话

quit-warning-unsaved-changes = 你有未保存的文件更改

quit-warning-unsaved-editor-tab = 是否保存对 { $file } 所做的更改？如果不保存，这些更改将被丢弃。
quit-warning-unsaved-editor-tab-fallback-name = 此文件

# --- ANCHOR-SUB-RULES-PAGE (agent-rules-page) ---
# Manage Rules 页面（Warp Drive 中的 AI Fact Collection）。
rules-collection-name = 规则

# --- ANCHOR-SUB-KEYBINDING-DESC (agent-keybinding-descriptions) ---
# 键盘快捷键设置页 / 命令面板里每个 binding 的 description 文案。
# binding name（如 `workspace:open_settings_file`）是协议字段（用户自定义快捷键持久化用），
# **不翻译**。

# 标签页 / 会话
keybinding-desc-workspace-cycle-next-session = 切换到下一个标签页
keybinding-desc-workspace-cycle-prev-session = 切换到上一个标签页
keybinding-desc-workspace-add-window = 创建新窗口
keybinding-desc-workspace-new-file = 新建文件
keybinding-desc-workspace-zoom-in = 放大
keybinding-desc-workspace-zoom-out = 缩小
keybinding-desc-workspace-reset-zoom = 重置缩放
keybinding-desc-workspace-increase-font-size = 增大字号
keybinding-desc-workspace-decrease-font-size = 减小字号
keybinding-desc-workspace-reset-font-size = 重置字号为默认值
keybinding-desc-workspace-increase-zoom = 放大缩放级别
keybinding-desc-workspace-decrease-zoom = 缩小缩放级别
keybinding-desc-workspace-reset-zoom-level = 重置缩放级别为默认值
keybinding-desc-workspace-save-launch-config = 保存新启动配置

# 项目浏览器 / 面板
keybinding-desc-workspace-toggle-project-explorer = 切换项目浏览器
keybinding-desc-workspace-toggle-project-explorer-menu = 项目浏览器
keybinding-desc-workspace-show-theme-chooser = 打开主题选择器
keybinding-desc-workspace-toggle-tab-configs-menu = 打开标签页配置菜单

# 切换到第 N 个标签页
keybinding-desc-workspace-activate-1st-tab = 切换到第 1 个标签页
keybinding-desc-workspace-activate-2nd-tab = 切换到第 2 个标签页
keybinding-desc-workspace-activate-3rd-tab = 切换到第 3 个标签页
keybinding-desc-workspace-activate-4th-tab = 切换到第 4 个标签页
keybinding-desc-workspace-activate-5th-tab = 切换到第 5 个标签页
keybinding-desc-workspace-activate-6th-tab = 切换到第 6 个标签页
keybinding-desc-workspace-activate-7th-tab = 切换到第 7 个标签页
keybinding-desc-workspace-activate-8th-tab = 切换到第 8 个标签页
keybinding-desc-workspace-activate-last-tab = 切换到最后一个标签页
keybinding-desc-workspace-activate-prev-tab = 激活上一个标签页
keybinding-desc-workspace-activate-next-tab = 激活下一个标签页

# 窗格导航
keybinding-desc-pane-group-navigate-prev = 激活上一个窗格
keybinding-desc-pane-group-navigate-next = 激活下一个窗格

# 鼠标 / 笔记本 / 工作流 / 文件夹
keybinding-desc-workspace-toggle-mouse-reporting = 切换鼠标报告
keybinding-desc-workspace-create-team-notebook = 新建团队笔记本
keybinding-desc-workspace-create-team-notebook-menu = 新建团队笔记本
keybinding-desc-workspace-create-personal-notebook = 新建个人笔记本
keybinding-desc-workspace-create-personal-notebook-menu = 新建个人笔记本
keybinding-desc-workspace-create-team-workflow = 新建团队工作流
keybinding-desc-workspace-create-team-workflow-menu = 新建团队工作流
keybinding-desc-workspace-create-personal-workflow = 新建个人工作流
keybinding-desc-workspace-create-personal-workflow-menu = 新建个人工作流
keybinding-desc-workspace-create-team-folder = 新建团队文件夹
keybinding-desc-workspace-create-team-folder-menu = 新建团队文件夹
keybinding-desc-workspace-create-personal-folder = 新建个人文件夹
keybinding-desc-workspace-create-personal-folder-menu = 新建个人文件夹

# 新建标签页变体
keybinding-desc-workspace-new-tab = 创建新标签页
keybinding-desc-workspace-new-terminal-tab = 新建终端标签页
keybinding-desc-workspace-new-agent-tab = 新建 Agent 标签页
keybinding-desc-workspace-new-cloud-agent-tab = 新建 Agent 标签页
new-session-create-new-tab = 新建标签页
new-session-create-new-window = 新建窗口
new-session-split-pane-down = 向下拆分窗格
new-session-split-pane-right = 向右拆分窗格
new-session-split-pane-up = 向上拆分窗格
new-session-split-pane-left = 向左拆分窗格
new-session-create-new-tab-with-shell = 新建标签页: { $shell }
new-session-create-new-window-with-shell = 新建窗口: { $shell }
new-session-split-pane-with-shell = 向{ $direction }拆分窗格: { $shell }
new-session-direction-down = 下
new-session-direction-right = 右
new-session-direction-up = 上
new-session-direction-left = 左

# 左 / 右面板切换
keybinding-desc-workspace-toggle-left-panel = 打开左侧面板
keybinding-desc-workspace-toggle-right-panel = 切换代码评审
keybinding-desc-workspace-toggle-right-panel-menu = 切换代码评审
keybinding-desc-workspace-toggle-vertical-tabs = 切换垂直标签页面板
keybinding-desc-workspace-toggle-vertical-tabs-menu = 切换垂直标签页面板
keybinding-desc-workspace-left-panel-agent-conversations = 左侧面板：Agent 对话
keybinding-desc-workspace-left-panel-project-explorer = 左侧面板：项目浏览器
keybinding-desc-workspace-left-panel-global-search = 左侧面板：全局搜索
keybinding-desc-workspace-left-panel-warp-drive = 左侧面板：Warp Drive
keybinding-desc-workspace-left-panel-ssh-manager = 左侧面板：SSH 管理器
keybinding-desc-workspace-left-panel-skill-manager = 左侧面板：Skill 管理器
keybinding-desc-workspace-open-global-search = 打开全局搜索
keybinding-desc-workspace-open-global-search-menu = 全局搜索
keybinding-desc-workspace-toggle-warp-drive = 切换 Warp Drive
keybinding-desc-workspace-toggle-warp-drive-menu = Warp Drive
keybinding-desc-workspace-toggle-conversation-list-view = 切换 Agent 对话列表视图
keybinding-desc-workspace-toggle-conversation-list-view-menu = Agent 对话列表视图
keybinding-desc-workspace-close-panel = 关闭聚焦面板

# 命令面板 / 导航
keybinding-desc-workspace-toggle-command-palette = 切换命令面板
keybinding-desc-workspace-toggle-command-palette-menu = 命令面板
keybinding-desc-workspace-toggle-navigation-palette = 切换导航面板
keybinding-desc-workspace-toggle-navigation-palette-menu = 导航面板
keybinding-desc-workspace-toggle-launch-config-palette = 启动配置面板
keybinding-desc-workspace-toggle-files-palette = 切换文件面板
keybinding-desc-workspace-search-drive = 搜索 Warp Drive
keybinding-desc-workspace-move-tab-left = 标签页左移
keybinding-desc-workspace-move-tab-up = 标签页上移
keybinding-desc-workspace-move-tab-right = 标签页右移
keybinding-desc-workspace-move-tab-down = 标签页下移

# 快捷键设置
keybinding-desc-workspace-toggle-keybindings-page = 切换键盘快捷键
keybinding-desc-workspace-show-keybinding-settings = 打开快捷键编辑器
keybinding-desc-workspace-toggle-block-snackbar = 切换粘性命令头

# 窗口 / 标签页关闭
keybinding-desc-workspace-rename-active-tab = 重命名当前标签页
keybinding-desc-workspace-terminate-app = 退出 Warp
keybinding-desc-workspace-close-window = 关闭窗口
keybinding-desc-workspace-close-active-tab = 关闭当前标签页
keybinding-desc-workspace-close-other-tabs = 关闭其他标签页
keybinding-desc-workspace-close-tabs-right = 关闭右侧的标签页
keybinding-desc-workspace-close-tabs-below = 关闭下方的标签页

# 通知
keybinding-desc-workspace-toggle-notifications-on = 开启通知
keybinding-desc-workspace-toggle-notifications-off = 关闭通知

# 更新 / 更新日志
keybinding-desc-workspace-update-and-relaunch = 安装更新并重启
keybinding-desc-workspace-check-for-updates = 检查更新
keybinding-desc-workspace-view-changelog = 查看最新更新日志

# 资源中心 / Drive 导出 / CLI
keybinding-desc-workspace-toggle-resource-center = 切换资源中心
keybinding-desc-workspace-export-all-warp-drive-objects = 导出所有 Warp Drive 对象
keybinding-desc-workspace-install-cli = 安装 Oz CLI 命令
keybinding-desc-workspace-uninstall-cli = 卸载 Oz CLI 命令

# AI 助手 / agent
keybinding-desc-workspace-toggle-ai-assistant = 切换 Warp AI

# 环境变量 / Prompt
keybinding-desc-workspace-create-team-env-vars = 新建团队环境变量
keybinding-desc-workspace-create-team-env-vars-menu = 新建团队环境变量
keybinding-desc-workspace-create-personal-env-vars = 新建个人环境变量
keybinding-desc-workspace-create-personal-env-vars-menu = 新建个人环境变量
keybinding-desc-workspace-create-personal-ai-prompt = 新建个人 Prompt
keybinding-desc-workspace-create-personal-ai-prompt-menu = 新建个人 Prompt
keybinding-desc-workspace-create-team-ai-prompt = 新建团队 Prompt
keybinding-desc-workspace-create-team-ai-prompt-menu = 新建团队 Prompt

# 焦点 / 导入
keybinding-desc-workspace-shift-focus-left = 切换焦点到左侧面板
keybinding-desc-workspace-shift-focus-right = 切换焦点到右侧面板
keybinding-desc-workspace-import-to-personal-drive = 导入到个人 Drive
keybinding-desc-workspace-import-to-team-drive = 导入到团队 Drive

# Drive / 仓库 / AI Rules / MCP
keybinding-desc-workspace-open-repository = 打开仓库
keybinding-desc-workspace-open-repository-menu = 打开仓库
keybinding-desc-workspace-open-ai-fact-collection = 打开 AI Rules
keybinding-desc-workspace-open-mcp-servers = 打开 MCP 服务器
keybinding-desc-workspace-jump-to-latest-toast = 跳转到最新 agent 任务
keybinding-desc-workspace-toggle-notification-mailbox = 切换通知邮箱
keybinding-desc-workspace-toggle-agent-management-view = 切换 agent 管理视图

# 设置页面
keybinding-desc-workspace-show-settings = 打开设置
keybinding-desc-workspace-show-settings-menu = 设置
keybinding-desc-workspace-show-settings-account = 打开设置：账户
keybinding-desc-workspace-show-settings-appearance = 打开设置：外观
keybinding-desc-workspace-show-settings-appearance-menu = 外观...
keybinding-desc-workspace-show-settings-features = 打开设置：功能
keybinding-desc-workspace-show-settings-shared-blocks = 打开设置：共享命令块
keybinding-desc-workspace-show-settings-shared-blocks-menu = 查看共享命令块...
keybinding-desc-workspace-show-settings-keyboard-shortcuts = 打开设置：键盘快捷键
keybinding-desc-workspace-show-settings-keyboard-shortcuts-menu = 配置键盘快捷键...
keybinding-desc-workspace-show-settings-about = 打开设置：关于
keybinding-desc-workspace-show-settings-about-menu = 关于 Warp
keybinding-desc-workspace-show-settings-teams = 打开设置：团队
keybinding-desc-workspace-show-settings-teams-menu = 打开团队设置
keybinding-desc-workspace-show-settings-privacy = 打开设置：隐私
keybinding-desc-workspace-show-settings-warpify = 打开设置：Warpify
keybinding-desc-workspace-show-settings-warpify-menu = 配置 Warpify...
keybinding-desc-workspace-show-settings-ai = 打开设置：AI
keybinding-desc-workspace-show-settings-code = 打开设置：代码
keybinding-desc-workspace-show-settings-referrals = 打开设置：推荐
keybinding-desc-workspace-show-settings-environments = 打开设置：环境
keybinding-desc-workspace-show-settings-mcp-servers = 打开设置：MCP 服务器
keybinding-desc-workspace-open-settings-file = 打开设置文件

# 溢出菜单 / 外部链接
keybinding-desc-workspace-link-to-slack = 加入我们的 Slack 社区（打开外部链接）
keybinding-desc-workspace-link-to-user-docs = 查看用户文档（打开外部链接）
keybinding-desc-workspace-send-feedback = 发送反馈（打开外部链接）
keybinding-desc-workspace-send-feedback-oz = 用 Oz 发送反馈
keybinding-desc-workspace-view-logs = 查看 Warp 日志
keybinding-desc-workspace-link-to-privacy-policy = 查看隐私政策（打开外部链接）

# 输入 / 终端 / 项目相关 binding（注册在 workspace/mod.rs 之外）
keybinding-desc-input-edit-prompt = 编辑 Prompt
keybinding-desc-terminal-attach-block-as-context = 将所选块作为 Agent 上下文附加
keybinding-desc-terminal-attach-text-as-context = 将所选文本作为 Agent 上下文附加
keybinding-desc-terminal-attach-as-context-menu = 将所选内容作为 Agent 上下文附加
keybinding-desc-workspace-init-project = 为 Warp 初始化项目
keybinding-desc-workspace-add-current-folder = 将当前文件夹添加为项目

# Workspace 调试 / crash / sentry / 堆分析相关 binding
keybinding-desc-workspace-crash-macos = 触发崩溃（用于测试 sentry-cocoa）
keybinding-desc-workspace-crash-other = 触发崩溃（用于测试 sentry-native）
keybinding-desc-workspace-log-review-comment-send-status = [调试] 记录当前标签页的评审评论发送状态
keybinding-desc-workspace-panic = 触发 panic（用于测试 sentry-rust）
keybinding-desc-workspace-open-view-tree-debugger = 打开视图树调试器
keybinding-desc-workspace-view-first-time-user-experience = [调试] 查看首次启动引导体验
keybinding-desc-workspace-open-build-plan-migration-modal = [调试] 打开构建计划迁移弹窗
keybinding-desc-workspace-reset-build-plan-migration-modal-state = [调试] 重置构建计划迁移弹窗状态
keybinding-desc-workspace-undismiss-aws-login-banner = [调试] 取消关闭 AWS 登录提示条
keybinding-desc-workspace-open-oz-launch-modal = [调试] 打开 Oz 启动弹窗
keybinding-desc-workspace-reset-oz-launch-modal-state = [调试] 重置 Oz 启动弹窗状态
keybinding-desc-workspace-open-openwarp-launch-modal = [调试] 打开 OpenWarp 启动弹窗
keybinding-desc-workspace-reset-openwarp-launch-modal-state = [调试] 重置 OpenWarp 启动弹窗状态
keybinding-desc-workspace-install-opencode-warp-plugin = [调试] 安装 OpenCode Warp 插件
keybinding-desc-workspace-use-local-opencode-warp-plugin = [调试] 使用本地 OpenCode Warp 插件（仅测试用）
keybinding-desc-workspace-open-session-config-modal = [调试] 打开会话配置弹窗
keybinding-desc-workspace-start-hoa-onboarding-flow = [调试] 启动 HOA 引导流程
keybinding-desc-workspace-sample-process = 采样进程
keybinding-desc-workspace-dump-heap-profile = 导出堆分析（只能执行一次）

# 终端输入相关 binding
keybinding-desc-input-show-network-log = 显示 Warp 网络日志
keybinding-desc-input-clear-screen = 清屏
keybinding-desc-input-toggle-classic-completions = （实验性）切换经典补全模式
keybinding-desc-input-command-search = 命令搜索
keybinding-desc-input-history-search = 历史记录搜索
keybinding-desc-input-open-completions-menu = 打开补全菜单
keybinding-desc-input-workflows = 工作流
keybinding-desc-input-open-ai-command-suggestions = 打开 AI 命令建议
keybinding-desc-input-new-agent-conversation = 新建智能体对话
keybinding-desc-input-trigger-auto-detection = 触发自动识别
keybinding-desc-input-clear-and-reset-ai-context-menu-query = 清空并重置 AI 上下文菜单查询

# 终端视图相关 binding
keybinding-desc-terminal-alternate-paste = 终端备用粘贴
keybinding-desc-terminal-toggle-cli-agent-rich-input = 切换 CLI 智能体富文本输入
keybinding-desc-terminal-warpify-subshell = Warpify 子 shell
keybinding-desc-terminal-warpify-ssh-session = Warpify SSH 会话
keybinding-desc-terminal-accept-prompt-suggestion = 接受 Prompt 建议
keybinding-desc-terminal-cancel-process-windows = 复制文本或取消正在运行的进程
keybinding-desc-terminal-cancel-process = 取消正在运行的进程
keybinding-desc-terminal-focus-input = 聚焦终端输入
keybinding-desc-terminal-paste = 粘贴
keybinding-desc-terminal-copy = 复制
keybinding-desc-terminal-reinput-commands = 重新输入所选命令
keybinding-desc-terminal-reinput-commands-sudo = 以 root 身份重新输入所选命令
keybinding-desc-terminal-find = 在终端中查找
keybinding-desc-terminal-select-bookmark-up = 选择上方最近的书签
keybinding-desc-terminal-select-bookmark-down = 选择下方最近的书签
keybinding-desc-terminal-open-block-context-menu = 打开命令块上下文菜单
keybinding-desc-terminal-toggle-team-workflows-modal = 切换团队工作流弹窗
keybinding-desc-terminal-copy-git-branch = 复制 git 分支
keybinding-desc-terminal-clear-blocks = 清空命令块
keybinding-desc-terminal-cursor-word-left = 在执行中的命令内向左移动一个单词
keybinding-desc-terminal-cursor-word-right = 在执行中的命令内向右移动一个单词
keybinding-desc-terminal-cursor-home = 在执行中的命令内移动到行首
keybinding-desc-terminal-cursor-end = 在执行中的命令内移动到行尾
keybinding-desc-terminal-delete-word-left = 在执行中的命令内向左删除一个单词
keybinding-desc-terminal-delete-line-start = 在执行中的命令内删除到行首
keybinding-desc-terminal-delete-line-end = 在执行中的命令内删除到行尾
keybinding-desc-terminal-backward-tabulation = 在执行中的命令内反向跳格
keybinding-desc-terminal-select-previous-block = 选择上一个命令块
keybinding-desc-terminal-select-next-block = 选择下一个命令块
keybinding-desc-terminal-share-selected-block = 分享所选命令块
keybinding-desc-terminal-bookmark-selected-block = 收藏所选命令块
keybinding-desc-terminal-find-within-selected-block = 在所选命令块内查找
keybinding-desc-terminal-copy-command-and-output = 复制命令与输出
keybinding-desc-terminal-copy-command-output = 复制命令输出
keybinding-desc-terminal-copy-command = 复制命令
keybinding-desc-terminal-scroll-up-one-line = 终端输出向上滚动一行
keybinding-desc-terminal-scroll-down-one-line = 终端输出向下滚动一行
keybinding-desc-terminal-scroll-up-one-page = 终端输出向上滚动一页
keybinding-desc-terminal-scroll-down-one-page = 终端输出向下滚动一页
keybinding-desc-terminal-scroll-to-top-of-block = 滚动到所选命令块顶部
keybinding-desc-terminal-scroll-to-bottom-of-block = 滚动到所选命令块底部
keybinding-desc-terminal-select-all-blocks = 选择全部命令块
keybinding-desc-terminal-expand-blocks-above = 向上扩展所选命令块
keybinding-desc-terminal-expand-blocks-below = 向下扩展所选命令块
keybinding-desc-terminal-insert-command-correction = 插入命令纠错
keybinding-desc-terminal-setup-guide = 设置向导
keybinding-desc-terminal-onboarding-warp-input-terminal = [调试] 引导提示：WarpInput - 终端
keybinding-desc-terminal-onboarding-warp-input-project = [调试] 引导提示：WarpInput - 项目
keybinding-desc-terminal-onboarding-warp-input-no-project = [调试] 引导提示：WarpInput - 无项目
keybinding-desc-terminal-onboarding-modality-project = [调试] 引导提示：Modality - 项目
keybinding-desc-terminal-onboarding-modality-no-project = [调试] 引导提示：Modality - 无项目
keybinding-desc-terminal-onboarding-modality-terminal = [调试] 引导提示：Modality - 终端
keybinding-desc-terminal-import-external-settings = 导入外部设置
keybinding-desc-terminal-share-current-session = 分享当前会话
keybinding-desc-terminal-stop-sharing-current-session = 停止分享当前会话
keybinding-desc-terminal-toggle-block-filter = 在所选或最近的命令块上切换块过滤
keybinding-desc-terminal-toggle-sticky-command-header = 在当前面板切换粘性命令头
keybinding-desc-terminal-toggle-autoexecute-mode = 切换自动执行模式
keybinding-desc-terminal-toggle-queue-next-prompt = 切换排队下一条 Prompt

# 面板组相关 binding
keybinding-desc-pane-group-close-current-session = 关闭当前会话
keybinding-desc-pane-group-split-left = 向左分屏
keybinding-desc-pane-group-split-up = 向上分屏
keybinding-desc-pane-group-split-down = 向下分屏
keybinding-desc-pane-group-split-right = 向右分屏
keybinding-desc-pane-group-switch-left = 切换到左侧面板
keybinding-desc-pane-group-switch-right = 切换到右侧面板
keybinding-desc-pane-group-switch-up = 切换到上方面板
keybinding-desc-pane-group-switch-down = 切换到下方面板
keybinding-desc-pane-group-resize-left = 调整面板 > 分隔条左移
keybinding-desc-pane-group-resize-right = 调整面板 > 分隔条右移
keybinding-desc-pane-group-resize-up = 调整面板 > 分隔条上移
keybinding-desc-pane-group-resize-down = 调整面板 > 分隔条下移
keybinding-desc-pane-group-toggle-maximize = 切换最大化当前面板

# 根视图相关 binding
keybinding-desc-root-view-toggle-fullscreen = 切换全屏
keybinding-desc-root-view-enter-onboarding-state = [调试] 进入引导状态

# 工作流视图相关 binding
keybinding-desc-workflow-view-save = 保存工作流
keybinding-desc-workflow-view-close = 关闭

# 编辑器视图 binding desc（由 editor/view/mod.rs、code/editor/view/actions.rs、notebooks/editor/view.rs 共用）
keybinding-desc-editor-copy = 复制
keybinding-desc-editor-cut = 剪切
keybinding-desc-editor-paste = 粘贴
keybinding-desc-editor-undo = 撤销
keybinding-desc-editor-redo = 重做
keybinding-desc-editor-select-left-by-word = 向左按词选择
keybinding-desc-editor-select-right-by-word = 向右按词选择
keybinding-desc-editor-select-left = 向左选中一个字符
keybinding-desc-editor-select-right = 向右选中一个字符
keybinding-desc-editor-select-up = 向上选择
keybinding-desc-editor-select-down = 向下选择
keybinding-desc-editor-select-all = 全选
keybinding-desc-editor-select-to-line-start = 选中到行首
keybinding-desc-editor-select-to-line-end = 选中到行尾
keybinding-desc-editor-select-to-line-start-cap = 选中到行首
keybinding-desc-editor-select-to-line-end-cap = 选中到行尾
keybinding-desc-editor-clear-and-copy-lines = 复制并清除选中行
keybinding-desc-editor-add-next-occurrence = 添加下一处匹配到选区
keybinding-desc-editor-up = 光标上移
keybinding-desc-editor-down = 光标下移
keybinding-desc-editor-left = 光标左移
keybinding-desc-editor-right = 光标右移
keybinding-desc-editor-move-to-line-start = 移动到行首
keybinding-desc-editor-move-to-line-end = 移动到行尾
keybinding-desc-editor-move-to-line-start-short = 移动到行首
keybinding-desc-editor-move-to-line-end-short = 移动到行尾
keybinding-desc-editor-home = 行首
keybinding-desc-editor-end = 行尾
keybinding-desc-editor-cmd-down = 移动到末尾
keybinding-desc-editor-cmd-up = 移动到开头
keybinding-desc-editor-move-to-and-select-buffer-start = 选中并移动到开头
keybinding-desc-editor-move-to-and-select-buffer-end = 选中并移动到末尾
keybinding-desc-editor-move-forward-one-word = 向后移动一个词
keybinding-desc-editor-move-backward-one-word = 向前移动一个词
keybinding-desc-editor-move-forward-one-word-cap = 向后移动一个词
keybinding-desc-editor-move-backward-one-word-cap = 向前移动一个词
keybinding-desc-editor-move-to-paragraph-start = 移动到段落开头
keybinding-desc-editor-move-to-paragraph-end = 移动到段落末尾
keybinding-desc-editor-move-to-paragraph-start-short = 移动到段落开头
keybinding-desc-editor-move-to-paragraph-end-short = 移动到段落末尾
keybinding-desc-editor-move-to-buffer-start = 移动到缓冲区开头
keybinding-desc-editor-move-to-buffer-end = 移动到缓冲区末尾
keybinding-desc-editor-cursor-at-buffer-start = 光标移到缓冲区开头
keybinding-desc-editor-cursor-at-buffer-end = 光标移到缓冲区末尾
keybinding-desc-editor-backspace = 删除前一个字符
keybinding-desc-editor-cut-word-left = 剪切左侧词
keybinding-desc-editor-cut-word-right = 剪切右侧词
keybinding-desc-editor-delete-word-left = 删除左侧词
keybinding-desc-editor-delete-word-right = 删除右侧词
keybinding-desc-editor-cut-all-left = 剪切左侧全部
keybinding-desc-editor-cut-all-right = 剪切右侧全部
keybinding-desc-editor-delete-all-left = 删除左侧全部
keybinding-desc-editor-delete-all-right = 删除右侧全部
keybinding-desc-editor-delete = 删除
keybinding-desc-editor-clear-lines = 清除选中行
keybinding-desc-editor-insert-newline = 插入换行
keybinding-desc-editor-fold = 折叠
keybinding-desc-editor-unfold = 展开
keybinding-desc-editor-fold-selected-ranges = 折叠选中范围
keybinding-desc-editor-insert-last-word-prev-cmd = 插入上一条命令的最后一个词
keybinding-desc-editor-move-backward-one-subword = 向前移动一个子词
keybinding-desc-editor-move-forward-one-subword = 向后移动一个子词
keybinding-desc-editor-select-left-by-subword = 向左按子词选择
keybinding-desc-editor-select-right-by-subword = 向右按子词选择
keybinding-desc-editor-accept-autosuggestion = 接受自动建议
keybinding-desc-editor-inspect-command = 检查命令
keybinding-desc-editor-clear-buffer = 清空命令编辑器
keybinding-desc-editor-add-cursor-above = 在上方添加光标
keybinding-desc-editor-add-cursor-below = 在下方添加光标
keybinding-desc-editor-insert-nonexpanding-space = 插入不可扩展空格
keybinding-desc-editor-vim-exit-insert-mode = 退出 Vim 插入模式
keybinding-desc-editor-toggle-comment = 切换注释
keybinding-desc-editor-go-to-line = 跳转到行
keybinding-desc-editor-find-in-code-editor = 在代码编辑器中查找

# 代码编辑器（Code） binding desc
keybinding-desc-code-save-as = 文件另存为
keybinding-desc-code-close-all-tabs = 关闭所有标签页
keybinding-desc-code-close-saved-tabs = 关闭已保存的标签页

# 欢迎视图 binding desc
keybinding-desc-welcome-terminal-session = 终端会话
keybinding-desc-welcome-add-repository = 添加仓库

# AI 助手面板 binding desc
keybinding-desc-ai-assistant-close = 关闭 Warp AI
keybinding-desc-ai-assistant-focus-terminal-input = 从 Warp AI 切回终端输入
keybinding-desc-ai-assistant-restart = 重启 Warp AI

# 代码审阅 binding desc
keybinding-desc-code-review-save-all = 保存代码审阅中所有未保存的文件
keybinding-desc-code-review-show-find = 在代码审阅中显示查找栏

# 项目按钮 binding desc
keybinding-desc-project-buttons-open-repository = 打开仓库
keybinding-desc-project-buttons-create-new-project = 创建新项目

# 查找视图 binding desc
keybinding-desc-find-next-occurrence = 查找下一处匹配
keybinding-desc-find-prev-occurrence = 查找上一处匹配

# Notebook 文件 / 笔记本 binding desc
keybinding-desc-notebook-focus-terminal-input-from-file = 从文件切回终端输入
keybinding-desc-notebook-reload-file = 重新加载文件
keybinding-desc-notebook-increase-font-size = 增大笔记本字号
keybinding-desc-notebook-decrease-font-size = 减小笔记本字号
keybinding-desc-notebook-reset-font-size = 重置笔记本字号
keybinding-desc-notebook-focus-terminal-input = 从笔记本切回终端输入
keybinding-desc-notebook-fb-increase-font-size = 增大字号
keybinding-desc-notebook-fb-decrease-font-size = 减小字号

# Notebook 编辑器 binding desc（在共享编辑器 key 之外的）
keybinding-desc-nbeditor-deselect-command = 取消选中 shell 命令
keybinding-desc-nbeditor-select-command = 选中光标处的 shell 命令
keybinding-desc-nbeditor-select-previous-command = 选中上一条命令
keybinding-desc-nbeditor-select-next-command = 选中下一条命令
keybinding-desc-nbeditor-run-commands = 运行选中的命令
keybinding-desc-nbeditor-toggle-debug = 切换富文本调试模式
keybinding-desc-nbeditor-debug-copy-buffer = 复制富文本缓冲区
keybinding-desc-nbeditor-debug-copy-selection = 复制富文本选区
keybinding-desc-nbeditor-log-state = 输出编辑器状态日志
keybinding-desc-nbeditor-edit-link = 创建或编辑链接
keybinding-desc-nbeditor-inline-code = 切换行内代码样式
keybinding-desc-nbeditor-strikethrough = 切换删除线样式
keybinding-desc-nbeditor-underline = 切换下划线样式
keybinding-desc-nbeditor-find = 在笔记本中查找
keybinding-desc-nbeditor-next-find-match = 聚焦下一处匹配
keybinding-desc-nbeditor-previous-find-match = 聚焦上一处匹配
keybinding-desc-nbeditor-toggle-regex-find = 切换正则表达式搜索
keybinding-desc-nbeditor-toggle-case-sensitive-find = 切换大小写敏感搜索

# 面板组 / 撤销关闭 binding desc
keybinding-desc-get-started-terminal-session = 终端会话
keybinding-desc-undo-close-reopen-session = 重新打开已关闭的会话
keybinding-desc-right-panel-toggle-maximize-code-review = 切换最大化代码审阅面板

# 工作区输入同步 binding desc
keybinding-desc-workspace-disable-sync-inputs = 停止同步所有面板
keybinding-desc-workspace-toggle-sync-inputs-tab = 切换同步当前标签页所有面板
keybinding-desc-workspace-toggle-sync-inputs-all-tabs = 切换同步所有标签页中的所有面板

# 工作区辅助功能 / 调试 binding desc
keybinding-desc-workspace-a11y-concise = [a11y] 设为简洁辅助播报
keybinding-desc-workspace-a11y-verbose = [a11y] 设为详细辅助播报
keybinding-desc-workspace-copy-access-token = 复制访问令牌到剪贴板

# 环境变量集合 binding desc
keybinding-desc-env-var-collection-close = 关闭

# 鉴权 / 分享模态 binding desc
keybinding-desc-share-block-copy = 复制
keybinding-desc-auth-paste-token = 粘贴
keybinding-desc-conversation-details-copy = 复制

# 终端补充 binding desc
keybinding-desc-terminal-show-history = 显示历史
keybinding-desc-terminal-ask-ai-selection = 就所选内容询问 Warp AI
keybinding-desc-terminal-ask-ai-last-block = 就最近的命令块询问 Warp AI
keybinding-desc-terminal-ask-ai = 询问 Warp AI
keybinding-desc-terminal-load-agent-conversation = 加载智能体模式会话（从剪贴板调试链接）
keybinding-desc-terminal-toggle-session-recording = 切换会话 PTY 录制

# Notebook 编辑器补充
keybinding-desc-nbeditor-select-to-paragraph-start = 选中到段落开头
keybinding-desc-nbeditor-select-to-paragraph-end = 选中到段落末尾

# 杂项 binding desc（收尾批次：常量/LazyLock/动态描述去硬编码）
keybinding-desc-save-file = 保存文件
keybinding-desc-new-agent-pane = 新建 Agent 窗格
keybinding-desc-edit-code-diff = 编辑代码差异
keybinding-desc-edit-requested-command = 编辑请求的命令
keybinding-desc-set-input-mode-agent = 切换输入模式为 Agent 模式
keybinding-desc-set-input-mode-terminal = 切换输入模式为终端模式
keybinding-desc-toggle-hide-cli-responses = 切换隐藏 CLI 回应
keybinding-desc-slash-command = 斜杠命令：{ $name }
keybinding-desc-take-control-of-running-command = 接管正在运行的命令

# --- 终端零状态块（欢迎提示） ---
terminal-zero-state-title = 新建终端会话
terminal-zero-state-start-agent = 开始新的 Agent 对话
terminal-zero-state-cycle-history = 翻阅历史命令与对话
terminal-zero-state-open-code-review = 打开代码评审
terminal-zero-state-autodetect-prompts = 在终端会话中自动检测 Agent 提示
terminal-zero-state-dismiss = 不再显示

# --- Rules 页面（ai/facts/view/rule.rs） ---
rules-description = Rules 通过提供结构化指引来增强 Agent，帮助保持一致性、贯彻最佳实践，并适应特定工作流，包括代码库或更宏观的任务。
rules-search-placeholder = 搜索规则
rules-name-placeholder = 例如 Rust 规则
rules-description-placeholder = 例如 不要在 Rust 中使用 unwrap
rules-zero-state-global = 添加规则后，它将显示在这里。
rules-zero-state-project = 为项目生成 WARP.md 规则文件后，它将显示在这里。
rules-disabled-banner-prefix = 你的规则已禁用，不会在会话中作为上下文使用。你可以随时
rules-disabled-banner-link = 重新开启
rules-disabled-banner-suffix = 。
rules-tab-global = 全局
rules-tab-project = 项目级
rules-add-button = 添加
rules-init-project-button = 初始化项目

# --- Agent 视图零状态 + 消息栏 ---
agent-zero-state-title = 新建 Agent 对话
# OpenWarp 已移除云端 Agent 入口，此 key 实际不会被渲染;保留以匹配 en 兜底链。
agent-zero-state-title-cloud = 新建 Agent 对话
agent-zero-state-description = 在下方输入提示开始新的对话
agent-zero-state-description-with-location = 在下方输入提示，于 `{ $location }` 开始新的对话
agent-zero-state-recent-activity = 最近活动
inline-agent-header-prompt-to-interact-command = 提示智能体与 `{ $command }` 交互
inline-agent-header-prompt-to-interact-running-command = 提示智能体与正在运行的命令交互
inline-agent-header-waiting-on-instructions = 智能体正在等待指令
inline-agent-header-waiting-for-command = 智能体正在等待命令退出
inline-agent-header-agent-blocked = 智能体需要你的权限才能继续
inline-agent-header-agent-in-control = 智能体正在控制
inline-agent-header-user-in-control = 用户正在控制
agent-toolbar-edit-agent-toolbelt = 编辑智能体工具带
agent-toolbar-edit-cli-agent-toolbelt = 编辑 CLI 智能体工具带
agent-toolbar-available-chips = 可用控件
agent-message-bar-get-figma-mcp = 获取 Figma MCP
agent-message-bar-enable-figma-mcp = 启用 Figma MCP
agent-message-bar-enabling = 正在启用...
orchestration-parent-conversation = 父对话
orchestration-back-to-parent-conversation = 返回父对话
child-agent-default-name = 智能体
agent-zero-state-switch-model = 切换模型
agent-zero-state-go-back-to-terminal = 返回终端
agent-message-bar-for-help = 查看帮助
agent-message-bar-for-commands = 查看命令
agent-message-bar-open-conversation = 打开对话
agent-message-bar-for-code-review = 进入代码评审
agent-message-bar-resume-conversation = 继续对话
agent-message-bar-hide-plan = 隐藏计划
agent-message-bar-view-plans = 查看计划
agent-message-bar-view-plan = 查看计划
agent-message-bar-fork-continue = 分叉并继续
agent-message-bar-new-pane = {" "}新窗格
agent-message-bar-new-tab = {" "}新标签页
agent-message-bar-current-pane = {" "}当前窗格
agent-message-bar-hide-help = 隐藏帮助
agent-message-bar-autodetected-shell-command-prefix = 已自动识别为 shell 命令，{" "}
agent-message-bar-autodetected-shell-command = 已自动识别为 shell 命令
agent-message-bar-override = {" "}覆盖
agent-message-bar-exit-shell-mode = 退出 shell 模式
agent-message-bar-again-stop-exit = 再按一次停止并退出
agent-message-bar-again-exit = 再按一次退出
agent-message-bar-again-start-new-conversation = 再按一次开始新对话
agent-shortcuts-input-shell-command = 输入 shell 命令
agent-shortcuts-slash-commands = 打开斜杠命令
agent-shortcuts-file-paths-context = 添加文件路径和其他上下文
agent-shortcuts-open-code-review = 打开代码评审
agent-shortcuts-toggle-conversation-list = 切换对话列表
agent-shortcuts-search-continue-conversations = 搜索并继续对话
agent-shortcuts-start-new-conversation = 开始新的对话
agent-shortcuts-toggle-auto-accept = 切换自动接受
agent-shortcuts-pause-agent = 暂停 Agent
agent-error-will-resume-when-network-restored = 网络恢复后将继续对话...
agent-error-attempting-resume-conversation = 正在尝试继续对话...

# --- ANCHOR-SUB-TOGGLE-PAIR (settings-toggle-pair) ---
toggle-setting-enable = 启用{ $suffix }
toggle-setting-disable = 禁用{ $suffix }

toggle-suffix-ai = AI
toggle-suffix-active-ai = 主动式 AI
toggle-suffix-ai-input-autodetect-agent = Agent 输入中的终端命令检测
toggle-suffix-ai-input-autodetect-nld = 自然语言检测
toggle-suffix-nld-in-terminal = 终端输入中的 Agent 提示词检测
toggle-suffix-next-command = Next Command 补全
toggle-suffix-prompt-suggestions = 提示词建议
toggle-suffix-code-suggestions = 代码建议
toggle-suffix-nl-autosuggestions = 自然语言自动建议
toggle-suffix-voice-input = 语音输入
toggle-suffix-codebase-index = 代码库索引
toggle-suffix-auto-indexing = 自动索引
toggle-suffix-compact-mode = 紧凑模式
toggle-suffix-themes-sync-os = 主题：跟随系统
toggle-suffix-cursor-blink = 光标闪烁
toggle-suffix-jump-bottom-block = 跳到块底部按钮
toggle-suffix-block-dividers = 块分隔线
toggle-suffix-dim-inactive-panes = 非活动面板调暗
toggle-suffix-tab-indicators = 标签页指示器
toggle-suffix-focus-follows-mouse = 焦点跟随鼠标
toggle-suffix-zen-mode = 禅模式
toggle-suffix-vertical-tabs = 垂直标签栏布局
toggle-suffix-ligature-rendering = 连字渲染
toggle-suffix-copy-on-select = 终端内选中即复制
toggle-suffix-linux-selection-clipboard = Linux 主选择剪贴板
toggle-suffix-autocomplete-symbols = 自动补全引号、圆括号和方括号
toggle-suffix-restore-session = 启动时恢复窗口、标签页和面板
toggle-suffix-left-option-meta = 左 Option 键作为 Meta
toggle-suffix-left-alt-meta = 左 Alt 键作为 Meta
toggle-suffix-right-option-meta = 右 Option 键作为 Meta
toggle-suffix-right-alt-meta = 右 Alt 键作为 Meta
toggle-suffix-scroll-reporting = 滚动事件上报
toggle-suffix-completions-while-typing = 输入时补全
toggle-suffix-command-corrections = 命令纠错
toggle-suffix-error-underlining = 错误下划线
toggle-suffix-syntax-highlighting = 语法高亮
toggle-suffix-audible-bell = 终端响铃
toggle-suffix-autosuggestions = 自动建议
toggle-suffix-autosuggestion-keybinding-hint = 自动建议快捷键提示
toggle-suffix-ssh-wrapper = Warp SSH 包装器
toggle-suffix-link-tooltip = 点击链接显示提示
toggle-suffix-quit-warning = 退出警告弹窗
toggle-suffix-alias-expansion = 别名展开
toggle-suffix-middle-click-paste = 中键粘贴
toggle-suffix-code-as-default-editor = VS Code 作为默认编辑器
toggle-suffix-input-hint-text = 输入提示文字
toggle-suffix-vim-keybindings = 用 Vim 快捷键编辑命令
toggle-suffix-vim-clipboard = Vim 默认寄存器使用系统剪贴板
toggle-suffix-vim-status-bar = Vim 状态栏
toggle-suffix-focus-reporting = 焦点上报
toggle-suffix-smart-select = 智能选择
toggle-suffix-input-message-line = 终端输入提示行
toggle-suffix-slash-commands-terminal = 终端模式斜杠命令
toggle-suffix-integrated-gpu = 集成 GPU 渲染（低功耗）
toggle-suffix-wayland = Wayland 窗口管理
toggle-suffix-settings-sync = 设置同步
toggle-suffix-app-analytics = 应用分析
toggle-suffix-crash-reporting = 崩溃上报
toggle-suffix-secret-redaction = 敏感信息脱敏
toggle-suffix-recording-mode = 录制模式
toggle-suffix-inband-generators = 新会话使用 in-band 生成器
toggle-suffix-debug-network = 网络状态调试
toggle-suffix-memory-stats = 内存统计

# Set agent thinking display
agent-thinking-display-show-collapse = 设置 Agent 思考展示：展示并折叠
agent-thinking-display-always-show = 设置 Agent 思考展示：始终展示
agent-thinking-display-never-show = 设置 Agent 思考展示：从不展示

# --- ANCHOR-SUB-EXTERNAL-EDITOR (settings-external-editor) ---
settings-external-editor-choose-default = 选择打开文件链接的编辑器
settings-external-editor-choose-code-panels = 选择从代码评审面板、项目浏览器和全局搜索打开文件的编辑器
settings-external-editor-choose-layout = 选择在 Warp 中打开文件的布局
settings-external-editor-tabbed-header = 多个文件合并到同一编辑器面板
settings-external-editor-tabbed-desc = 开启后，同一标签页中打开的文件会自动归并到单一编辑器面板。
settings-external-editor-prefer-markdown = 默认用 Warp Markdown 查看器打开 Markdown 文件
settings-external-editor-layout-split-pane = 分屏面板
settings-external-editor-layout-new-tab = 新建标签页
settings-external-editor-default-app = 系统默认

# =============================================================================
# SECTION: context-menu (Owner: agent-context-menu)
# 鼠标右键弹出菜单
# =============================================================================

# --- block 右键菜单（terminal/view.rs） ---
menu-block-copy = 复制
menu-block-copy-url = 复制 URL
menu-block-copy-path = 复制路径
menu-block-show-in-finder = 在 Finder 中显示
menu-block-show-containing-folder = 显示所在文件夹
menu-block-open-in-warp = 在 Warp 中打开
menu-block-open-in-editor = 在编辑器中打开
menu-block-insert-into-input = 插入到输入框
menu-block-copy-command = 复制命令
menu-block-copy-commands = 复制命令
menu-block-find-within-block = 在块内查找
menu-block-find-within-blocks = 在块内查找
menu-block-scroll-to-top-of-block = 滚动到块顶部
menu-block-scroll-to-top-of-blocks = 滚动到块顶部
menu-block-scroll-to-bottom-of-block = 滚动到块底部
menu-block-scroll-to-bottom-of-blocks = 滚动到块底部
menu-block-save-as-workflow = 另存为工作流
menu-block-ask-warp-ai = 询问 Warp AI
menu-block-copy-output = 复制输出
menu-block-copy-filtered-output = 复制过滤后的输出
menu-block-toggle-block-filter = 切换块过滤器
menu-block-toggle-bookmark = 切换收藏
menu-block-copy-prompt = 复制提示符
menu-block-copy-right-prompt = 复制右侧提示符
menu-block-copy-working-directory = 复制工作目录
menu-block-copy-git-branch = 复制 git 分支
menu-block-edit-prompt = 编辑提示符
menu-block-edit-cli-agent-toolbelt = 编辑 CLI Agent 工具带
menu-block-edit-agent-toolbelt = 编辑 Agent 工具带
menu-block-split-pane-right = 向右分割面板
menu-block-split-pane-left = 向左分割面板
menu-block-split-pane-down = 向下分割面板
menu-block-split-pane-up = 向上分割面板
menu-block-close-pane = 关闭面板

# --- input 右键菜单（terminal/view.rs） ---
menu-input-cut = 剪切
menu-input-copy = 复制
menu-input-paste = 粘贴
menu-input-select-all = 全选
menu-input-command-search = 命令搜索
menu-input-ai-command-search = AI 命令搜索
menu-input-ask-warp-ai = 询问 Warp AI
menu-input-save-as-workflow = 另存为工作流
menu-input-hide-hint-text = 隐藏输入框提示文本
menu-input-show-hint-text = 显示输入框提示文本

# --- AI block overflow 菜单（terminal/view.rs） ---
menu-ai-block-copy = 复制
menu-ai-block-copy-prompt = 复制提示词
menu-ai-block-copy-output-as-markdown = 复制输出为 Markdown
menu-ai-block-copy-url = 复制 URL
menu-ai-block-copy-path = 复制路径
menu-ai-block-copy-command = 复制命令
menu-ai-block-copy-git-branch = 复制 git 分支
menu-ai-block-save-as-prompt = 另存为提示词
menu-ai-block-copy-conversation-text = 复制对话文本
menu-ai-block-fork-from-here = 从此处分叉
menu-ai-block-rewind-to-before-here = 回退到此处之前
menu-ai-block-fork-from-last-query = 从上一次提问分叉
menu-ai-block-fork-from-query = 从「{ $query }」分叉

# --- tab 右键菜单（tab.rs） ---
menu-tab-stop-sharing = 停止共享
menu-tab-share-session = 共享会话
menu-tab-stop-sharing-all = 停止共享全部
menu-tab-copy-link = 复制链接
menu-tab-rename = 重命名标签页
menu-tab-reset-name = 重置标签页名称
menu-tab-move-down = 向下移动标签页
menu-tab-move-right = 向右移动标签页
menu-tab-move-up = 向上移动标签页
menu-tab-move-left = 向左移动标签页
menu-tab-close = 关闭标签页
menu-tab-close-other = 关闭其他标签页
menu-tab-close-below = 关闭下方标签页
menu-tab-close-right = 关闭右侧标签页
menu-tab-save-as-new-config = 另存为新配置
menu-tab-default-no-color = 默认（无颜色）

# --- pane header 溢出菜单（terminal/view/pane_impl.rs） ---
menu-pane-copy-link = 复制链接
menu-pane-stop-sharing-session = 停止共享会话
menu-pane-open-on-desktop = 在桌面端打开

# --- 文件树右键菜单（code/file_tree/view.rs） ---
menu-filetree-open-in-new-pane = 在新面板中打开
menu-filetree-open-in-new-tab = 在新标签页中打开
menu-filetree-open-file = 打开文件
menu-filetree-new-file = 新建文件
menu-filetree-cd-to-directory = cd 到该目录
menu-filetree-reveal-finder = 在 Finder 中显示
menu-filetree-reveal-explorer = 在资源管理器中显示
menu-filetree-reveal-file-manager = 在文件管理器中显示
menu-filetree-rename = 重命名
menu-filetree-delete = 删除
menu-filetree-attach-as-context = 附加为上下文
menu-filetree-copy-path = 复制路径
menu-filetree-copy-relative-path = 复制相对路径

# --- 代码编辑器右键菜单（code/local_code_editor.rs） ---
menu-codeeditor-go-to-definition = 跳转到定义
menu-codeeditor-find-references = 查找引用

# --- 共享标签：附加为 agent 上下文（blocklist/view_util.rs） ---
menu-attach-as-agent-context = 附加为 agent 上下文

# --- ANCHOR-SUB-SLASH-COMMANDS (agent-slash-commands) ---
# 斜杠命令面板的描述与参数提示
# (app/src/search/slash_command_menu/static_commands/commands.rs)
slash-cmd-agent-desc = 开始新对话
slash-cmd-add-mcp-desc = 添加新的 MCP 服务器
slash-cmd-pr-comments-desc = 拉取 GitHub PR 评审评论
slash-cmd-create-environment-desc = 通过引导式流程创建 Oz 环境（Docker 镜像 + 仓库）
slash-cmd-create-environment-hint = <可选：仓库路径或 GitHub URL>
slash-cmd-docker-sandbox-desc = 创建新的 Docker 沙盒终端会话
slash-cmd-create-new-project-desc = 由 Oz 引导你创建新的代码项目
slash-cmd-create-new-project-hint = <描述你想构建什么>
slash-cmd-open-skill-desc = 在 Warp 内置编辑器中打开技能的 markdown 文件
slash-cmd-skills-desc = 调用技能
slash-cmd-add-prompt-desc = 添加新的智能体提示词
slash-cmd-add-rule-desc = 为智能体添加新的全局规则
slash-cmd-open-file-desc = 在 Warp 代码编辑器中打开文件
slash-cmd-open-file-hint = <path/to/file[:line[:col]]> 或输入「@」搜索
slash-cmd-rename-tab-desc = 重命名当前标签页
slash-cmd-rename-tab-hint = <标签页名称>
slash-cmd-fork-desc = 在新窗格或新标签页中分叉当前对话
slash-cmd-fork-hint = <可选：在分叉后的对话中发送的提示词>
slash-cmd-open-code-review-desc = 打开代码评审
slash-cmd-init-desc = 生成或更新 AGENTS.md 文件
slash-cmd-open-project-rules-desc = 打开项目规则文件（AGENTS.md）
slash-cmd-open-mcp-servers-desc = 打开 MCP 服务器
slash-cmd-open-settings-file-desc = 打开设置文件（TOML）
slash-cmd-changelog-desc = 打开最新更新日志
slash-cmd-open-repo-desc = 切换到另一个已索引的仓库
slash-cmd-open-rules-desc = 查看你的全部全局规则与项目规则
slash-cmd-new-desc = 开始新对话（/agent 的别名）
slash-cmd-model-desc = 切换基础智能体模型
slash-cmd-profile-desc = 切换当前激活的执行配置
slash-cmd-plan-desc = 让智能体调研并为任务创建计划
slash-cmd-plan-hint = <描述你的任务>
slash-cmd-orchestrate-desc = 将任务拆分为子任务并由多个智能体并行执行
slash-cmd-orchestrate-hint = <描述你的任务>
slash-cmd-compact-desc = 通过摘要对话历史来释放上下文
slash-cmd-compact-hint = <可选：自定义摘要指令>
slash-cmd-compact-and-desc = 压缩对话并随后发送一条后续提示词
slash-cmd-compact-and-hint = <压缩后要发送的提示词>
slash-cmd-queue-desc = 排队一条提示词，在智能体完成响应后再发送
slash-cmd-queue-hint = <智能体完成后要发送的提示词>
slash-cmd-fork-and-compact-desc = 分叉当前对话并在分叉副本中压缩
slash-cmd-fork-and-compact-hint = <可选：压缩后要发送的提示词>
slash-cmd-fork-from-desc = 从特定查询处分叉对话
slash-cmd-remote-control-desc = 为此会话启动远程控制
slash-cmd-conversations-desc = 打开对话历史
slash-cmd-prompts-desc = 搜索已保存的提示词
slash-cmd-rewind-desc = 倒回到对话中的上一个节点
slash-cmd-export-to-clipboard-desc = 以 markdown 格式将当前对话导出到剪贴板
slash-cmd-export-to-file-desc = 将当前对话导出为 markdown 文件
slash-cmd-export-to-file-hint = <可选：文件名>

# --- ANCHOR-SUB-PROMPT-TIPS ---
# 提示词编辑弹窗（app/src/prompt/editor_modal.rs）
prompt-editor-title = 编辑提示词
prompt-editor-warp-prompt-section = Warp 终端提示词
prompt-editor-shell-prompt-section = Shell 提示符（PS1）
prompt-editor-restore-default = 恢复默认
prompt-editor-same-line-prompt = 同行提示词
prompt-editor-separator = 分隔符
prompt-editor-cancel = 取消
prompt-editor-save-changes = 保存更改

# 欢迎提示（app/src/tips/tip_view.rs）
welcome-tips-command-palette-title = 命令面板
welcome-tips-command-palette-description = 无需双手离开键盘，即可轻松发现 Warp 的全部功能。
welcome-tips-split-pane-title = 拆分窗格
welcome-tips-split-pane-description = 将标签页拆分为多个窗格，创建理想布局。
welcome-tips-history-search-title = 历史搜索
welcome-tips-history-search-description = 查找、编辑并重新运行之前执行过的命令。
welcome-tips-ai-command-search-title = AI 命令搜索
welcome-tips-ai-command-search-description = 用自然语言生成 shell 命令。
welcome-tips-theme-picker-title = 主题选择器
welcome-tips-theme-picker-description = 选择内置主题，让 Warp 更符合你的风格。也可以创建自己的主题。
welcome-tips-shortcut-label = 快捷键
welcome-tips-skip = 跳过欢迎提示
welcome-tips-complete-title = 完成！
welcome-tips-complete-description = 欢迎提示已完成，做得不错！
welcome-tips-close = 关闭欢迎提示

# --- ANCHOR-SUB-SMALL-DIALOGS ---
# 倒回确认弹窗（app/src/workspace/rewind_confirmation_dialog.rs）
rewind-dialog-title = 倒回
rewind-dialog-body = 确定要倒回吗？这会将你的代码和对话恢复到此节点之前，并取消智能体当前正在运行的任何命令。原始对话的副本将保存到你的对话历史中。
rewind-dialog-info = 倒回不会影响手动编辑或通过 shell 命令编辑的文件。
rewind-dialog-cancel = 取消
rewind-dialog-confirm = 倒回

# 转让所有权确认弹窗（app/src/settings_view/transfer_ownership_confirmation_modal.rs）
transfer-ownership-dialog-body = 确定要将团队所有权转让给 { $name } 吗？你将不再是所有者，也无法对此团队执行任何管理操作。
transfer-ownership-dialog-cancel = 取消
transfer-ownership-dialog-confirm = 转让

# --- ANCHOR-SUB-SEARCH-PALETTES ---
# 搜索面板（app/src/search/command_palette/view.rs, app/src/search/welcome_palette/view.rs）
command-palette-search-placeholder = 搜索命令
command-palette-no-results = 未找到结果
command-palette-toast-cannot-switch-conversations = 智能体正在监控命令时，无法切换对话。
command-palette-toast-cannot-start-new-conversation = 智能体正在监控命令时，无法开始新对话。
command-palette-zero-state-recent = 最近使用
command-palette-zero-state-suggested = 推荐
welcome-palette-search-placeholder = 编码、构建，或搜索任意内容...
welcome-palette-no-results = 未找到结果
search-filter-placeholder-history = 搜索历史记录
search-filter-placeholder-workflows = 搜索工作流
search-filter-placeholder-agent-mode-workflows = 搜索提示词
search-filter-placeholder-notebooks = 搜索笔记本
search-filter-placeholder-plans = 搜索计划
search-filter-placeholder-natural-language = 例如：替换文件中的字符串
search-filter-placeholder-actions = 搜索操作
search-filter-placeholder-sessions = 搜索会话
search-filter-placeholder-conversations = 搜索对话
search-filter-placeholder-historical-conversations = 搜索历史对话
search-filter-placeholder-launch-configurations = 搜索启动配置
search-filter-placeholder-drive = 搜索 Drive 中的对象
search-filter-placeholder-environment-variables = 搜索环境变量
search-filter-placeholder-prompt-history = 搜索提示词历史
search-filter-placeholder-files = 搜索文件
search-filter-placeholder-commands = 搜索命令
search-filter-placeholder-blocks = 搜索命令块
search-filter-placeholder-code = 搜索代码符号
search-filter-placeholder-rules = 搜索 AI 规则
search-filter-placeholder-repos = 搜索代码仓库
search-filter-placeholder-diff-sets = 搜索差异集
search-filter-placeholder-static-slash-commands = 搜索静态斜杠命令
search-filter-placeholder-skills = 搜索技能
search-filter-placeholder-base-models = 搜索基础模型
search-filter-placeholder-full-terminal-use-models = 搜索完整终端使用模型
search-filter-placeholder-current-directory-conversations = 搜索当前目录中的对话
search-filter-display-history = 历史记录
search-filter-display-workflows = 工作流
search-filter-display-agent-mode-workflows = 提示词
search-filter-display-notebooks = 笔记本
search-filter-display-plans = 计划
search-filter-display-natural-language = AI 命令建议
search-filter-display-actions = 操作
search-filter-display-sessions = 会话
search-filter-display-conversations = 对话
search-filter-display-launch-configurations = 启动配置
search-filter-display-drive = Warp Drive
search-filter-display-environment-variables = 环境变量
search-filter-display-prompt-history = 提示词历史
search-filter-display-files = 文件
search-filter-display-commands = 命令
search-filter-display-blocks = 命令块
search-filter-display-code = 代码
search-filter-display-rules = 规则
search-filter-display-repos = 仓库
search-filter-display-diff-sets = 差异集
search-filter-display-static-slash-commands = 斜杠命令
search-filter-display-historical-conversations = 历史对话
search-filter-display-skills = 技能
search-filter-display-base-models = 基础模型
search-filter-display-full-terminal-use-models = 完整终端使用模型
search-filter-display-current-directory-conversations = 当前目录对话
search-results-menu-no-results = 未找到结果
search-results-menu-prompts-title = 提示词
ai-context-diffset-uncommitted-changes = 未提交的更改
ai-context-diffset-changes-vs-main-branch = 与 main 分支相比的更改
ai-context-diffset-changes-vs-branch = 与 { $branch } 相比的更改
ai-context-diffset-uncommitted-changes-description = 工作目录中所有未提交的更改
ai-context-diffset-changes-vs-main-branch-description = 与 main 分支相比的所有更改
ai-context-diffset-changes-vs-branch-description = 与 { $branch } 相比的所有更改
ai-context-code-search-failed = 代码搜索失败
ai-context-files-directory-accessibility-label = 目录：{ $path }
ai-context-files-file-accessibility-label = 文件：{ $path }
ai-context-blocks-just-now = 刚刚
ai-context-blocks-minutes-ago = { $count } 分钟前
ai-context-blocks-hours-ago = { $count } 小时前
ai-context-blocks-days-ago = { $count } 天前
ai-context-blocks-no-output = 无输出
ai-context-blocks-accessibility-label = 命令块：{ $command }

# --- ANCHOR-SUB-DRIVE-NAMING-IMPORT ---
# Drive 命名弹窗（app/src/drive/cloud_object_naming_dialog.rs）
drive-naming-notebook-name = 笔记本名称
drive-naming-folder-name = 文件夹名称
drive-naming-collection-name = 集合名称
drive-naming-create = 创建
drive-naming-cancel = 取消
drive-naming-rename = 重命名

# Drive 导入弹窗（app/src/drive/import/modal.rs, app/src/drive/import/modal_body.rs）
drive-import-title = 导入
drive-import-close = 关闭
drive-import-cancel = 取消
drive-import-preparing = 正在准备...
drive-import-choose-files = 选择文件...
drive-import-learn-file-support = 了解文件支持和格式设置
drive-import-file-upload-error = 文件上传到服务器失败
drive-import-folder-upload-error = 文件夹上传到服务器失败

# Drive 主面板和 workflow 编辑器（app/src/drive/index.rs, app/src/drive/workflows/*）
drive-title = Drive
drive-create-team-text = 与团队成员共享命令和知识。
drive-join-team-header = 与已经在 Warp 上的 { $count } 位团队成员协作。
drive-environment-variables = 环境变量
drive-folder = 文件夹
drive-notebook = 笔记本
drive-workflow = workflow
drive-prompt = 提示词
drive-import = 导入
drive-remove = 移除
drive-new-folder = 新建文件夹
drive-new-notebook = 新建笔记本
drive-new-workflow = 新建 workflow
drive-new-prompt = 新建提示词
drive-new-environment-variables = 新建环境变量
drive-offline-banner = 你已离线。部分文件将为只读。
drive-sort-by = 排序方式
drive-retry-sync = 重试同步
drive-empty-trash = 清空废纸篓
drive-trash-section-title = 废纸篓
drive-trash-title = 废纸篓
drive-trash-deletion-warning = 废纸篓中的项目将在 30 天后永久删除。
drive-team-space-zero-state = 将个人 workflow 或笔记本拖放或移动到此处，即可与团队共享。
drive-create-team-button = 创建团队
drive-view-teams-to-join = 查看可加入的团队
drive-or = 或
drive-sign-up-storage-limit = 免费注册可提升存储上限并解锁更多功能。
drive-sign-up = 注册
drive-copy-link = 复制链接
drive-collapse-all = 全部折叠
drive-revert-to-server = 恢复为服务器版本
drive-attach-to-active-session = 附加到活动会话
drive-copy-prompt = 复制提示词
drive-copy-workflow-text = 复制 workflow 文本
drive-copy-id = 复制 ID
drive-copy-variables = 复制变量
drive-load-in-subshell = 在子 shell 中加载
drive-delete-forever = 永久删除
drive-rename = 重命名
drive-retry = 重试
drive-move-to-space = 移动到 { $space }
drive-open-on-desktop = 在桌面端打开
drive-duplicate = 复制副本
drive-export = 导出
drive-trash-menu = 移到废纸篓
drive-open = 打开
drive-edit = 编辑
drive-restore = 恢复
drive-compare-plans = 比较套餐
drive-manage-billing = 管理账单
drive-object-type-notebook-plural = 笔记本
drive-object-type-workflow-plural = workflow
drive-object-type-folder-plural = 文件夹
drive-object-type-env-var-collection-plural = 环境变量集合
drive-object-type-object-plural = 对象
drive-object-type-notebooks = 笔记本
drive-object-type-workflows = workflow
drive-object-type-environment-variables = 环境变量
drive-object-type-folders = 文件夹
drive-object-type-agent-workflows = Agent workflow
drive-object-type-ai-fact = AI 事实
drive-object-type-rules = 规则
drive-object-type-mcp-server = MCP 服务器
drive-object-type-mcp-servers = MCP 服务器
drive-shared-object-limit-hit-banner-prefix = 你当前套餐中的 { $object_type } 数量已用完。
drive-shared-object-limit-hit-banner = 你当前套餐中的 { $object_type } 数量已用完。升级后可使用更多笔记本、workflow、共享会话和 AI 额度。
drive-payment-issue-banner-prefix = 由于订阅付款问题，共享对象已受限。
drive-payment-issue-banner-admin = 由于订阅付款问题，共享对象已受限。请更新付款信息以恢复访问。
drive-payment-issue-banner-admin-enterprise = 由于订阅付款问题，共享对象已受限。请联系 support@warp.dev 恢复访问。
drive-payment-issue-banner-nonadmin = 由于订阅付款问题，共享对象已受限。请联系团队管理员恢复访问。
drive-empty-trash-title = 确定要清空废纸篓吗？
drive-empty-trash-body = 此操作无法撤销。
drive-empty-trash-confirm = 是，清空废纸篓
drive-empty-trash-cancel = 取消
workflow-title-placeholder = 未命名 workflow
workflow-description-placeholder = 添加描述
workflow-title-input-placeholder = 添加标题
workflow-description-input-placeholder = 添加描述
workflow-new-argument = 新增参数
workflow-arguments-label = 参数
workflow-argument-description-placeholder = 描述
workflow-argument-value-placeholder = 值（可选）
workflow-default-value-placeholder = 默认值（可选）
workflow-agent-mode-query-placeholder = 在此输入你的提示...（例如，「创建一个按日期排序对象数组的函数」或「帮我调试这个 React 组件」）。
workflow-save = 保存 workflow
workflow-unsaved-changes = 你有未保存的更改。
workflow-keep-editing = 继续编辑
workflow-discard-changes = 放弃更改
workflow-ai-assist-autofill = 自动填充
workflow-ai-assist-loading = 加载中
workflow-ai-assist-tooltip = 使用 Warp AI 生成标题、描述或参数
workflow-tooltip-restore-from-trash = 从废纸篓恢复 workflow
workflow-ai-assist-error-byop-required = 自动填充需要 BYOP 模型。请在「设置 → AI」中配置 provider 与模型。
workflow-ai-assist-error-bad-command = 无法生成元数据。请换一个命令后重试。
workflow-ai-assist-error-generic = 出错了。请重试。
workflow-ai-assist-error-rate-limited = 看起来你的 AI 额度已用完。请稍后重试。
workflow-enum-new = 新建
workflow-alias-name-placeholder = 别名
workflow-add-argument-tooltip = 添加工作流参数

# --- ANCHOR-SUB-SETTINGS-PRIVACY-ADD-REGEX ---
# 隐私设置添加正则表达式弹窗（app/src/settings_view/privacy/add_regex_modal.rs）
settings-privacy-add-regex-name-placeholder = 例如「Google API Key」
settings-privacy-add-regex-name-label = 名称（可选）
settings-privacy-add-regex-pattern-label = 正则表达式模式
settings-privacy-add-regex-invalid = 无效的正则表达式
settings-privacy-add-regex-cancel = 取消

# 工作区面板（app/src/workspace/view/*）
workspace-conversation-list-search = 搜索
workspace-conversation-list-active = 当前
workspace-conversation-list-past = 历史
workspace-conversation-list-view-all = 查看全部
workspace-conversation-list-show-less = 收起
workspace-conversation-list-empty-title = 暂无对话
workspace-conversation-list-empty-description = 你与本地和环境智能体的当前及历史对话会显示在这里。
workspace-conversation-list-new-conversation = 新建对话
conversation-untitled = 未命名对话
conversation-deleted = 已删除的对话
workspace-conversation-list-no-matching = 没有匹配的对话
workspace-conversation-list-delete = 删除
workspace-conversation-list-delete-in-progress-error = 对话进行中时无法删除。
workspace-conversation-list-delete-ambient-tooltip = 无法删除环境智能体对话
workspace-conversation-list-fork-new-pane = 在新窗格中派生
workspace-conversation-list-fork-new-tab = 在新标签页中派生
workspace-conversation-list-fallback-title = 对话
command-palette-conversations-active-pane = 当前窗格中的对话
command-palette-conversations-other-active = 其他活动对话
command-palette-conversations-past = 历史对话
command-palette-conversations-fork-current = 派生当前对话
command-palette-conversations-fork-current-with-title = 派生当前对话（{ $title }）
command-palette-conversations-a11y-navigate = 按 Enter 导航到对话
command-palette-conversations-a11y-fork = 按 Enter 将当前对话派生为新对话。
command-palette-conversations-a11y-new = 按 Enter 创建新对话。
workspace-left-panel-project-explorer = 项目浏览器
project-explorer-unavailable-title = 项目浏览器不可用
project-explorer-unavailable-disabled-description = 项目浏览器需要访问本地工作区。请打开新会话或切换到活动会话后查看。
project-explorer-unavailable-remote-description = 项目浏览器需要访问本地工作区,远程会话暂不支持。
project-explorer-unavailable-wsl-description = 项目浏览器当前不支持 WSL。
workspace-left-panel-global-search = 全局搜索
workspace-left-panel-warp-drive = Warp Drive
workspace-left-panel-agent-conversations = 智能体对话
workspace-left-panel-ssh-manager = SSH 管理器
workspace-left-panel-skill-manager = Skill 管理器
skill-manager-search-placeholder = 搜索 skill
skill-manager-filter-all = 全部
skill-manager-filter-provider = 来源
skill-manager-meta-default = 默认
skill-manager-meta-duplicate = 重复
skill-manager-empty = 当前过滤条件下没有 skill。
skill-manager-preview-empty = 选择一个 skill 预览 SKILL.md。
workspace-left-panel-ssh-manager-placeholder = SSH 管理器 — 功能开发中
workspace-left-panel-ssh-manager-detail-empty = 选择左侧的服务器以查看详情。
workspace-left-panel-ssh-manager-detail-host = 主机
workspace-left-panel-ssh-manager-detail-port = 端口
workspace-left-panel-ssh-manager-detail-user = 用户
workspace-left-panel-ssh-manager-detail-auth = 认证方式
workspace-left-panel-ssh-manager-detail-key-path = 私钥路径
workspace-left-panel-ssh-manager-auth-password = 密码
workspace-left-panel-ssh-manager-auth-key = 私钥
workspace-left-panel-ssh-manager-menu-new-folder = 新建文件夹
workspace-left-panel-ssh-manager-menu-new-server = 新建 SSH 服务器
workspace-left-panel-ssh-manager-menu-edit = 编辑
workspace-left-panel-ssh-manager-menu-connect = 连接
workspace-left-panel-ssh-manager-menu-delete = 删除
workspace-left-panel-ssh-manager-pane-hint = 字段编辑与「连接」按钮将在下一迭代加入。目前此面板仅展示已保存的配置。
workspace-left-panel-ssh-manager-pane-folder-body = 文件夹。在此文件夹内选择某个服务器查看详情;右键此节点可以新建/删除。
workspace-left-panel-ssh-manager-server-missing = 找不到该服务器。可能已经从其他窗口被删除。
workspace-left-panel-ssh-manager-field-name = 名称
workspace-left-panel-ssh-manager-passphrase = 私钥口令
workspace-left-panel-ssh-manager-save = 保存
workspace-left-panel-ssh-manager-status-saved = 已保存。
workspace-left-panel-ssh-manager-error-name-required = 名称不能为空。
workspace-left-panel-ssh-manager-error-port-invalid = 端口必须是 1 到 65535 之间的数字。
workspace-left-panel-ssh-manager-error-host-required = 主机不能为空。
workspace-left-panel-ssh-manager-connect = 连接
search-filter-placeholder-ssh-servers = 搜索 SSH 服务器…
search-filter-display-ssh-servers = SSH 服务器
workspace-left-panel-ssh-manager-menu-rename = 重命名
workspace-left-panel-ssh-manager-tree-empty = 还没有 SSH 服务器。点击 📁 新建文件夹，+ 新建服务器。
workspace-left-panel-close-panel = 关闭面板
workspace-tabs-panel-tooltip = 标签页面板
workspace-tools-panel-tooltip = 工具面板
workspace-agent-management-panel-tooltip = 智能体管理面板
workspace-code-review-panel-tooltip = 代码评审面板
workspace-notifications-tooltip = 通知
workspace-new-tab-tooltip = 新建标签页
workspace-tab-configs-tooltip = 标签页配置
workspace-offline-tooltip = 离线时部分功能可能不可用
workspace-right-panel-open-repository = 打开仓库
workspace-right-panel-open-repository-tooltip = 导航到仓库并将其初始化用于编码
workspace-right-panel-close-panel = 关闭面板
workspace-right-panel-code-review = 代码评审
workspace-right-panel-minimize = 最小化
workspace-right-panel-maximize = 最大化
workspace-right-panel-unknown = 未知
terminal-pane-new-cloud-agent-title = 新建智能体
terminal-pane-new-agent-conversation-title = 新建智能体对话
vertical-tabs-no-tabs-open = 没有打开的标签页
vertical-tabs-untitled-tab = 未命名标签页
vertical-tabs-view-options-tooltip = 查看选项
vertical-tabs-new-session = 新建会话
vertical-tabs-terminal-kind-oz = Oz
vertical-tabs-pane-kind-terminal = 终端
vertical-tabs-pane-kind-code = 代码
vertical-tabs-pane-kind-code-diff = 代码差异
vertical-tabs-pane-kind-file = 文件
vertical-tabs-pane-kind-notebook = Notebook
vertical-tabs-pane-kind-workflow = 工作流
vertical-tabs-pane-kind-environment-variables = 环境变量
vertical-tabs-pane-kind-environments = 环境
vertical-tabs-pane-kind-rules = 规则
vertical-tabs-pane-kind-plan = 计划
vertical-tabs-pane-kind-execution-profile = 执行配置
vertical-tabs-pane-kind-other = 其他
vertical-tabs-setting-view-as = 显示为
vertical-tabs-setting-panes = 窗格
vertical-tabs-setting-tabs = 标签页
vertical-tabs-setting-tab-item = 标签项
vertical-tabs-setting-focused-session = 当前会话
vertical-tabs-setting-summary = 摘要
vertical-tabs-setting-density = 密度
vertical-tabs-setting-pane-title-as = 窗格标题显示为
vertical-tabs-setting-command-conversation = 命令 / 对话
vertical-tabs-setting-working-directory = 工作目录
vertical-tabs-setting-branch = 分支
vertical-tabs-setting-additional-metadata = 附加元数据
vertical-tabs-setting-show = 显示
vertical-tabs-setting-pr-link-requires-gh = 需要安装并认证 GitHub CLI
vertical-tabs-setting-pr-link = PR 链接
vertical-tabs-setting-diff-stats = Diff 统计
vertical-tabs-setting-show-details-on-hover = 悬停时显示详情
global-search-placeholder = 在文件中搜索
global-search-toggle-case-sensitivity = 切换大小写敏感
global-search-toggle-regex = 切换正则表达式
global-search-label = 搜索
global-search-no-results-gitignore = 未找到结果。请检查你的 gitignore 文件。
global-search-result-count-one = 1 个结果，位于 { $files } 个文件中
global-search-result-count-many = { $n } 个结果，位于 { $files } 个文件中
global-search-subset-warning = 结果集仅包含所有匹配项的一部分。请使用更具体的搜索词来缩小结果范围。
global-search-title = 全局搜索
global-search-description = 在当前目录的文件中搜索。
global-search-unavailable-title = 全局搜索不可用
global-search-unavailable-description = 全局搜索需要访问本地工作区。请打开新会话或切换到活动会话后查看。
global-search-remote-description = 全局搜索需要访问本地工作区，远程会话不支持此功能
global-search-unsupported-session-description = 全局搜索目前不支持 Git Bash 或 WSL。
global-search-failed = 全局搜索失败。

# Wasm NUX dialog (app/src/wasm_nux_dialog.rs)
wasm-nux-open-desktop-title = 在 Warp 桌面版中打开？
wasm-nux-open-desktop-detail = 之后的链接会自动在桌面版中打开。
wasm-nux-open-desktop-confirm = 在 Warp 中打开
wasm-nux-download-title = 下载 Warp 桌面版？
wasm-nux-download-description = Warp 是一款内置 AI 和团队知识的智能终端。
wasm-nux-learn-more = 了解更多
wasm-nux-download-confirm = 下载
wasm-nux-object-kind-drive-objects = Warp Drive 对象
wasm-nux-object-kind-shared-sessions = 共享会话
wasm-nux-object-kind-warp-links = Warp 链接
wasm-nux-always-open-on-web-title = 始终在网页中打开 { $object_kind } 吗？
wasm-nux-always-open-on-web-detail = 你可以随时在设置中更改此项。
wasm-nux-yes = 是

# Drive cloud action confirmation dialog (app/src/drive/cloud_action_confirmation_dialog.rs)
drive-cloud-action-cancel = 取消
drive-cloud-action-delete-team-title = 你确定要删除这个团队吗？
drive-cloud-action-leave-team-title = 你确定要离开这个团队吗？
drive-cloud-action-delete-team-body = 删除这个团队会永久移除该团队及其所有相关内容，包括账单信息或额度。删除后无法恢复。
drive-cloud-action-leave-team-body = 如需重新加入，你需要再次被邀请。
drive-cloud-action-delete-team-confirm = 是，删除
drive-cloud-action-leave-team-confirm = 是，离开

# Auth override warning (app/src/auth/auth_override_warning_body.rs)
auth-override-warning-title = 检测到新的登录
auth-override-warning-confirm-title = 要删除个人 Warp Drive 对象和偏好吗？
auth-override-warning-description = 看起来你刚刚通过网页浏览器登录了一个 Warp 账号。如果继续，当前匿名会话中的所有个人 Warp Drive 对象和偏好都将被永久删除。
auth-override-warning-cannot-undo = 此操作无法撤销。
auth-override-warning-export = 导出你的数据
auth-override-warning-export-description = ，稍后可再导入。
auth-override-warning-cancel = 取消
auth-override-warning-continue = 继续
auth-override-warning-accessibility-help = Warp 检测到你通过网页浏览器进行了新的登录。按 Esc 键取消，并继续在未登录状态下使用 Warp。

# Auth SSO link/login failures/paste token/logout/offline/privacy
auth-needs-sso-link-button = 关联 SSO
auth-needs-sso-link-title = 你的组织已为该账号启用 SSO
auth-needs-sso-link-detail = 点击下方按钮，将你的 Warp 账号关联到你的 SSO 提供商。
auth-login-failure-troubleshooting-prefix =  这已经不是第一次了？请查看我们的
auth-login-failure-troubleshooting-link = 故障排查文档
auth-login-failure-troubleshooting-suffix = 。
auth-login-failure-invalid-token = 你在弹窗中输入了无效的认证令牌。
auth-login-failure-copy-token-manually = 登录失败。请尝试从认证网页手动复制认证令牌并粘贴到弹窗中。
auth-login-failure-login-request = 登录请求失败。
auth-login-failure-signup-request = 注册请求失败。
auth-login-failure-wrong-redirect-url = 你粘贴的重定向 URL 不是从当前应用发起的。请点击下方按钮重试。
auth-paste-token-placeholder = 输入认证令牌
auth-paste-token-title = 在下方粘贴你的认证令牌
auth-paste-token-detail = 从浏览器粘贴你的认证令牌，以完成登录。
auth-paste-token-cancel = 取消
auth-paste-token-continue = 继续
auth-offline-first-use-description = 你当前处于离线状态。首次使用 Warp 时需要联网。
auth-offline-first-use-learn-more = 了解更多
auth-offline-overlay-title = 离线使用 Warp
auth-offline-overlay-paragraph-1 = Warp 的所有非云端功能都可以离线使用。
auth-offline-overlay-paragraph-2 = 但在首次使用 Warp 时，我们要求你保持联网，以启用 Warp 的 AI 和云功能。
auth-offline-overlay-paragraph-3 = 我们向所有用户提供云功能，因此需要互联网连接来计量 AI 使用量、防止滥用，并将云对象与用户关联起来。如果你选择在退出登录状态下使用 Warp，系统会把一个唯一 ID 绑定到匿名用户账号上，以支持这些功能。
auth-offline-overlay-dismiss = 关闭
auth-privacy-settings-title = 隐私设置
auth-privacy-settings-done = 完成
auth-privacy-settings-help-improve = 帮助改进 Warp
auth-privacy-settings-help-improve-description = 高层级的功能使用数据可以帮助 Warp 产品团队确定路线图优先级。
auth-privacy-settings-learn-more = 了解更多
auth-privacy-settings-send-crash-reports = 发送崩溃报告
auth-privacy-settings-crash-reports-description = 崩溃报告能帮助 Warp 工程团队了解稳定性情况并改进性能。
auth-logout-confirm = 是，退出登录
auth-logout-show-running-processes = 显示正在运行的进程
auth-logout-cancel = 取消
auth-logout-title = 要退出登录吗？
auth-logout-running-processes-warning = 你有 { $count } 个正在运行的进程。
auth-logout-shared-sessions-warning = 你有 { $count } 个共享会话。
auth-logout-unsynced-drive-objects-warning = 你有 { $count } 个未同步的 Warp Drive 对象。退出登录后将失去这些对象。
auth-logout-unsaved-files-warning = 你有 { $count } 个未保存的文件。退出登录后将失去这些文件。

# CLI agent plugin instructions
cli-agent-plugin-run-on-remote = 请确认在远程机器上运行这些命令。
cli-agent-plugin-codex-install-title = 为 Codex 启用 Warp 通知
cli-agent-plugin-codex-install-subtitle = 将 Codex 更新到最新版本，然后启用聚焦时通知，这样你工作时 Warp 就能显示通知。
cli-agent-plugin-codex-update-step = 将 Codex 更新到最新版本。
cli-agent-plugin-codex-notification-step = 在 Codex 配置中将通知条件设为 "always"。打开或创建 ~/.codex/config.toml 并添加：
cli-agent-plugin-codex-restart-note = 重启 Codex 以应用更改。
cli-agent-plugin-deepseek-install-title = 为 DeepSeek 启用 Warp 通知
cli-agent-plugin-deepseek-install-subtitle = 在 DeepSeek 配置文件（~/.deepseek/config.toml）中添加以下内容，以启用对话完成通知。
cli-agent-plugin-deepseek-notification-step = 在 ~/.deepseek/config.toml 中将通知条件设为 "always"：
cli-agent-plugin-deepseek-restart-note = 重启 DeepSeek 以应用更改。
cli-agent-plugin-claude-install-title = 安装 Claude Code 的 Warp 插件
cli-agent-plugin-claude-install-subtitle = 确保你的机器已安装 jq，然后运行这些命令。
cli-agent-plugin-claude-add-marketplace-step = 添加 Warp 插件市场仓库
cli-agent-plugin-install-warp-plugin-step = 安装 Warp 插件
cli-agent-plugin-claude-restart-note = 重启 Claude Code 以启用插件。
cli-agent-plugin-claude-known-issues-note = Claude Code 的插件系统存在一些已知问题。如果第 1 步后找不到插件，可以尝试手动向 ~/.claude/settings.json 添加 "extraKnownMarketplaces" 条目。
cli-agent-plugin-claude-update-title = 更新 Claude Code 的 Warp 插件
cli-agent-plugin-run-following-commands = 运行以下命令。
cli-agent-plugin-remove-existing-marketplace-step = 移除已有市场仓库（如果存在）
cli-agent-plugin-readd-marketplace-step = 重新添加市场仓库
cli-agent-plugin-install-latest-version-step = 安装最新插件版本
cli-agent-plugin-claude-restart-update-note = 重启 Claude Code 以启用更新。
cli-agent-plugin-gemini-install-title = 安装 Gemini CLI 的 Warp 插件
cli-agent-plugin-gemini-run-command-restart = 运行以下命令，然后重启 Gemini CLI。
cli-agent-plugin-install-warp-extension-step = 安装 Warp 扩展
cli-agent-plugin-gemini-restart-note = 重启 Gemini CLI 以启用插件。
cli-agent-plugin-gemini-update-title = 更新 Gemini CLI 的 Warp 插件
cli-agent-plugin-update-warp-extension-step = 更新 Warp 扩展
cli-agent-plugin-gemini-restart-update-note = 重启 Gemini CLI 以启用更新。
cli-agent-plugin-opencode-install-title = 安装 OpenCode 的 Warp 插件
cli-agent-plugin-opencode-install-subtitle = 将 Warp 插件加入 OpenCode 配置，然后重启 OpenCode。
cli-agent-plugin-opencode-open-config-step = 打开或创建你的 opencode.json。它可以位于项目根目录，也可以位于全局配置路径：
cli-agent-plugin-opencode-add-plugin-step = 将 "@warp-dot-dev/opencode-warp" 添加到顶层 JSON 对象的 "plugin" 数组中：
cli-agent-plugin-opencode-restart-note = 重启 OpenCode 以启用插件。
cli-agent-plugin-opencode-update-title = 更新 OpenCode 的 Warp 插件
cli-agent-plugin-opencode-update-subtitle = 在 opencode.json 中将插件固定到最新版本。OpenCode 会按版本规格缓存插件，修改固定版本会强制它在重启时重新拉取。
cli-agent-plugin-opencode-replace-plugin-step = 将 "plugin" 数组中现有的 "@warp-dot-dev/opencode-warp" 条目替换为显式版本：
cli-agent-plugin-opencode-restart-update-note = 重启 OpenCode 以加载更新后的插件。

# Remaining visible UI strings
ai-ask-user-questions-unavailable = 问题不可用
ai-ask-user-questions-skipped-auto-approve = 已因自动批准跳过问题
terminal-bootstrapping-checking = 正在检查...
terminal-bootstrapping-installing-progress = 正在安装...（{ $p }%）
terminal-bootstrapping-installing = 正在安装...
terminal-bootstrapping-updating = 正在更新...
terminal-bootstrapping-initializing = 正在初始化...
terminal-bootstrapping-installing-warp-ssh-extension-progress = 正在安装 Warp SSH 扩展...（{ $p }%）
terminal-bootstrapping-installing-warp-ssh-extension = 正在安装 Warp SSH 扩展...
terminal-bootstrapping-updating-warp-ssh-extension = 正在更新 Warp SSH 扩展...
terminal-bootstrapping-starting-shell-name = 正在启动 { $shell }...
agent-tip-prefix = 提示：
agent-tip-slash-menu = 输入 `/` 打开斜杠菜单并访问快捷智能体操作。
agent-tip-toggle-input-mode = 按 <keybinding> 切换自然语言检测，在智能体输入和终端输入之间切换。
agent-tip-plan = 输入 `/plan` <prompt>，先为智能体创建计划再执行。
agent-tip-command-palette = 按 <keybinding> 打开命令面板，访问 Warp 操作和快捷方式。
agent-tip-warp-drive = 将可复用的工作流、Notebook 和提示词存入你的
agent-tip-redirect-running-agent = 输入新的提示词即可在智能体运行时重定向它。
agent-tip-add-context = 输入 `@` 将文件、块或 Warp Drive 对象作为上下文添加到提示词。
agent-tip-attach-prior-output = 按 <keybinding> 将上一条命令输出作为智能体上下文附加。
agent-tip-init-index = 输入 `/init` 索引仓库，让智能体理解你的代码库。
agent-tip-agent-profiles = 添加智能体配置，按会话自定义权限和模型。
agent-tip-fork-block = 右键点击块，可从该位置 fork 对话。
agent-tip-copy-output = 右键点击块，可复制对话输出。
agent-tip-drag-image = 将图片拖入面板，可作为智能体上下文附加。
agent-tip-interactive-tools = 可以让智能体控制 node、python、postgres、gdb 或 vim 等交互式工具。
agent-tip-code-review-panel = 按 <keybinding> 打开代码审查面板并审查智能体的改动。
agent-tip-add-mcp = 输入 `/add-mcp` 将 MCP 服务器添加到工作区。
agent-tip-open-mcp-servers = 输入 `/open-mcp-servers` 查看并与团队共享 MCP 服务器。
agent-tip-create-environment = 输入 `/create-environment` 将仓库转换为智能体可运行的远程 docker 环境。
agent-tip-add-prompt = 输入 `/add-prompt` 创建可复用提示词，用于重复工作流。
agent-tip-add-rule = 输入 `/add-rule` 创建全局智能体规则。
agent-tip-fork = 输入 `/fork` 创建当前对话的新副本，也可以附带新提示词。
agent-tip-open-code-review = 输入 `/open-code-review` 打开代码审查面板并检查智能体生成的 diff。
agent-tip-new-conversation = 输入 `/new` 用干净上下文开始新的智能体对话。
agent-tip-compact = 输入 `/compact` 总结当前对话，释放上下文窗口空间。
agent-tip-usage = 输入 `/usage` 查看当前 AI credits 用量。
agent-tip-oz-headless = 使用 `oz` 命令以 headless 模式运行 Oz 智能体，适合远程机器。
agent-tip-selected-text-context = 右键点击选中文本，可作为智能体上下文附加。
agent-tip-project-rules = 使用 `AGENTS.md` 或 `CLAUDE.md` 应用项目级规则。
agent-tip-url-context = 粘贴 URL 即可将网页作为智能体上下文附加。
agent-tip-warpify-ssh = 对远程 SSH 会话执行 Warpify，即可在该环境中启用 Oz。
agent-tip-switch-profiles = 切换智能体配置，快速更改模型和智能体权限。
agent-tip-init-rules = 输入 `/init` 生成 `WARP.md` 文件并为智能体定义项目规则。
agent-tip-auto-approve = 按 <keybinding> 可在本会话剩余时间内自动批准智能体命令和 diff。
agent-tip-desktop-notifications = 启用桌面通知，当智能体需要你处理时收到提醒。
agent-tip-cancel-task = 按 <keybinding> 取消当前智能体任务。
agent-tip-action-open-palette = 打开命令面板
agent-tip-action-warp-drive = Warp Drive。
agent-tip-action-show-diff-view = 显示 diff 视图
agent-tip-voice-input = 按住 <keybinding>，直接用语音向智能体输入提示词。
hoa-welcome-banner-title = 引入通用智能体支持：用 Warp 增强任何编码智能体
hoa-feature-vertical-tabs-title = 垂直标签页
hoa-feature-vertical-tabs-description = 丰富的标签标题和元数据，如 git 分支、worktree 和 PR。完全可自定义。
hoa-feature-tab-configs-title = 标签页配置
hoa-feature-tab-configs-description = 标签页级 schema，一键设置目录、启动命令、主题和 worktree
hoa-feature-agent-inbox-title = 智能体收件箱
hoa-feature-agent-inbox-description = 任一智能体需要你处理时收到通知，也可在集中收件箱中访问
hoa-feature-native-code-review-title = 原生代码审查
hoa-feature-native-code-review-description = 将 Warp 代码审查中的行内评论直接发送到 Claude Code、Codex 或 OpenCode
resource-center-whats-new-section = 新变化
resource-center-getting-started-section = 入门
resource-center-maximize-warp-section = 充分使用 Warp
resource-center-advanced-setup-section = 高级设置
resource-center-create-first-block-title = 创建第一个块
resource-center-create-first-block-description = 运行命令，查看命令和输出如何分组。
resource-center-navigate-blocks-title = 导航块
resource-center-navigate-blocks-description = 点击选择块，并使用方向键导航。
resource-center-block-action-title = 对块执行操作
resource-center-block-action-description = 右键点击块即可复制/粘贴、共享或查看更多操作。
resource-center-command-palette-title = 打开命令面板
resource-center-command-palette-description = 通过键盘访问 Warp 的全部功能。
resource-center-set-theme-title = 设置主题
resource-center-set-theme-description = 选择主题，让 Warp 符合你的偏好。
resource-center-custom-prompt-title = 使用自定义提示符
resource-center-custom-prompt-description = 设置 Warp 以遵循你的 PS1 设置
resource-center-view-documentation = 查看文档
resource-center-integrate-ide-title = 将 Warp 与 IDE 集成
resource-center-integrate-ide-description = 配置 Warp，从你最常用的开发工具中启动
resource-center-how-warp-uses-warp-title = Warp 如何使用 Warp
resource-center-how-warp-uses-warp-description = 了解 Warp 工程团队如何使用他们最喜欢的功能
resource-center-read-article = 阅读文章
resource-center-command-search-title = 命令搜索
resource-center-command-search-description = 查找并运行之前执行过的命令、工作流等。
resource-center-ai-command-search-title = AI 命令搜索
resource-center-ai-command-search-description = 用自然语言生成 shell 命令。
resource-center-split-panes-title = 拆分面板
resource-center-split-panes-description = 将标签页拆分为多个面板，组成理想布局。
resource-center-launch-configuration-title = 启动配置
resource-center-launch-configuration-description = 保存当前窗口、标签页和面板配置。
notebook-link-new-session = 新会话
notebook-link-new-session-tooltip = 在此目录中打开新的终端会话
notebook-link-open-terminal-session = 在终端会话中打开
notebook-link-open-in-editor = 在编辑器中打开
notebook-link-edit-markdown-file = 编辑 Markdown 文件
auth-token-placeholder = Auth Token
sharing-inherited-from-prefix = 继承自
sharing-inherited-permission-label = 继承的权限
sharing-inherited-permissions-edit-parent-tooltip = 在父文件夹上编辑继承权限
sharing-inherited-permissions-cannot-edit-tooltip = 无法编辑继承权限
command-palette-navigation-running = 正在运行...
command-palette-navigation-completed-over-hour = 1 小时前已完成
command-palette-navigation-completed-minute-ago = { $mins } 分钟前已完成
command-palette-navigation-completed-minutes-ago = { $mins } 分钟前已完成
command-palette-navigation-no-timestamp = 未找到时间戳
command-palette-navigation-completed = 已完成
command-palette-navigation-empty-session = 空会话
terminal-history-tab-commands = 命令
terminal-history-tab-prompts = 提示词
common-current = 当前
auth-browser-token-placeholder = 浏览器认证 token
requested-script-expand-to-show = 展开以显示脚本
common-hide = 隐藏
terminal-message-new-conversation = {" "}新建对话
agent-message-bar-again-send-to-agent = 再次发送给智能体

# =============================================================================
# SECTION: remaining-ui-surfaces (Owner: codex-i18n-remaining-ui-surfaces)
# =============================================================================

onboarding-intention-title = 欢迎使用 Warp
onboarding-intention-subtitle = 你想如何工作？
onboarding-intention-agent-title = 使用 AI 智能体更快构建
onboarding-intention-agent-description = 以智能体优先的体验，同时保留一流终端能力。可使用这些终端与智能体驱动开发能力：
onboarding-intention-terminal-title = 只使用终端
onboarding-intention-terminal-badge = 不启用 AI 功能
onboarding-intention-terminal-description = 现代化终端，专注速度、上下文和控制，不启用 AI。
onboarding-ai-feature-warp-agents = Warp 智能体
onboarding-ai-feature-oz-cloud-agents-platform = Oz 本地智能体平台
onboarding-ai-feature-next-command-predictions = 下一条命令预测
onboarding-ai-feature-prompt-suggestions = 提示词建议
onboarding-ai-feature-remote-control-agents = 通过 Claude Code、Codex 等智能体进行远程控制
onboarding-ai-feature-agents-over-ssh = SSH 上的智能体
onboarding-agent-title = 自定义你的 Warp 智能体
onboarding-agent-subtitle = 选择应用内智能体的默认设置。
onboarding-agent-default-model = 默认模型
onboarding-agent-autonomy = 自主程度
onboarding-agent-set-by-team-workspace = 由团队工作区设置
onboarding-agent-team-workspace-autonomy-description = 自主程度设置由你的团队工作区统一配置。
onboarding-agent-autonomy-full-title = 完全
onboarding-agent-autonomy-full-subtitle = 无需询问即可运行命令、编写代码和读取文件。
onboarding-agent-autonomy-partial-title = 部分
onboarding-agent-autonomy-partial-subtitle = 可以规划、读取文件并执行低风险命令；在改动文件或执行敏感命令前会询问。
onboarding-agent-autonomy-none-title = 无
onboarding-agent-autonomy-none-subtitle = 未经你批准不会执行任何操作。
onboarding-agent-disable-warp-agent = 禁用 Warp 智能体
onboarding-agent-upgrade-title = 升级以使用高级模型。
onboarding-agent-upgrade-subtitle = 前沿模型需要付费套餐。
onboarding-agent-paste-token-link = 点击这里
onboarding-agent-open-page-manually = {" "}并手动打开页面。{" "}
onboarding-agent-paste-token-suffix = {" "}以粘贴浏览器中的 token。
onboarding-agent-plan-activated = 套餐已成功激活。所有高级模型均可使用。
onboarding-project-title = 打开项目
onboarding-project-subtitle = 设置项目，让 Warp 更适合编码。
onboarding-project-open-local-folder = 打开本地文件夹
onboarding-project-initialize-automatically = 自动初始化项目
onboarding-project-initialize-description = 准备项目环境、构建代码索引并生成项目规则，让智能体理解更深入、表现更好。
onboarding-intro-already-have-account = 已有账号？{" "}
onboarding-intro-subtitle = 内置先进智能体的现代终端。
onboarding-get-started = 开始使用
onboarding-theme-title = 选择主题
onboarding-theme-subtitle = 点击或使用方向键选择，按 Enter 确认。
onboarding-theme-sync-with-os = 跟随系统切换浅色/深色主题
onboarding-third-party-title = 自定义第三方智能体
onboarding-third-party-subtitle = 为 Claude Code、Codex、Gemini 等智能体设置默认行为。
onboarding-third-party-cli-toolbar = CLI 智能体工具栏
onboarding-third-party-notifications = 通知
onboarding-customize-title = 自定义你的 Warp
onboarding-customize-subtitle = 根据你的工作方式定制功能和界面。
onboarding-customize-tab-styling = 标签页样式
onboarding-customize-vertical = 垂直
onboarding-customize-horizontal = 水平
onboarding-customize-conversation-history = 对话历史
onboarding-customize-file-explorer = 文件浏览器
onboarding-customize-global-file-search = 全局文件搜索
onboarding-customize-warp-drive = Warp Drive
onboarding-customize-tools-panel = 工具面板
onboarding-customize-code-review = 代码评审
onboarding-free-user-title = 开始使用吧。
onboarding-free-user-agent-title = 使用 Warp 内置智能体进行开发
onboarding-free-user-agent-description = 使用 Oz（Warp 内置智能体）迭代、规划和构建。可在本地或云端运行。
onboarding-free-user-terminal-title = 搭配第三方智能体的经典终端
onboarding-free-user-terminal-description = 现代终端，支持第三方智能体（Claude Code、Codex、Gemini CLI）和经典终端工作流。
onboarding-free-user-subscribe-title = 订阅后即可使用 Warp 的智能体开发能力。
onboarding-free-user-subscribe-item-credits = 每月 1,500 点额度
onboarding-free-user-subscribe-item-models = 使用 OpenAI、Anthropic 和 Google 的前沿模型
onboarding-free-user-subscribe-item-reload = 使用 Reload 额度和按量折扣
onboarding-free-user-subscribe-item-cloud-agents = 更长的智能体访问权限
onboarding-free-user-subscribe-item-indexing = 最高代码库索引额度
onboarding-free-user-subscribe-item-drive = 不限量 Warp Drive 对象和协作
onboarding-free-user-subscribe-item-support = 私人邮件支持
onboarding-free-user-subscribe-item-cloud-storage = 不限量云端对话存储

auth-opt-out-line-1 = 如果你想停用分析和 AI 功能，
auth-opt-out-line-2-prefix = 可以调整你的
auth-privacy-settings-prefix = 如果你想停用分析，可以调整你的{" "}
auth-privacy-settings-ai-prefix = 如果你想停用分析和 AI 功能，可以调整你的{" "}
auth-privacy-settings = 隐私设置
auth-terms-prefix = 继续即表示你同意 Warp 的{" "}
auth-terms-of-service = 服务条款
auth-log-in = 登录
auth-paste-token-from-browser = 点击这里粘贴浏览器中的 token
auth-login-slide-title-warp-drive = 开始使用 Warp Drive
auth-login-slide-title-ai = 开始使用 AI
auth-login-slide-subtitle-warp-drive = 连接你的账号，以便跨设备保存和分享笔记、工作流等内容。
auth-login-slide-subtitle-ai = 连接你的账号，以启用 AI 驱动的规划、编码和自动化。
auth-disable-warp-drive = 停用 Warp Drive
auth-disable-ai-features = 停用 AI 功能
auth-enable-warp-drive = 启用 Warp Drive
auth-enable-ai-features = 启用 AI 功能
auth-browser-sign-in-one-line-title = 在浏览器中登录以继续
auth-open-page-manually-line-prefix = {" "}并打开
auth-open-page-manually-line-suffix = 手动打开页面。
auth-disable-warp-drive-confirm-title = 确定要停用 Warp Drive 吗？
auth-disable-ai-features-confirm-title = 确定要停用 AI 功能吗？
auth-disable-warp-drive-confirm-body = Warp Drive 可让你跨设备保存工作流和知识，并与团队分享。继续后，你将无法使用以下功能：
auth-disable-ai-features-confirm-body = 开启 AI 后 Warp 会更强大。继续后，你将无法使用以下功能：
auth-feature-session-sharing = 会话分享
auth-sign-up = 注册
auth-sign-in = 登录
auth-already-have-account = 已有账号？{" "}
auth-dont-want-sign-in-now = 现在不想登录？{" "}
auth-skip-for-now = 暂时跳过
auth-skip-login-confirm-title = 确定要跳过登录吗？
auth-skip-login-confirm-line-1 = 你可以稍后注册，但部分功能（例如 AI）
auth-skip-login-confirm-line-2-prefix = 仅登录用户可用。{" "}
auth-yes-skip-login = 是，跳过登录
auth-require-login-ai-collaboration = 要使用 Warp 的 AI 功能或与他人协作，请创建账号。
auth-require-login-drive-limit = 要在 Warp Drive 中创建更多对象，请创建账号。
auth-require-login-share = 要分享内容，请创建账号。
auth-welcome-title = 欢迎使用 Warp！
auth-sign-up-for-warp = 注册 Warp
auth-browser-sign-in-title = 在浏览器中登录\n以继续
auth-browser-not-launched-prefix = 如果浏览器没有打开，{" "}
auth-copy-url = 复制 URL
auth-open-page-manually-suffix = 然后手动打开页面。

voice-try-input = 试用语音输入
voice-input-enabled-toast = 语音输入已启用。你也可以按住 `{ $key }` 键激活语音输入（可在 设置 > AI > 语音 中配置）
voice-input-microphone-access-error = 无法启动语音输入（可能需要启用麦克风访问权限）
voice-transcription-disabled-microphone = 由于未授予麦克风访问权限，语音转写已禁用。
voice-transcription = 语音转写
voice-transcription-hold-key = 语音转写（按住 `{ $key }` 键）

get-started-welcome-title = 欢迎使用 Warp
get-started-subtitle = 智能体开发环境
theme-creator-theme-name = 主题名称
theme-creator-background-color = 背景色
theme-creator-image-subheader = 根据图片（.png、.jpg）中提取的颜色自动生成主题。
theme-creator-select-image = 选择图片
theme-creator-selecting-image = 正在选择图片...
theme-creator-select-new-image = 选择新图片
theme-creator-create-theme = 创建主题
theme-creator-process-image-failed = 无法处理所选图片。请换一张图片后重试。
theme-chooser-current-description = 更改当前主题。
theme-chooser-light-description = 选择系统处于浅色模式时使用的主题。
theme-chooser-dark-description = 选择系统处于深色模式时使用的主题。
theme-chooser-no-matching-themes = 没有匹配的主题！
resource-center-keyboard-shortcuts = 键盘快捷键
resource-center-keybindings-essentials = 基础
resource-center-keybindings-blocks = 命令块
resource-center-keybindings-input-editor = 输入编辑器
resource-center-keybindings-terminal = 终端
resource-center-keybindings-fundamentals = 基本操作

launch-config-save-success-prefix = 已成功保存到
launch-config-save-failure-already-exists = 保存失败。已存在同名启动配置。
launch-config-save-failure-other = 保存时遇到问题。
launch-config-save-configuration = 保存配置
launch-config-open-yaml-file = 打开 YAML 文件
launch-config-save-current-configuration = 保存当前配置
launch-config-link-to-documentation = 文档链接
launch-config-save-modal-a11y-title = 保存配置弹窗
launch-config-save-modal-a11y-description = 输入要保存当前窗口、标签页和面板配置的文件名。按 Enter 保存启动配置，按 Esc 退出保存配置弹窗。
launch-config-save-description-no-keybinding = 这会将当前窗口、标签页和面板配置保存到文件中，方便之后再次打开。
launch-config-save-description-with-keybinding = 这会将当前窗口、标签页和面板配置保存到文件中，之后可通过 { $keybinding } 再次打开。
launch-config-yaml-saved-to-prefix = \nYAML 文件会保存到
notebook-file-could-not-read = 无法读取 { $name }
notebook-file-loading = 正在加载 { $name }...
notebook-file-missing-source = 缺少源文件

terminal-shared-session-reconnecting = 已离线，正在尝试重新连接...
terminal-banner-p10k-supported = Powerlevel10k 现在支持 Warp！{"  "}
terminal-banner-p10k-older-version-prefix = 你似乎正在运行较旧（不受支持）的版本，请按照
terminal-banner-these-instructions = 这些说明
terminal-banner-update-latest-suffix = {" "}更新到最新版本。
terminal-banner-pure-unsupported = Warp 尚不支持 Pure。你可以考虑改用受支持的提示符。{"  "}
terminal-loading-session = 正在加载会话...

ai-footer-hide-rich-input = 隐藏富输入
ai-footer-choose-environment = 选择环境
ai-footer-agent-environment = 智能体环境
ai-footer-enable-terminal-command-autodetection = 启用终端命令自动检测
ai-footer-disable-terminal-command-autodetection = 停用终端命令自动检测
ai-footer-turn-off-auto-approve-agent-actions = 关闭所有智能体操作的自动批准
ai-footer-auto-approve-agent-actions-for-task = 为此任务自动批准所有智能体操作
ai-footer-start-remote-control = 开始远程控制
ai-footer-login-required-remote-control = 登录后使用 /remote-control
ai-footer-see-logs-for-details = 查看日志详情
ai-footer-plugin-installed-restart-session = Warp 插件已安装。请重启会话以启用。
ai-footer-installing-warp-plugin = 正在安装 Warp 插件...
ai-footer-failed-install-warp-plugin = 安装 Warp 插件失败
ai-footer-plugin-updated-restart-session = Warp 插件已更新。请重启会话以启用。
ai-footer-updating-warp-plugin = 正在更新 Warp 插件...
ai-footer-failed-update-warp-plugin = 更新 Warp 插件失败
voice-input-limit-reached = 语音输入额度已达上限
voice-input-transcription-failed = 语音输入转写失败
ai-toolbar-context-chip = 上下文 Chip
ai-toolbar-model-selector = 模型选择器
ai-toolbar-autodetection = 自动检测
ai-toolbar-voice-input = 语音输入
ai-toolbar-attach-file = 附加文件
ai-toolbar-context-usage = 上下文用量
ai-toolbar-file-explorer = 文件浏览器
ai-toolbar-rich-input = 富输入
ai-toolbar-fast-forward = 快进
ai-tool-output-grep-for = Grep{" "}
ai-tool-output-grepping-for = 正在 grep{" "}
ai-tool-output-in-path-cancelled = {" "}于 { $path } 中已取消
ai-tool-output-in-path = {" "}于 { $path } 中
ai-tool-output-grep-patterns-cancelled = 已取消在 { $path } 中 grep 以下模式
ai-tool-output-grep-patterns-queued = 在 { $path } 中 grep 以下模式
ai-tool-output-grep-patterns-running = 正在 { $path } 中 grep 以下模式
ai-tool-output-search-files-match = 搜索匹配以下模式的文件{" "}
ai-tool-output-finding-files-match = 正在查找匹配以下模式的文件{" "}
ai-tool-output-file-patterns-cancelled = 已取消在 { $path } 中搜索匹配以下模式的文件
ai-tool-output-file-patterns-queued = 在 { $path } 中查找匹配以下模式的文件
ai-tool-output-file-patterns-running = 正在 { $path } 中查找匹配以下模式的文件
ai-tool-output-listing-messages = 正在列出消息
ai-tool-output-grepping-patterns = 正在 grep 模式
ai-tool-output-grepping-patterns-with-query = 正在 grep 模式：{ $query }
ai-tool-output-reading-messages = 正在读取 { $count } 条消息

code-review-discard-uncommitted-changes-title = 放弃未提交的更改？
code-review-discard-file-uncommitted-changes-title = 放弃此文件的所有未提交更改？
code-review-discard-all-changes-title = 放弃所有更改？
code-review-discard-file-changes-title = 放弃此文件的所有更改？
code-review-discard-uncommitted-changes-description = 你将放弃所有尚未提交的本地更改。
code-review-discard-file-uncommitted-changes-description = 这会将此文件恢复到上次提交的版本，并放弃本地编辑。
code-review-discard-all-changes-description = 你将放弃所有已提交和未提交的更改。
code-review-discard-file-main-branch-description = 这会将此文件恢复到 main 分支版本，并放弃所有已提交和未提交的编辑。
code-review-discard-file-branch-description = 这会将此文件重置为 { $branch } 分支版本，并放弃所有已提交和未提交的编辑。
code-review-stash-changes = 暂存更改
code-review-no-changes-to-commit = 没有可提交的更改
code-review-no-git-actions-available = 没有可用的 Git 操作
command-search-out-of-credits-contact-admin = 你的额度似乎已用完。请联系团队管理员升级以获取更多额度。
command-search-out-of-credits-prefix = 你的额度似乎已用完。{" "}
command-search-for-more-credits-suffix = {" "}以获取更多额度。
search-not-visible-to-other-users = 其他用户不可见
sharing-invite = 邀请
sharing-who-has-access = 有权访问的用户
terminal-shared-session-cancel-request = 取消请求
terminal-shared-session-continue-sharing = 继续分享
settings-import-reset-to-warp-defaults = 重置为 Warp 默认设置
settings-import-type-theme = 主题
settings-import-type-theme-with-comma = 主题，
settings-import-type-option-as-meta = Option 作为 Meta
settings-import-type-mouse-scroll-reporting = 鼠标/滚动上报
settings-import-type-font = 字体
settings-import-type-default-shell = 默认 Shell
settings-import-type-working-directory = 工作目录
settings-import-type-global-hotkey = 全局热键
settings-import-type-window-dimensions = 窗口尺寸
settings-import-type-copy-on-select = 选中即复制
settings-import-type-window-opacity = 窗口透明度
settings-import-type-cursor-blinking = 光标闪烁
settings-import-one-other-setting = 1 项其他设置
settings-import-other-settings = { $count } 项其他设置
workflow-argument-editor-helper = 填写此工作流的参数，并复制到终端会话中运行
workflow-add-environment-variables = 添加环境变量
workflow-environment-variables = 环境变量
workflow-new-environment-variables = 新建环境变量
ai-history-completed-successfully = 已成功完成
ai-history-pending = 等待中
ai-history-cancelled-by-user = 已由用户取消
ai-block-always-allow = 始终允许
ai-cancel-summarization = 取消摘要
ai-continue-summarization = 继续摘要
ai-dont-show-suggested-code-banners-again = 不再显示建议代码横幅
ai-inline-code-diff-no-file-name = 无文件名
ai-tool-call-cancelled = 工具调用已取消
ai-agent-view-open-in-different-pane = 已在其他窗格打开
passive-suggestion-feature-or-bug-label = 在 {1} 中开发功能或修复 bug
passive-suggestion-help-feature-or-bug-label = 帮我在 {1} 中开发功能或修复 bug
passive-suggestion-implement-feature-or-bug-query = 在 {1} 中实现功能或修复 bug。请询问你需要的所有细节。
passive-suggestion-create-pull-request-query = 帮我创建 Pull Request。
passive-suggestion-start-new-project-label = 帮我启动新项目
passive-suggestion-start-new-project-query = 帮我启动新项目。请询问你需要的所有细节。
passive-suggestion-node-project-label = 帮我启动 Node.js 项目
passive-suggestion-node-project-query = 帮我启动 Node.js 项目。请询问你需要的所有细节。
passive-suggestion-react-app-label = 帮我创建新的 React 应用
passive-suggestion-react-app-query = 帮我创建名为 {1} 的新 React 应用。请询问你需要的所有细节。
passive-suggestion-next-app-label = 帮我创建新的 Next.js 应用
passive-suggestion-next-app-query = 帮我创建名为 {1} 的新 Next.js 应用。请询问你需要的所有细节。
passive-suggestion-rust-project-label = 帮我为 {1} 启动 Rust 项目
passive-suggestion-rust-project-query = 帮我为 {1} 启动 Rust 项目。请询问你需要的所有细节。
passive-suggestion-poetry-project-label = 帮我为 {1} 启动 Poetry 项目
passive-suggestion-poetry-project-query = 帮我为 {1} 启动 Poetry 项目。请询问你需要的所有细节。
passive-suggestion-django-project-label = 帮我为 {1} 启动 Django 项目
passive-suggestion-django-project-query = 帮我为 {1} 启动 Django 项目。请询问你需要的所有细节。
passive-suggestion-rails-app-label = 帮我为 {1} 启动 Rails 应用
passive-suggestion-rails-app-query = 帮我为 {1} 启动 Rails 应用。请询问你需要的所有细节。
passive-suggestion-gradle-maven-project-label = 帮我启动 Gradle/Maven 项目
passive-suggestion-gradle-maven-project-query = 帮我启动 Gradle/Maven 项目。请询问你需要的所有细节。
passive-suggestion-go-project-label = 帮我为 {1} 启动 Go 项目
passive-suggestion-go-project-query = 帮我为 {1} 启动 Go 项目。请询问你需要的所有细节。
passive-suggestion-swift-project-label = 帮我启动 Swift 项目
passive-suggestion-swift-project-query = 帮我启动 Swift 项目。请询问你需要的所有细节。
passive-suggestion-terraform-config-label = 帮我启动 Terraform 配置
passive-suggestion-terraform-config-query = 帮我启动 Terraform 配置。请询问你需要的所有细节。
passive-suggestion-prisma-setup-label = 帮我在此项目中设置 Prisma
passive-suggestion-prisma-setup-query = 帮我在此项目中设置 Prisma。
passive-suggestion-install-dependencies-query = 帮我为 {1} 安装依赖。
passive-suggestion-ruby-project-label = 帮我设置新的 Ruby 项目
passive-suggestion-ruby-project-query = 帮我设置新的 Ruby 项目。请询问你需要的所有细节。
passive-suggestion-modelfile-query = 帮我为 {1} 设置 Modelfile。
passive-suggestion-kubernetes-utilization-query = 帮我理解集群中的资源使用情况。
passive-suggestion-kubernetes-inspect-query = 帮我检查 Kubernetes 资源。
passive-suggestion-docker-containers-query = 帮我管理正在运行的容器。
passive-suggestion-docker-images-query = 帮我管理 Docker 镜像。
passive-suggestion-docker-compose-label = 帮我用 Docker Compose 管理或排查 {1}
passive-suggestion-docker-compose-query = 帮我用 Docker Compose 管理或排查 {1}。
passive-suggestion-docker-network-query = 帮我配置容器使用 {1}。
passive-suggestion-vagrant-box-query = 帮我设置或自定义 Vagrant box {1}。
passive-suggestion-vagrant-up-query = 帮我预配环境或排查 Vagrant 启动问题。
passive-suggestion-grep-search-query = 帮我在文件中搜索 {1}。
passive-suggestion-find-search-query = 帮我使用 {1} 在文件中搜索代码。
passive-suggestion-ssh-keygen-query = 带我完成 SSH key 生成。

# =============================================================================
# SECTION: remaining-ui-surfaces (Owner: agent-i18n-remaining)
# =============================================================================

common-update = 更新
common-reject = 拒绝
common-open-link = 打开链接
common-open-file = 打开文件
common-open-folder = 打开文件夹
common-name = 名称
common-rule = 规则
common-skip-for-now = 暂时跳过
common-never = 从不
common-save-changes = 保存更改
common-do-not-show-again = 不再显示
common-dont-show-again-with-period = 不再显示。
common-refresh = 刷新
common-resource-not-found-or-access-denied = 资源不存在或无访问权限
workspace-close-session = 关闭会话
workspace-auto-reload = 自动重载
workspace-add-new-repo = {" "}+ 添加新仓库
workspace-notification-permission-denied-toast = Warp 没有发送桌面通知的权限。
workspace-troubleshoot-notifications-link = 排查通知问题
workspace-plan-synced-to-warp-drive-toast = 计划已同步到你的 Warp Drive
workspace-remote-control-link-copied-toast = 远程控制链接已复制。
workspace-update-now = 立即更新
workspace-update-warp = 更新 Warp
workspace-app-out-of-date-needs-update = 你的应用已过期，需要更新。
workspace-restart-app-and-update-now = 重启应用并立即更新
workspace-sampling-process-toast = 正在采样进程，持续 3 秒...
workspace-version-deprecation-banner = 你的应用已过期，部分功能可能无法正常工作。请立即更新。
workspace-version-deprecation-without-permissions-banner = 若不立即更新，部分 Warp 功能可能无法正常工作，但 Warp 无法执行更新。
workspace-new-version-unable-to-update-banner = 有新版本可用，但 Warp 无法执行更新。
workspace-unable-to-launch-new-installed-version = Warp 无法启动新安装的版本。
tab-config-session-type = 会话类型
terminal-copy-error = 复制错误
terminal-authenticate-with-github = 使用 GitHub 认证
terminal-create-environment = 创建环境
terminal-regenerate-agents-file = 重新生成 AGENTS.md 文件
terminal-view-index-status = 查看索引状态
terminal-shared-session-request-edit-access = 请求编辑权限
terminal-create-team = 创建团队
terminal-warpify-without-tmux = 不使用 TMUX 进行 Warpify
terminal-continue-without-warpification = 不进行 Warpification 继续
terminal-always-install = 始终安装
terminal-never-install = 从不安装
terminal-ssh-report-issue-prefix = 我们正在积极改进 Warp 中 SSH 的稳定性。请考虑
terminal-ssh-report-issue-link = 提交 issue
terminal-ssh-report-issue-suffix = ，方便我们更好地定位问题。
terminal-ssh-why-need-tmux = 为什么需要 tmux？
terminal-ssh-file-uploads-title = 文件上传
terminal-ssh-close-upload-session = 关闭上传会话
terminal-ssh-view-upload-session = 查看上传会话
terminal-reveal-secret = 显示密钥
terminal-hide-secret = 隐藏密钥
terminal-copy-secret = 复制密钥
terminal-tag-agent-for-assistance = 标记智能体协助
terminal-save-as-workflow-secrets-tooltip = 包含密钥的 Block 无法保存。
terminal-agent-mode-setup-title = 为此代码库优化 Warp？
terminal-agent-mode-setup-description = 让智能体理解你的代码库并为其生成规则，以获得更智能、更一致的响应。你也可以随时运行 /init 来完成。
terminal-agent-mode-setup-optimize = 优化
terminal-no-active-conversation-to-export = 没有可导出的活动对话
terminal-slow-shell-startup-banner-prefix = 你的 shell 启动时间似乎有点久...{"  "}
terminal-more-info = 更多信息
terminal-show-initialization-block = 显示初始化 Block
terminal-shell-process-exited = Shell 进程已退出
terminal-shell-process-could-not-start = Shell 进程无法启动！
terminal-shell-process-exited-prematurely = Shell 进程过早退出！
terminal-shell-premature-subtext = 启动 { $shell_detail } 并对其进行 Warpify 时出错，导致进程终止。这里显示了 Warpify 脚本输出，可能能帮助定位原因。
terminal-file-issue = 提交 issue
notifications-banner-troubleshoot = 排查问题
notifications-banner-dismissed-title = 我们不会再显示此横幅，但你仍可随时前往设置启用通知。
notifications-banner-disabled-title = 通知已关闭，但你仍可随时前往设置启用通知。
notifications-banner-enable = 启用
notifications-banner-permissions-accepted-title = 成功！你现在可以接收桌面通知了。
notifications-banner-permissions-denied-title = Warp 被拒绝发送通知的权限。
notifications-banner-permissions-error-title = 请求权限时出错。
notifications-banner-allow-permissions-title = 别忘了在权限请求中点击「允许」以完成通知设置。
notifications-banner-configure-notifications = 配置通知
notifications-banner-set-permissions = 设置权限
ai-edit-api-keys = 编辑 API Key
ai-manage-privacy-settings = 管理隐私设置
ai-block-manage-agent-permissions = 管理智能体权限
agent-zero-state-cloud-agents-description = 使用本地智能体并行运行多个智能体，构建可自主运行的智能体，并在本机查看智能体状态。
agent-zero-state-visit-docs = 查看文档
ai-execution-profile-agent-decides = 由智能体决定
ai-execution-profile-always-ask = 始终询问
ai-execution-profile-ask-on-first-write = 首次写入时询问
ai-execution-profile-never-ask = 从不询问
ai-execution-profile-ask-unless-auto-approve = 自动批准以外均询问
code-accept-and-save = 接受并保存
code-hunk-label = 代码块：
code-discard-this-version = 放弃此版本
code-overwrite = 覆盖
code-review-send-to-agent = 发送给智能体
code-review-open-pr = 打开 PR
code-review-pr-created-toast = PR 创建成功。
code-review-comments-sent-to-agent = 评论已发送给智能体
code-review-could-not-submit-comments = 无法将评论提交给智能体
code-review-tooltip-view-changes = 查看变更
code-review-diffs-local-workspaces-only = Diff 仅适用于本地工作区。
code-review-diffs-git-repositories-only = Diff 仅适用于 Git 仓库。
code-review-diffs-wsl-unsupported = WSL 暂不支持 Diff。
code-review-generating-commit-message-placeholder = 正在生成提交信息…
code-review-type-commit-message-placeholder = 输入提交信息
code-review-committing-loading = 正在提交…
code-review-commit-message-label = 提交信息
code-review-no-non-outdated-comments-to-send = 没有可发送的未过期评论
code-review-send-diff-comments-to = 将 diff 评论发送给 { $label }
code-review-ai-must-be-enabled-to-send-comments = 必须启用 AI 才能将评论发送给智能体
code-review-agent-code-review-requires-ai-credits = 智能体代码评审需要 AI 额度
code-review-all-terminals-are-busy = 所有终端都正忙
code-review-send-diff-comments-to-agent = 将 diff 评论发送给智能体
code-failed-to-load-file-toast = 文件加载失败。
code-failed-to-save-file-toast = 文件保存失败。
code-file-saved-toast = 文件已保存。
notebook-apply-link = 应用链接
notebook-sync-conflict-resolution-message = 此 Notebook 无法保存，因为你编辑时内容已被其他更改更新。请复制你的内容并刷新。
notebook-sync-feature-not-available-message = 此 Notebook 无法保存到服务器，因为该功能暂时不可用。更改已保存在本地，请稍后重试。
notebook-link-copied-toast = 链接已复制
settings-share-with-team = 与团队共享
tooltip-secrets-not-sent-to-warp-server = *密钥不会发送到 Warp 服务器。
editor-voice-limit-hit-toast = 你已达到语音请求额度上限。额度将在下个周期刷新。
editor-voice-error-toast = 处理语音输入时出错。
ai-copied-branch-name-toast = 分支名称已复制
workflow-new-enum = 新建枚举
workflow-edit-enum = 编辑枚举
workflow-enum-variant-placeholder = 变体
workflow-enum-variants = 变体
quit-warning-dont-save = 不保存
quit-warning-show-running-processes = 显示运行中的进程
quit-warning-save-changes-title = 保存更改？
