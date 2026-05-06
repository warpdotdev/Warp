# Warp Desktop i18n 翻译进度看板

> **本文档是多 agent 并行翻译的协调中心。** 每个 agent 启动前先读这里认领 surface,完成后更新对应行。
> Source-of-truth locale 是 `en`(必须 100% 完整);其它 locale 缺 key 自动 fallback 到英文,可以分批补译。

## 架构速览

- **加载链**:`app/src/i18n.rs` → `i18n-embed` 编译期嵌入 `app/i18n/{locale}/*.ftl` → `FluentLanguageLoader` 全局单例 → `t!("key")` 宏(返回 `String`,直接喂 GPUI Text/label_text)
- **fallback chain**:用户系统 locale (例 `zh-CN`) → 同语族 (`zh`) → 兜底 `en`,任意一层缺 key 自动下沉
- **每个 surface 一对 .ftl 文件**(`en/<surface>.ftl` + `zh-CN/<surface>.ftl`),用前缀 namespace 隔离 key,避免 agent 间合并冲突
- **`fl!()` 宏在编译期验证** key 存在于 fallback 语言(`en`)→ 调用方写 `t!("key")` 但 key 没加进 `en/*.ftl` 会编译失败,这是好事,等于强制对齐
- **运行时切换**:目前 `OnceLock`,只在启动 init 一次;后续支持 settings 动态切换需重构成 `RwLock<FluentLanguageLoader>`(本期不做)

## 进度状态图例

| 符号 | 含义 |
|---|---|
| ✅ | 完成(en + zh-CN 全译,call sites 全替换,cargo check 通过) |
| 🟡 | 部分完成(en/zh-CN 不齐,或 call sites 未全部替换) |
| ⬜ | 未开始 |
| 🔒 | 已被 agent 认领(in_progress) |
| ➖ | 不适用(纯非 UI 模块,无字符串需要翻译) |

## Surface 清单

| # | Surface | 文件路径 | en 状态 | zh-CN 状态 | call sites | Owner | 备注 |
|---|---|---|---|---|---|---|---|
| 0 | common (基础原子) | `app/i18n/{en,zh-CN}/common.ftl` | ✅ | ✅ | n/a | foundation | 通用按钮/状态文案 |
| 1 | settings (PoC 起点) | `app/src/settings_view/**` | 🟡 (AI + mod nav + about/main + referrals + agent_providers) | 🟡 | mod.rs:31, about/main:21, referrals:24, agent_providers:30 | foundation, agent-settings-mod, agent-settings-about, agent-settings-referrals, agent-settings-agent-providers | AI 页基础 key 已建;mod.rs SettingsSection Display + pane menu + debug 已替换;about_page.rs(1 key/1 cs)+ main_page.rs(20 key/20 cs);referrals_page.rs(28 key/24 cs);agent_providers_widget.rs ✅ (33 key/30 cs:title/description/empty/add-button/search-placeholder/quick-add-title/refresh-catalog/loading-catalog/catalog-empty/no-match/collapse/expand-remaining/row-missing/field-name/-base-url/-api-key/-api-type/api-type-hint/name-placeholder/api-key-placeholder/models-label/-empty-hint/-header-{name,id,context,output}/model-{name,id,context,output}-placeholder/add-model/fetch-from-api/sync-models-dev/remove)。BYOP 配置 UI 全译;文件中新增的 reasoning chip section(ReasoningEffortSetting,在 i18n 进行中由其他 agent 添加)未在本轮范围内。整 crate `cargo check` 失败因 `app/src/lib.rs` 缺 `mod i18n;`(基础设施 agent 责任,非本任务) |
| 2 | ai 主体 | `app/src/ai/**`, `app/src/ai_assistant/**` | ⬜ | ⬜ | ⬜ | (free) | BYOP / agent / blocklist / mcp 子目录较多,可再拆 |
| 3 | command_palette | `app/src/command_palette.rs`, `app/src/palette/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 4 | drive | `app/src/drive/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 5 | onboarding | `crates/onboarding/**`, `app/src/coding_entrypoints/**` | ⬜ | ⬜ | ⬜ | (free) | 跨 crate 注意:`onboarding` 是独立 crate,要看是否单独建 i18n |
| 6 | workspace | `app/src/workspace/**`, `app/src/workspaces/**` | 🟡 (view.rs + delete_conversation_confirmation_dialog.rs + vertical_tabs.rs) | 🟡 | 20+ | agent-workspace-i18n | workspace-runtime section 已有基础 key；新增版本更新(workspace-version-*)、worktree(workspace-worktree-*)、对话框(workspace-dialog-*)、toast(workspace-toast-*)、溢出计数(workspace-overflow-*)相关 key。call sites:view.rs(版本更新3处+worktree4处+对话框1处+toast8处)、delete_conversation_confirmation_dialog.rs(3处)、vertical_tabs.rs(溢出计数3处)。需验证 cargo check。
| 7 | modal & prompt | `app/src/modal/**`, `app/src/prompt/**`, `app/src/quit_warning/**` | 🟡 | 🟡 | 14 | agent-quit-warning | quit_warning ✅,modal/prompt 待办 |
| 7a | quit_warning | `app/src/quit_warning/mod.rs` | ✅ | ✅ | 14 | agent-quit-warning | 退出/关闭确认对话框 |
| 1b | settings-warpify | `app/src/settings_view/warpify_page.rs` | ✅ | ✅ | 17 | agent-settings-warpify | Warpify 子页(subshell + SSH 配置)。19 key:page-title / description-prefix / learn-more / section-subshells(+subtitle)/ section-ssh(+subtitle)/ added-commands / denylisted-commands / denylisted-hosts / command-placeholder / host-placeholder / enable-ssh / install-ssh-extension(+description)/ use-tmux / tmux-description / ssh-tmux-toggle-binding-label。Category 要求 'static 用 Box::leak 提升 |
| 1c | settings-keybindings | `app/src/settings_view/keybindings.rs` | ✅ | ✅ | 14 | agent-settings-keybindings | 13 key:search-placeholder / conflict-warning / button-default/cancel/clear/save / press-new-shortcut / description / use-prefix / use-suffix / not-synced-tooltip / subheader / command-column。`render_button` 形参从 `&'static str` 升级为 `String`(`Text::new_inline` 接 `Cow<'static, str>` 兼容)。`SEARCH_PLACEHOLDER` 仍以 `pub const` 形式留作 `resource_center/keybindings_page.rs` 的复用入口,等该文件单独 i18n 时迁移。所有 `crate::t!` 调用与其它 settings agent 一致,实际编译通过依赖基础设施 agent 在 `app/src/lib.rs` 注册 `mod i18n;`。 |
| 8 | auth | `app/src/auth/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 9 | workflows | `app/src/workflows/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 10 | editor & search | `app/src/editor/**`, `app/src/search/**`, `app/src/search_bar.rs` | ⬜ | ⬜ | ⬜ | (free) | |
| 11 | terminal | `app/src/terminal/**`, `app/src/shell_indicator.rs` | ⬜ | ⬜ | ⬜ | (free) | |
| 12 | mcp servers | `app/src/settings_view/mcp_servers/**`, `app/src/ai/mcp/**` | ✅ (settings_view/mcp_servers/**) | ✅ | 78 | agent-settings-mcp-servers-subdir | mcp_servers_page.rs ✅(6 key/6 cs);settings_view/mcp_servers/** 子目录 ✅:destructive_mcp_confirmation_dialog.rs(9 key/12 cs)+ edit_page.rs(12 key/13 cs)+ installation_modal.rs(6 key/6 cs)+ list_page.rs(20 key/16 cs:删 3 个 const + LazyLock 改运行时 fragments)+ server_card.rs(14 key/14 cs:tooltip×4/button×3/status×4/tools×2/update-tooltip)+ update_modal.rs(10 key/9 cs:default-name/title/desc/publisher×2/from/version/cancel/update/no-updates)。cargo check -p warp --lib 0 error / 50s。剩余 ai/mcp/** 待认领 |
| 13 | billing & pricing | `app/src/billing/**`, `app/src/pricing/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 14 | notebooks | `app/src/notebooks/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 15 | code_review | `app/src/code_review/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 16 | banner & tips | `app/src/banner/**`, `app/src/tips/**` | ✅ (banner) | ✅ (banner) | 1 | agent-banner | banner 已完成,tips 待认领 |
| 17 | crash_recovery & errors | `app/src/crash_recovery.rs`, `app/src/crash_reporting/**` | ⬜ | ⬜ | ⬜ | (free) | |
| 18 | menu & app_menus | `app/src/menu.rs`, `app/src/app_menus.rs` | ⬜ | ⬜ | ⬜ | (free) | |
| 19 | view_components | `app/src/view_components/**` | ⬜ | ⬜ | ⬜ | (free) | 通用 UI 控件 placeholder/tooltip |
| 20 | misc(其余 single-file) | 见 lib.rs mod 列表 | ⬜ | ⬜ | ⬜ | (free) | 收尾兜底 |
| - | settings-rules-page | drive/items/ai_fact{,_collection}.rs + ai_page.rs:5521 | ✅ | ✅ | 4 | agent-rules-page | Manage Rules 页面。新增 2 key:`rules-collection-name`(Drive 侧 collection 标题,新 ANCHOR-SUB-RULES-PAGE)+ `settings-ai-rules-description`(AI 设置页 rules 段描述,放 ANCHOR-SUB-AI-PAGE)。复用已有 `settings-ai-learn-more`。call sites:ai_fact_collection.rs `display_name`、ai_page.rs rules_description plain_text + hyperlink(2 处)。ai_fact.rs 单条 fact 渲染均为用户数据(name/content),无硬编码字符串需翻译。cloud_object_naming_dialog.rs 无 rule 相关字符串。 |
| - | slash-commands | `app/src/search/slash_command_menu/static_commands/commands.rs` | ✅ | ✅ | 33 desc + 13 hint | agent-slash-commands | 命令面板 `/agent` `/skills` `/profile` 等斜杠命令的 description 与 argument hint_text。新 ANCHOR-SUB-SLASH-COMMANDS。原 `pub const StaticCommand` 全部转 `pub static LazyLock<StaticCommand>`(因为 `description: &'static str` 字段无法在 const ctx 调函数);新增 `t_static!` 宏(`app/src/i18n.rs`)= `Box::leak(t!(...).into_boxed_str())` 一次性泄漏给 `&'static str` 字段使用,仅在 LazyLock init 调一次。`zero_state.rs:48-55` prioritized_commands vec 里 `&commands::CONVERSATIONS/PROMPTS/AGENT` 改 `&*`(LazyLock 不会自动从 `&LazyLock<T>` coerce 到 `&T`)。`all_commands()` 中原 const push 全部改 `.clone()`。`rename_tab_command_requires_argument` 单测加 `crate::i18n::init(Some("en"))` 才能拿到真实 hint。cargo check -p warp --lib 0 error / 92s。 |
| - | keybinding descriptions | binding 注册点 (workspace/mod.rs 等) | ✅ | ✅ | 156 | agent-keybinding-descriptions | binding description 文案。新 ANCHOR-SUB-KEYBINDING-DESC,116 key,聚焦 `app/src/workspace/mod.rs`(workspace 内全部用户可见 description 已替换:FixedBinding::custom + EditableBinding::new + BindingDescription::new + with_custom_description(MAC_MENUS_CONTEXT) + with_dynamic_override 闭包)。`BindingDescription::new` 已经是 `S: Into<String>` 泛型,直接接受 `crate::t!()` 返回的 `String`,无需改 API;`titlecase` 仍会被应用,但中文不受影响。binding `name`(协议字段)未动。**未碰**:`[Debug]/[a11y]/sample_process/dump_heap_profile/crash` 等仅 debug build 出现的工程类条目(对终端用户不可见,刻意跳过减负);其它 binding 文件 — `terminal/view/init.rs`(77 处,terminal binding/agent context 等)、`editor/view/mod.rs`(60)、`notebooks/editor/view.rs`(51)、`code/editor/view/actions.rs`(39)、`pane_group/mod.rs`(14)、`terminal/input.rs`(14) 全部待续作。cargo check -p warp --lib 0 error 27s。 |

## Agent 工作流(拷贝执行)

每个并行 agent 接到 surface 名后按这个流程走:

1. **认领**:在本表 Owner 列写自己的 agent ID + 改状态为 🔒
2. **抽取**:`grep` 该 surface 目录下所有面向用户的硬编码字符串(label/title/tooltip/placeholder/error message)
   - **跳过**:log/telemetry/debug 字符串、enum Display impl(已结构化)、key/setting name(协议字段)
   - **保留**:UI 文案、按钮文字、错误提示、状态文字、对话框标题正文
3. **加 key**:在 `app/i18n/en/<surface>.ftl` 添加新 key,使用 `<surface>-<area>-<purpose>` kebab-case 命名
4. **替换 call sites**:把 `"hardcoded".to_string()` 改成 `crate::t!("surface-area-purpose")`(import path 视模块嵌套调整)
5. **翻译中文**:把同样的 key 加到 `app/i18n/zh-CN/<surface>.ftl`,保持术语一致(见下方"术语表")
6. **验证**:`cargo check -p warp` 通过(因为 `fl!()` 编译期校验 key,缺一个就编译失败)
7. **回写进度**:更新本表对应行 → ✅,在 `Owner` 列填 commit SHA,call sites 列填实际替换条目数

## 术语表(必须统一)

| EN | zh-CN | 说明 |
|---|---|---|
| Agent | 智能体 | warp 自家 + BYOP 通用 |
| Block | 命令块 | warp 核心概念 |
| Drive | 云盘 | warp drive 文件协作产品 |
| Workflow | 工作流 | |
| Notebook | 笔记本 | |
| Profile | 配置 | execution profile / agent profile |
| Permission | 权限 | |
| Prompt | 提示词 | LLM 上下文,非 shell prompt |
| Shell Prompt | Shell 提示符 | 必要时用全称区分 |
| Setting | 设置 | |
| Provider | 提供商 | BYOP / 模型源 |
| MCP Server | MCP 服务器 | 全大写保留缩写 |
| Skill | 技能 | |
| Tool | 工具 | LLM tool calling |
| Command | 命令 | shell command |
| Block List | 命令块列表 | UI 主区域 |
| Pane | 窗格 | terminal pane |
| Tab | 标签页 | |
| Subagent | 子智能体 | |

## 反模式(别这么做)

- ❌ 翻译协议字段名 / setting key / 序列化用的 enum variant —— 它们出现在配置文件和持久化数据里,不是 UI 文案
- ❌ 翻译 log message —— 日志只面向开发者,统一英文减小排错歧义
- ❌ 翻译错误类型 (`anyhow::Error`/`thiserror`) 的 source 链文本 —— 同上
- ❌ 把变量插值用拼接做(`"Hello, " + name`)—— 必须 Fluent `{ $name }`,中文语序可能完全不同
- ❌ 在 `t!()` 调用里传非字面量(动态 key)—— `fl!()` 是编译期验证,只能字面量

## 加新 surface ftl 的清单

当新增 surface 时:

1. 创建 `app/i18n/en/<surface>.ftl` 和 `app/i18n/zh-CN/<surface>.ftl`(前者必须有内容,后者可空)
2. 不需要改 `i18n.toml` 或 `app/src/i18n.rs` —— `RustEmbed` 自动捕获目录下所有文件
3. 在本 PROGRESS.md 表里加一行
