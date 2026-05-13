# AGENTS.md

> 本文件是给在本仓库中工作的 AI/自动化 agent 的导航文档。它汇总了仓库的整体架构、Cargo 工作区中每个 crate 的职责、`app/` 主二进制下各子模块的边界,以及在做改动前必须遵守的工程约定。
>
> 与 `WARP.md` 是配套关系:`WARP.md` 是工程师手册(命令、风格、流程),本文件是**代码地图**。先读 `WARP.md`,再用本文件定位到正确的 crate / 模块。

---

## 1. 仓库总览

Warp 是一个以 Rust 为主的 **agentic 终端 / 开发环境**:在一个自研 UI 框架(WarpUI)上,集成了终端模拟、AI Agent、云同步(Drive)、代码评审、补全、Notebook、设置、IPC 等能力。

顶层目录:

| 目录 | 作用 |
|------|------|
| `app/` | 主二进制 crate(`warp`),装配所有子系统、UI、数据库迁移、平台粘合层 |
| `crates/` | 67 个工作区成员,按职责拆分的库 crate |
| `command-signatures-v2/` | 独立子项目(在 nextest 运行时被 `--exclude`) |
| `script/` | 跨平台 bootstrap、构建、presubmit 脚本 |
| `resources/` | 字体、图标、shell 集成脚本、shaders 等运行时资源 |
| `docker/` | 容器化构建相关 |
| `specs/` | 产品/技术 spec 文档 |
| `.agents/skills`, `.claude/skills` | agent 工作流的 skill 描述(创建 PR、修复错误、特性灰度等) |
| `.warp/`, `.config/`, `.cargo/`, `.vscode/` | 各类工具配置 |

构建系统:Cargo workspace,`resolver = "2"`,`default-members` 故意收敛到经常需要编译/测试的子集(见 `Cargo.toml`)。`serve-wasm` 与 `integration` 默认不在 `default-members` 内。

许可证拆分:
- `crates/warpui` 与 `crates/warpui_core` → MIT
- 其余 → AGPL-3.0-only

---

## 2. 顶层架构分层

从底向上大致是 4 层。在新增代码或定位 bug 时,先确定改动属于哪一层,**不要跨层倒挂依赖**。

```
app/  (主二进制:装配、入口、平台粘合、持久化迁移、UI 视图根)
  ↑
产品域 crate:ai / computer_use / vim / onboarding /
              warp_completer / lsp / languages / code-review …
  ↑
框架 crate:warpui / warpui_core / warpui_extras / editor /
            ui_components / sum_tree / syntax_tree
  ↑
基础设施 crate:warp_core / warp_util / http_client /
                websocket / ipc / jsonrpc / persistence / graphql /
                managed_secrets / virtual_fs / watcher / asset_cache …
```

关键架构模式(详见 `WARP.md`):

1. **Entity-Handle 系统**:`App` 全局拥有所有 view/model entity,View 之间通过 `ViewHandle<T>` 引用,而不是直接拥有。
2. **Element / Action**:UI 由声明式 Element 树 + Action 事件系统组成(Flutter 风格)。
3. **跨平台**:macOS / Windows / Linux 原生实现 + WASM 目标;平台代码用 `#[cfg(...)]` 隔离。
4. **AI 集成**:Agent Mode 与上下文索引,代码集中在 `app/src/ai`(389 文件)与 `crates/ai`。
5. **云同步**:`Drive` 让对象在多设备同步,见 `app/src/drive` 与 `crates/warp_files`。
6. **Feature Flag**:运行时灰度优先于 `#[cfg]`,枚举定义在 `crates/warp_core/src/features.rs`。

---

## 3. `crates/` 一览

下表按主题分组列出全部 67 个 crate。每行只写**一句话职责**;要看实现细节,直接打开对应 `crates/<name>/src/lib.rs`(很多 crate 在 `lib.rs` 顶部有 `//!` 模块文档)。

### 3.1 UI 框架 / 视图层

| Crate | 职责 |
|-------|------|
| `warpui_core` | WarpUI 框架核心(MIT):`App` / `Entity` / `ViewHandle` / `AppContext` 等基础设施 |
| `warpui` | WarpUI 上层组件、Element 树、布局、渲染管线(MIT) |
| `warpui_extras` | WarpUI 的可选扩展件,默认不启用全部 features |
| `ui_components` | 跨视图复用的高层组件库(按钮、输入、列表、模态等) |
| `editor` (`warp_editor`) | 文本编辑器:缓冲区、选择、光标、键映射、撤销栈 |
| `sum_tree` | 持久化平衡 B-树,编辑器 / Notebook / 大列表的核心数据结构 |
| `syntax_tree` | Tree-sitter 封装与语法高亮支持 |
| `markdown_parser` | Markdown 解析(用于 AI 消息、文档视图、Notebook 等) |
| `vim` | Vim 模式键绑定与操作语义 |
| `voice_input` | 语音输入支持 |

### 3.2 终端

| Crate | 职责 |
|-------|------|
| `warp_terminal` | 终端模拟核心:PTY 管理、ANSI/VT 解析、grid、滚动、shell 集成钩子 |
| `input_classifier` | 终端输入意图分类(纯命令 / 自然语言 / AI Prompt) |
| `natural_language_detection` | 自然语言识别(配合 `input_classifier`) |

### 3.3 AI / Agent

| Crate | 职责 |
|-------|------|
| `ai` | AI 模型客户端、Prompt 编排、Agent 协议、工具调用框架 |
| `computer_use` | "Computer Use" 工具能力(截屏、点击、键入等)的 Rust 端实现 |
| `command-signatures-v2` | 命令签名 v2(给 AI 用的命令分类元数据);独立项目,不进入主工作区测试集 |
| `onboarding` | 新用户引导流程数据/状态 |

### 3.4 网络 / 协议 / IPC

| Crate | 职责 |
|-------|------|
| `http_client` | 工作区统一 HTTP 客户端封装 |
| `http_server` | 内嵌 HTTP server(本地 RPC、登录回调等) |
| `websocket` | 原生与 WASM 共用的 WebSocket 抽象,适配 `graphql_ws_client` |
| `ipc` | 通用类型化 IPC 请求/响应协议(进程间) |
| `jsonrpc` | JSON-RPC 实现 |
| `lsp` | Language Server Protocol 客户端实现 |
| `remote_server` | 远端 sshd 模式下的服务端逻辑 |
| `serve-wasm` | 把 WASM 构建产物 host 出来的辅助 server(默认不参与编译) |
| `firebase` | Firebase 客户端工具(Crash/分析等渠道) |

### 3.5 持久化 / 文件 / 资源

| Crate | 职责 |
|-------|------|
| `persistence` | Diesel + SQLite 持久层基础;**migrations 在 `app/migrations/`,schema 在 `app/src/persistence/schema.rs`** |
| `warp_files` | Drive 文件、Workflow、Notebook 等可同步文件对象 |
| `virtual_fs` | 抽象文件系统(测试用 mock 与生产用真实 FS 同接口) |
| `repo_metadata` | 仓库元数据:文件树构建、`.gitignore` 处理、文件系统监听 |
| `watcher` | 文件系统监视器(对 `notify` 的封装) |
| `asset_cache` | 资源磁盘/内存缓存 |
| `asset_macro` | `bundled!` / `theme!` 等资源引用宏 |
| `managed_secrets` / `managed_secrets_wasm` | Keychain / DPAPI / Linux Keyring 抽象 + WASM 代理 |

### 3.6 配置 / 设置

| Crate | 职责 |
|-------|------|
| `settings` | 设置存储与变更分发 |
| `settings_value` | `SettingsValue` trait:控制 TOML 序列化语义 |
| `settings_value_derive` | `#[derive(SettingsValue)]` 过程宏(枚举变体转 snake_case 等) |
| `warp_features` | Feature flag 高层 API(消费者侧) |
| `channel_versions` | 发布通道(stable/preview/dogfood)与版本对比 |

### 3.7 命令 / 补全 / 语言

| Crate | 职责 |
|-------|------|
| `command` | 跨平台进程派生的安全封装,**特别处理 Windows 的 `no_window` 标志**;新派生子进程一律走这里 |
| `warp_completer` | 补全引擎(支持 `--features v2`) |
| `languages` | 语言/扩展名/Tree-sitter grammar 注册 |
| `warp_ripgrep` | 给 `warp_cli` 用的 ripgrep 薄封装 |
| `warp_cli` | 二进制内的 CLI 子命令解析(`warp <subcmd>`) |
| `fuzzy_match` | 模糊匹配 + glob 风格通配,用于路径搜索与命令面板 |

### 3.8 平台 / 系统服务

| Crate | 职责 |
|-------|------|
| `app-installation-detection` | 检测系统中已安装的 app(用于 launcher 联动) |
| `prevent_sleep` | 抑制休眠(长任务/AI Agent 期间) |
| `isolation_platform` | 在 Docker / GitHub Actions 等沙箱中运行的兼容层 |
| `node_runtime` | 自动安装/管理 Node.js 与 npm(macOS/Linux/Windows × 多架构) |
| `warp_js` | 在 Rust 侧操作 JavaScript 值/函数的助手抽象 |

### 3.9 通用工具 / 通信

| Crate | 职责 |
|-------|------|
| `warp_core` | 工作区内最底层的"core":平台抽象、`features.rs` 中 `FeatureFlag` 枚举与 `DOGFOOD/PREVIEW/RELEASE_FLAGS` |
| `warp_util` | 跨多个 crate 复用的通用工具函数 |
| `warp_logging` | 日志配置统一入口 |
| `simple_logger` | 给 `remote_server` 等 stderr-only 进程用的简易异步文件日志 |
| `warp_web_event_bus` | Web 端事件总线(给嵌入的 web view) |
| `field_mask` | gRPC/Proto 风格 FieldMask 工具 |
| `string-offset` | 偏移量基础类型(byte/char/utf16) |
| `handlebars` | Handlebars 模板引擎封装 |
| `integration` | 集成测试框架,只用于测试 |

> 命名小坑:`crates/editor` 的 package 名是 `warp_editor`;`crates/isolation_platform` 是 `warp_isolation_platform`;`crates/managed_secrets` 是 `warp_managed_secrets`;`crates/virtual_fs` 是 `virtual-fs`(短横线);`crates/string-offset` 是 `string-offset`(短横线)。

---

## 4. `app/` 子模块导航

`app/src/` 下平铺了 60+ 个产品域目录,每个目录大致对应一条产品功能线。以下按主题分组,括号内是大致 `.rs` 文件数,用于估计模块体量:

### 4.1 启动 / 装配 / 全局
- `bin/` (7) — 多个二进制入口(主程序、附带工具)。
- `lib.rs` / `app_state.rs` / `app_state_tests.rs` — 应用状态根。
- `app_menus.rs`, `app_services/`, `app_id_test.rs`
- `appearance.rs`, `gpu_state.rs`, `font_fallback.rs`, `global_resource_handles.rs`
- `dynamic_libraries.rs`, `alloc.rs`, `tracing.rs`, `profiling.rs`
- `crash_recovery.rs`, `crash_reporting/` (4)
- `features.rs` — `app/` 内对 `warp_core::FeatureFlag` 的消费;新增 flag 时通常需要在两处都接好。
- `channel.rs`, `download_method.rs`, `autoupdate/` (8)

### 4.2 终端
- `terminal/` (427) — 主体:shell 进程、PTY、grid、blocks、shell 集成、命令执行、I/O 流水线。
- `default_terminal/` (2) — 默认终端启动逻辑。
- `shell_indicator.rs`, `prefix.rs` / `prefix_test.rs`(命令前缀解析),`vim_registers.rs`

### 4.3 AI / Agent
- `ai/` (389) — 包含 Agent UI、对话模型、Agent 管理、工具/MCP、Cloud Agent、Plan/Diff 视图、artifacts、blocklist、execution profiles 等。**这是仓库最大的子树**,改动前先在该目录内 grep 具体子主题(`agent_*`, `conversation_*`, `cloud_agent_*`, `mcp`, `tool_*`)。
- `ai_assistant/` (9) — 旧版 AI 辅助入口/适配。
- `chip_configurator/`, `context_chips/` (22) — Agent 上下文 chip 选择/构造。
- `coding_entrypoints/` (5), `coding_panel_enablement_state.rs`
- `prompt/` (2), `tips/` (3), `voice/` (2), `completer/` (3)

### 4.4 编辑器 / 代码 / Review
- `editor/` (38) — 主编辑器集成。
- `code/` (52) — 代码视图、diff、navigation。
- `code_review/` (36) — Code Review 流。
- `notebooks/` (30), `workflows/` (22)

### 4.5 搜索
- `search/` (172) — 多目标搜索(文件、命令、Agent 历史等)。
- `search_bar.rs`

### 4.6 服务端通信 / Drive / 同步
- `server/` (55) — 与 warp 后端的 HTTP/WS 交互(对应本地开发模式 `with_local_server`)。
- `drive/` (45) — 云端对象同步入口。
- `cloud_object/` (12) — 云对象抽象层(workflow、notebook 等)。
- `remote_server/` (5) — 客户端侧连接 remote 模式 sshd 的 glue。

### 4.7 设置 / 用户配置 / 主题 / Onboarding
- `settings/` (46), `settings_view/` (63)
- `user_config/` (6), `themes/` (11), `appearance.rs`
- `experiments/` (7), `tab_configs/` (15), `launch_configs/` (4)
- `tips/`, `banner/` (3), `quit_warning/` (1), `wasm_nux_dialog.rs`, `referral_theme_status.rs`

### 4.8 认证 / 计费 / 使用量
- `auth/` (22) — 登录、token、SSO。
- `billing/` (3), `pricing/` (1), `usage/` (1), `reward_view.rs`

### 4.9 持久化
- `persistence/` (9) — Diesel migrations 装配、`schema.rs`(由 Diesel 生成)、迁移运行器。
- 迁移文件在仓库 `migrations/` 顶级目录(由 Diesel CLI 管理)。

### 4.10 平台 / 系统集成
- `platform/` (2), `system/` (3) / `system.rs`
- `login_item/` (3), `antivirus/` (3), `network.rs`
- `external_secrets/` (1), `env_vars/` (14)
- `keyboard.rs` / `keyboard_test.rs`, `safe_triangle.rs` / `safe_triangle_tests.rs`(菜单悬停安全三角)

### 4.11 视图根 / 面板 / 通用 UI
- `root_view.rs` / `root_view_tests.rs`
- `pane_group/` (35) — 分屏分块布局。
- `tab.rs`, `command_palette.rs`, `modal.rs`, `menu.rs` / `menu_test.rs`
- `palette.rs`, `notification.rs`, `resource_center/` (10)
- `view_components/` (20), `ui_components/` (14)
- `workspace/` (54), `workspaces/` (10), `voltron.rs`(多窗口/多 workspace 协调)
- `session_management.rs`, `undo_close/` (3), `word_block_editor.rs`
- `suggestions/` (2), `input_suggestions.rs` / `input_suggestions_test.rs`
- `plugin/` (21) — 插件系统接入。
- `uri/` (7) — `warp://` URL 处理。
- `debug_dump.rs`, `debounce.rs`, `interval_timer.rs`, `throttle.rs`
- `linear.rs`, `resource_limits.rs`, `warp_managed_paths_watcher.rs`
- `preview_config_migration.rs` / `preview_config_migration_tests.rs`
- `window_settings.rs`, `projects.rs`

### 4.12 测试基建
- `integration_testing/` (79) — 端到端集成测试支撑。
- `test_util/` (6) — 单元测试公共 util。

---

## 5. 工程纪律(给 Agent 的强约束)

> 这些基于 `WARP.md` 与项目自定义规则整理;本文件对 agent 的验证要求以 `cargo check` 为准。

### 5.1 必读约定
- **注释/回复一律使用简体中文**(用户规则)。
- 在 git 索引内的搜索/grep 使用 `fff` 工具或 `rg -n "<关键词>" <路径>`;`read_file` 仅用于图片/二进制。
- 提 PR / 推新 commit 之前,**只需**通过:`cargo check`。
- 改动需精准:**每一行修改都能溯源到用户请求**,不要顺手"改进"无关代码、注释、格式。
- 简洁优先:不要为单点使用引入抽象、配置、错误处理、多余特性。
- 多解释方案、暴露不确定性,而不是默默替用户做选择。

### 5.2 Rust 风格(摘自 `WARP.md`)
- 闭包参数不要写多余类型注解。
- 顶部统一 `use`,不要写一长串路径限定;`#[cfg]` 分支内例外。
- 上下文参数命名为 `ctx` 且放在最后;若同时有闭包参数,闭包放最后。
- 未使用参数**直接删除**而不是加 `_` 前缀,同步更新调用点。
- `println!` / `format!` 等宏使用内联格式参数(`"{x}"` 而不是 `"{}", x`)以满足 `uninlined_format_args`。
- `match` 语句**禁止使用 `_` 通配**(除非确实需要),保持穷尽匹配。
- 不要因为不相关的修改去删/改既有注释。

### 5.3 终端模型锁(高优先级!)
- 调用 `TerminalModel::lock()` 极易死锁(macOS 上表现为 UI 卡死/沙滩球)。
- 新增 `model.lock()` 前必须确认调用栈中没有上层已经持锁;尽量把已锁定的引用沿调用栈往下传,而不是再次加锁。
- 持锁范围最小化,持锁时不要调用可能再次加锁的函数。

### 5.4 Feature Flag
- 新增:在 `crates/warp_core/src/features.rs` 的 `FeatureFlag` 枚举里加 variant;按需把它加入 `DOGFOOD_FLAGS` / `PREVIEW_FLAGS` / `RELEASE_FLAGS`。
- 使用:**优先**用运行时 `FeatureFlag::Xxx.is_enabled()`,而不是 `#[cfg(...)]`;只有当无 `cfg` 就无法编译(平台/可选依赖)时才用 `cfg`。
- 包裹整段产品功能,而非每个调用点都加;上线稳定后**清理 flag 与死分支**。
- UI 入口要与代码路径用同一个 flag。

### 5.5 数据库
- ORM:Diesel + SQLite。
- 新增/改 schema 必须走 migration:在 `migrations/` 加新目录(`up.sql` / `down.sql`),不要手改 `app/src/persistence/schema.rs`(由 `diesel print-schema` 生成)。

### 5.6 测试
- 用 `cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2`。
- 单元测试放到 `${文件名}_tests.rs` 或 `mod_test.rs`,在原文件末尾用:

  ```rust
  #[cfg(test)]
  #[path = "filename_tests.rs"]
  mod tests;
  ```

- 集成测试用 `crates/integration` 的框架,样例在 `app/src/integration_testing/`。

### 5.7 跨进程命令
- 不要直接 `std::process::Command::new(...)`(尤其在 Windows 上会弹窗),统一走 `crates/command`。

### 5.8 子代理 / 多代理
- 大任务拆分为**写入域不重叠**的子任务并行下发;信息收集类任务可以并行。
- 简单任务直接做,不要过度拆分。

---

## 6. 常用入口速查

| 想做的事 | 起点 |
|---------|------|
| 改终端 grid / shell 集成 | `crates/warp_terminal/src/`,联动 `app/src/terminal/` |
| 改 Agent UI / 对话 | `app/src/ai/` 内按 `agent_*` / `conversation_*` 分主题 grep |
| 改命令补全 | `crates/warp_completer/`(注意 `--features v2`) |
| 改 AI 模型 / 工具调用协议 | `crates/ai/` |
| 加新设置项 | `crates/settings_value*`、`crates/settings`,UI 在 `app/src/settings_view/` |
| 加 Feature Flag | `crates/warp_core/src/features.rs` + 使用点 |
| 改云端同步对象 | `crates/warp_files` + `app/src/drive/` + `app/src/cloud_object/` |
| 改持久化结构 | `migrations/` 加迁移 + `crates/persistence` |
| 加新二进制工具 | `app/src/bin/` |
| 平台特定代码 | 用 `#[cfg(target_os = "...")]`,UI 平台胶水在 `app/src/platform/` |
| Vim 模式 | `crates/vim` + `app/src/vim_registers.rs` |
| Notebook / Workflow | `app/src/notebooks/`、`app/src/workflows/`、`crates/warp_files` |
| 跨平台进程派生 | `crates/command` |
| 文件搜索 / 监听 | `crates/repo_metadata`、`crates/watcher`、`crates/warp_ripgrep` |

---

## 7. 修改前的检查清单

在动键盘改代码前,自问一次:

1. 这件事属于哪一层 / 哪个 crate / 哪个 `app/src/<子模块>`?改动是否会跨越层界?
2. 是否需要新增依赖?如已存在的 workspace 依赖能复用,优先复用 `Cargo.toml` `[workspace.dependencies]`。
3. 这是产品功能吗?是否需要 Feature Flag 包起来?
4. 涉及终端模型?当前调用栈是否已经持有 `TerminalModel` 锁?
5. 涉及子进程?是否走了 `crates/command`?
6. 涉及持久化?是否需要 migration?
7. 已经写了对应的 `${file}_tests.rs`?
8. `cargo check` 是否绿?
9. 改动的每一行能否一一对应到用户请求?顺手做的"小重构"是否应该回滚?

把上面 9 条都过一遍,再交付。
