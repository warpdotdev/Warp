# Changelog

本文档记录 OpenWarp 各个发布版本的关键变更。仅收录功能性 commit,省略 dev / stable 等内部滚动 tag。

## [Unreleased]

- **AI / BYOP**:port opencode `applyCaching`,启用 prompt caching;`write_to_long_running_shell_command` 在 line 模式下拒绝嵌入 LF;BYOP LRC monitor fallback 改走 silent subtask
- **云端剥离 Phase 1–2**:增加 `cloud-disabled` channel 谓词;清理 billing/pricing、referral/reward、cloud sharing dialog UI;退订 RTC UpdateManager;退役 notebook/folder sync queue
- **平台**:修复 Spotlight/Finder/Launchpad 启动 macOS 时的 panic;`run_shell_command` stdout 兜底回退至 command grid
- **基建**:`.gitattributes` 强制 LF;新增 stale bot 与 Claude Code GitHub workflow
- **编辑器**:代码/Markdown 查看器新增 15 种语言语法高亮(Dart、Zig、SCSS、R、Julia、OCaml、Erlang、Nix、Groovy、Solidity、GraphQL、Protobuf、Clojure、Elm、CMake)

## [v2026.05.06.preview] — 2026-05-06

- **AI**
  - 集成 DeepSeek CLI agent,提升 LSP 安装可靠性
  - LSP 改为全局 `enabled_lsp_servers` setting,移除 `/index` 命令与 codebase indexing runtime
  - `/plan` 真实复刻 Plan Mode(system prompt + 工具硬护栏)
  - Agent dynamic tool whitelist、`persist_conversations` setting、auto-approve 下 `ask_user_question` 始终询问
  - BYOP 支持 provider extra headers
- **修复**
  - `apply_file_diffs` schema 从 `const` 改为 `enum` 适配 Gemini
  - SSE 卡顿根因——genai gzip 默认关闭 + workflow 拆分
  - 无云端环境下计划文件夹笔记本立即创建
- **品牌**:logo 与图标改用白色背景;BYOP 模式隐藏 credits/billing UI

## [v2026.05.04.preview] — 2026-05-04

- **SSH Manager**:数据层 + 持久化 + keychain 落地;UI/UX 完整接入(面板 + 中央 Pane + 拖拽 + 折叠 + Connect + Command Palette)
- **AI**:区分模型"无建议"输出并完善提示系统;BYOP 历史多模态扩展到 PDF/audio,opencode 风格 ERROR 替换;UserQuery.context.images 全链路保活
- **UI**:标题栏搜索框可隐藏开关;键位设置编辑态与快捷键徽章对比度修复
- **i18n**:剩余主要界面固定文案汉化;`/model` 默认绑定 `alt-shift-/`
- **修复**:Anthropic adapter 默认带 1M context beta header;BYOP ToolCall 首帧即 emit 占位卡;OpenAI-strict provider 禁回传 `reasoning_content`
- **基建**:CI 修复 `.deb` 构建并启用 PR 测试

## [v2026.05.03.preview(.2/.3/.4)] — 2026-05-03

- **上游同步**:合入大批 warp-upstream commit(tab 跨窗口拖拽、shell 脚本识别、IME cursor、远程服务器初始化重构、SSH remote-server 自动升级、跨窗口 tab drag 等);建立 rerere + `openwarp-ours` 合并驱动;新增黑名单文档
- **AI / BYOP**:工具参数 type-mismatched 输出的 coerce 层;suspicious backslash 扫描收紧消除 ls/diff 误报
- **i18n**:中文国际化补齐(设置面板等)
- **网站**:GitHub 地址统一为 `zerx-lab/warp`;移动端横向溢出修复
- **修复**:Windows 任务栏 ICO 与上游格式对齐;NLD in terminal 默认 true 恢复中文输入自动入 AI

## [v2026.05.02.preview] — 2026-05-02

- **AI / BYOP**
  - 完成会话压缩闭环——`byop_compaction` 模块、settings 持久化、auto prune、overflow 透传,1:1 复刻 opencode
  - reasoning effort 从 provider settings 迁移到输入框 picker
  - 多模态附件能力接入 BYOP 路径
  - 本地 BYOP webfetch / websearch 与 Exa 集成
  - 按模型标识选择系统提示模板,新增多份模板
- **隐私 / 云端剥离**
  - 物理删 P4 易剥离死代码(anonymous_id / EXPERIMENT_ID_HEADER / settings 同步 / app_focus)
  - 切断闭源遥测、Sentry、anonymous_id、Settings 同步四条外发链路
  - 三个隐私开关默认值 true → false
  - `cloud_conversations` 两波清理(UI / 隐私 / FeatureFlag / AIClient / cargo feature)
- **重构**:移除 blocklist 人工智能响应评分及埋点;移除 `agent_attribution` 与 Oz changelog toggle
- **CI**:周构建改为正式发布并规范 tag

## [v2026.05.01.preview] — 2026-05-01

- **云端剥离**:物理删 6 个云端 LLM tool + child_agent + orchestration;物理删 share modal 三件套与 billing denied modal;website 换单色 logo
- **AI**
  - Workflow Autofill 接入 BYOP one-shot
  - BYOP LRC 后续轮持续注入上下文 + sanitize 强化 + 控制键 token
  - 聊天流增加远程登录会话提示与推理回传
  - genai 错误映射细化为 Stream / Other variants
  - chat stream adapter,修复 ToolCall None 处理
- **平台**:`warpui_core` 避免重复扫描系统字体;同步命令无条件禁用 pager,改用 `PAGER=cat` 保留真实退出码
- **网站**:全站组件与 i18n 重构,Tailwind 与全局样式同步

## [v2026.04.30.oss] — 2026-04-30

- **CI**:CHANNEL `preview` → `oss`,修 Windows / macOS 构建失败
- **重构**:删除 cloud_mode 残留代码与设置

## [v2026.04.30.preview] — 2026-04-30

OpenWarp 社区分支首个预览版本。

- **品牌与定位**:OpenWarp 改名 + logo 重制 + 社区分支 README
- **BYOP**
  - `async-openai` → `genai`,支持 5 种原生协议显式绑定
  - Providers 子页 + models.dev 数据源 + 快速添加搜索框
  - prompt 模板精简
- **去中心化清理**:移除 `UseComputer` / `RequestComputerUse` 工具、Drive `Create team` / `Join team` 入口、referral 相关代码
- **i18n**:Fluent 基础设施 + 12 个 settings_view 文件翻译;ai / features / teams 三页 i18n 补全
- **网站**:新增 BYOP 落地页(Astro + Tailwind, 中英双语);响应式优化
- **AI**:CJK 输入分类、reasoning 拆分、BYOP tool_call 诊断、LRC tag-in 合成虚拟 subagent + 浮窗 spawn 链路
- **CI**:Release 显式声明 `contents: write` 权限修 403

[Unreleased]: https://github.com/zerx-lab/warp/compare/v2026.05.06.preview...HEAD
[v2026.05.06.preview]: https://github.com/zerx-lab/warp/compare/v2026.05.04.preview...v2026.05.06.preview
[v2026.05.04.preview]: https://github.com/zerx-lab/warp/compare/v2026.05.03.preview.4...v2026.05.04.preview
[v2026.05.03.preview(.2/.3/.4)]: https://github.com/zerx-lab/warp/compare/v2026.05.02.preview...v2026.05.03.preview.4
[v2026.05.02.preview]: https://github.com/zerx-lab/warp/compare/v2026.05.01.preview...v2026.05.02.preview
[v2026.05.01.preview]: https://github.com/zerx-lab/warp/compare/v2026.04.30.oss...v2026.05.01.preview
[v2026.04.30.oss]: https://github.com/zerx-lab/warp/compare/v2026.04.30.preview...v2026.04.30.oss
[v2026.04.30.preview]: https://github.com/zerx-lab/warp/releases/tag/v2026.04.30.preview
