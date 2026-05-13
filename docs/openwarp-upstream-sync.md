# openWarp 上游同步指南

## 当前同步状态

| 阶段 | 上游范围 | commit 数 | 已合入 | 永久黑名单 | 暂不评估 | 完成日期 |
|---|---|---:|---:|---:|---:|---|
| 第一轮(初始) | `0443f3f..e089051` | 138 | 81 | 57 | 0 | 2026-05-04 |
| **第二轮(终端+安全)** | **`e089051..898336e3`** | **151** | **15** | **3** | **133** | **2026-05-08** |

第二轮采用**主题分批策略**:本轮只处理**终端 + 安全**主题(15 个 cherry-pick + 3 个永久黑名单),其余 133 个非终端非安全 commit 暂不评估,留给后续按主题分批合并(AI/Agent / 编辑器 / Notebook / Bootstrap / 其他)。

**所有已评估 commit 100% 明确归属**,黑名单写在下方表格,后续 sync 用同一份判断标准。

### 2026-05-04 增量同步(03ef4d0..e089051,新增 5 条)

| Commit | 标题 | 处理 |
|---|---|---|
| `e089051` | fix: point OSS desktop entry at package launcher (#9424) | 黑名单(空 commit;与 `6eefa4b` 同类,openWarp 用 `warp-oss` 命名) |
| `3ce4239` | Remove blocklist markdown images from preview flags (#9993) | 已合入(干净 cherry-pick) |
| `d7c45ca` | enable tab dragging between windows for internal warp users (#9991) | 已合入(手工只取 `DragTabsToWindows`,丢弃上游试图引入的 `CloudModeInputV2` 与重复 `SshRemoteServer`) |
| `525dfb6` | Spec: per-tab theme overrides driven by directory and launch configurations (GH478) (#9910) | 黑名单(纯上游内部 spec 文档) |
| `cabd329` | docs: replace Becoming a Collaborator with Code Review section (#9982) | 黑名单(上游 Oz review 流程,zerx-lab fork 不适用) |

### 2026-05-08 增量同步(`e089051..898336e3`,新增 151 条)

**策略**: 主题分批 — 本轮只处理"**终端 + 安全**"两类。其余主题(AI/Agent、编辑器、Notebook、Bootstrap、CI/文档、UI 杂项 等)留给后续轮次按主题分批评估。

#### 已合入(15 条,按 cherry-pick 顺序)

所有候选已逐 diff 核对 `main` 上对应代码 + 无云端依赖。

| # | Commit | PR | 主题 | 说明 |
|--:|---|---|---|---|
| 1 | `71edcac8` | #9624 | 终端 | PageUp/PageDown 在 prompt 处滚动终端输出,改为可绑定快捷键 |
| 2 | `a9a5b6af` | #9987 | 终端/Windows | restored block 关闭 reset grid checks 防 panic |
| 3 | `c65ae255` | #9891 | 终端/Windows | quake mode 窗口正确 sized 且接收 focus |
| 4 | `b7dd0ef8` | #9448 | 终端 | 拖选超出可视区时自动滚动(z-index 绕过) |
| 5 | `b9e21940` | #10186 | 编辑器/AI 稳定性 | V4A diff 重叠 panic — 影响 BYOP apply diff |
| 6 | `69638b8f` | #10057 | 终端 find | 输出流式追加时焦点漂移修复 |
| 7 | `3ff78d29` | #9730 | 终端/IME | macOS 日语 IME Enter 不误提交表单 |
| 8 | `74672609` | #10241 | 终端/race | 并行对话中 requested commands 被误取消 — `controller.rs` 自治区,手工带 2 行 |
| 9 | `c28fdddb` | #10305 | 终端 | RowIterator 在 CLI agent 下 grid resize 崩溃(防御性) |
| 10 | `fc1157e0` | #9711 | 终端/IME | macOS 第三方 IME(超注音/Yahoo Bopomofo)compose 时方向键双触发 |
| 11 | `543d54ec` | #9602 | 终端/IME | Linux/Wayland 启用 IME — CJK 用户必需 |
| 12 | `4dbf8758` | #10004 | 终端 | tree 命令输出文件名识别为 file link |
| 13 | `5aec7009` | #9981 | **安全** | actix-http 3.12.1 → GHSA-xhj4-vrgc-hr34(本地 `cargo update -p actix-http --precise 3.12.1` 等价上游 `c68b9775`,因 BYOP 导致 Cargo.lock 分叉) |
| 14 | `9af4629d` | #10263 | **安全** | diesel → GHSA-h5x4-m2qf-r4f2(干净 cherry-pick `9d9972cb`) |
| 15 | `2a48032e` | #10060 | **安全** | rand 0.9.4 → GHSA-cq8v-f236-94qc(`cargo update -p rand@0.9.1 --precise 0.9.4` 等价上游 `64a0dfbe`) |

附:`9cb749b8` chore — cherry-pick `c65ae255` 时附带的 `specs/CODE-1787/` 上游产品文档已删除。

#### 永久黑名单(3 条 — 谨慎评估永久排除,后续 sync 不再重复检查)

| Commit | 标题 | 永久排除原因 |
|---|---|---|
| `a639d761` | Fix deadlock in terminal view rendering (#10308) | 修复目标 `is_cloud_agent_pre_first_exchange` 的"加锁参数"在 OpenWarp 基线就**没有**(上游另一条已合 commit 引入,OpenWarp 没合)。即 OpenWarp 不存在此 bug。且改 `app/src/terminal/view/ambient_agent/**` 自治区。 |
| `361c267a` | Implement full-frame clear for active block CLI Agents (#9877) | **行为变化**而非修复(改变 grid `ClearMode::All` 行为)。其引入的回归才是 `c28fdddb` 修的 — 我们只取 `c28fdddb` 防御性 fix。 |
| `9eaa55f7` | remove block attachment in has_locking_attachment (#10416) | **行为变化**而非修复("block 不再算 locking attachment",上游 commit message 自述"as a quick fix to unblock release")。涉及 `BlocklistAIInputModel`/NLD 路径,与 OpenWarp BYOP 输入策略可能冲突。 |

#### 暂不评估(133 条 — 待后续主题轮次)

非"终端/安全"主题的 133 commit 留待按主题分批评估,**不视为永久排除**。下次 sync 应针对以下主题逐个评估:

- AI / Agent / CLI agent / MCP / harness 相关(部分可能合入)
- 编辑器 / 代码视图 / Notebook / 设置
- Bootstrap / 跨平台编译 / FreeBSD / Xcode / .deb / Windows installer
- 文件类型识别(.h++/.htm/.command/.yml 等)
- UI 杂项(file picker / file tree / command palette 等)
- 文档 / 内部 STAKEHOLDERS / CI / README / spec 文档(基本默认黑名单)

## 一次性配置(每个 clone)

```bash
bash script/setup-merge-drivers.sh
```

这会启用 `rerere`(记忆冲突解析)+ 注册 `openwarp-ours` 合并驱动(`.gitattributes` 中标记的路径永远保留 openWarp 版本)。

## 已知"上游不再合并"的 commit 黑名单

下列 commit 已评估,在 openWarp 中**永久跳过**,后续 sync 时不需要再评估:

> **为什么不能用 `merge=openwarp-ours` 路径排除来代替?** 已实验验证:把 ai/agent_sdk/、blocklist/、ambient_agent/、slash_commands/ 等路径加进自治区后,这 10 个 commit 物理上能 cherry-pick(冲突自动消化),**但新增文件**(codex.rs / wake_driver.rs / orchestration_event_streamer 等)会引用自治区中已不存在的字段、enum 变体、trait 方法,导致 **85+ 编译错误**。修这些错误需要逐个补回 openWarp 已删的 cloud/orchestration API,得不偿失。所以保持 commit 级黑名单。

| Commit | 标题 | 跳过原因 |
|---|---|---|
| `b59e351` | add /continue-locally slash command | 依赖 cloud Oz handoff(`conversation_is_cloud_oz_for_slash_command` 已删) |
| `9551831` | Initial codex CLI harness setup | `load_conversation_from_server` 在 openWarp 已 stub 为 None |
| `70c725f` | Conversation resuming for codex | 依赖 9551831 的 cloud 加载链路 |
| `2bdbb61` | Save and upload codex conversation transcript | cloud 链路 |
| `5c89948` | add hook for file editing | snapshot DeclarationsWriter 走 OzHandoff cloud 链路 |
| `1148ae3` | Wake up remote Claude Code agents on new events | cloud agent orchestration,5 处冲突且本质 cloud-tied |
| `6995005` | Scope orchestration SSE subscriptions | cloud orchestration SSE 流,openWarp 无 |
| `1314819` | Merge org and user command denylists | UI 重写,与 openWarp i18n + render_list_section 路径完全不同 |
| `fd8e0fb` | preserve user query modes in CloudMode | CloudMode UI,openWarp 已删 cloud 路径 |
| `71054d6` | Remove `NotAmbientAgent` state | 大型 ambient_agent 重构,32 处冲突,openWarp 已分叉 |
| `99f80df` | Fix bad merge for remote server(替代 SSH 对齐分支) | 已通过整族对齐(f0c8b7f→b19866a→99f80df→e75b315)合入,单独路径过时 |
| `6eefa4b` | OSS .desktop align Exec | openWarp 用 `warp-oss` + `OpenWarp`,与上游 `warp-terminal-oss` 命名分叉 |
| `4dddda6` | Preseed auth and trust settings for codex CLI | codex CLI harness 是 cloud-tied,openWarp BYOP 不需要 |
| `5762baa` | feature flag + API binding scaffolding for cloud→cloud handoff | cloud→cloud 编排,openWarp 已删 cloud_conversations |
| `0ab9e71` | Orchestration pills bar in Agent View (1/N) | orchestration UI,依赖已删 orchestration_event_streamer |
| `88930cf` | Cache settings schema between Linux builds | openWarp 用自己的 openwarp_release.yml |
| `99b287f` | ci: simplify external contributor check | openWarp 自有 workflow |
| `0fca61d` | ci: label external-contributor PRs | openWarp 自有 workflow |
| `805b3e2` | Increase timeout for linux builds | openWarp 自有 workflow |
| `404bfbe` | ci: remove workflows now served by Vercel webhook | openWarp 不接 Vercel webhook |
| `874a257` | Add stakeholders for `lsp` and `languages` crates | Warp 内部 code owners 治理,与 fork 无关 |
| `d1601f5` | add stakeholders(vertical tabs / tab configs / worktree / notifications / rich input) | 同上 |
| `67b929c` | Add @harryalbert as CLI agent stakeholder | 同上 |
| `33c4885` | Add vkodithala as co-owner of skills/MCP/long-running commands | 同上 |
| `a12d9e4` | Add more UI framework stakeholders | 同上 |
| `182c1ac` | chore: assign / route to @warpdotdev/oss-maintainers in STAKEHOLDERS | 同上 |
| `73074ba` | remove @moirahuang from context chips stakeholders | 同上 |
| `1849795` | Point stable-skill instructions at resources/bundled/skills/ | Warp 内部 stable channel 用,openWarp 走 oss channel 无关 |
| `bb5edc0` | Drop warp-internal references from docker/linux-dev README | 内部 dogfood docker 文档,openWarp 不用 |
| `33c4860` | Update env_vars README to match current file layout | 上游 README 内部路径调整 |
| `b740b82` | Update persistence README paths to crates/persistence | 同上 |
| `799e13f` | docs: simplify PR template for public contributors | Warp 自己的 PR 模板 |
| `6898ac2` | docs: surface #oss-contributors Slack channel | Warp 自己的 Slack |
| `ed0cdae` | docs: attribute Alacritty/vte derivative code(2 more files) | 上游 license 归属 |
| `a8f57a8` | Clarify `alacritty_terminal` origins for terminal model code | 同上注释类 |
| `7784428` | Remove stray backticks from Windows installer README | Warp Windows installer README,openWarp 用 openwarp_release.yml 自己生成 |
| `b7c64bc` | Add Build Status section linking to build.warp.dev | build.warp.dev 是 Warp 内部 dashboard |
| `acb2fc6` | Add telemetry events for git button clicks | telemetry 事件,openWarp 不上报到 Warp 后端 |
| `d0f045c` | Auto oss vs cost efficient 50/50 A/B test | Warp 实验框架 + 计费路径 |
| `79df582` | Initialize privacy settings from `WarpDrivePrivacySettings` | WarpDrive 是 cloud,openWarp 不接 |
| `899d966` | Show all personal runs in the conversation list | cloud personal runs,需要 server 支持 |
| `9eaee8f` | Add experiment setup for SSH | 实验框架 |
| `4ac7378` | Rename Warp Agent to Warp | cloud "Warp Agent" 品牌,openWarp 用本地名 |
| `e058136` | Slash command menu working(cloud mode input v2) | cloud_mode_v2_view 已删 |
| `199cd94` | Slash command menu sidecar(cloud mode input v2) | 同上 |
| `9b3a990` | Enabled cloud mode input v2 on dogfood | 同上 |
| `157f358` | Introduce `/harness` `/host` `/environment` slash commands | cloud mode 新命令,openWarp 删 cloud_mode_v2 |
| `aa2ac33` | Skip onboarding UIs in SDK/headless mode | SDK / headless 是 cloud-tied 模式 |
| `0ac090c` | [REMOTE-1326] Link shared sessions to local interactive Oz runs | Oz orchestration |
| `10ec3d1` | Hide host selector menu if no default host present | cloud host selector,openWarp 无 |
| `ac493e6` | Auto-open rich input for non-Oz harness cloud agent sessions | cloud agent |
| `6184f4e` | Refactor AmbientAgentViewModel to handle follow-up run executions | 自治区核心,与 71054d6 同代次重构 |
| `f696f5b` | Revert "Fix schema generator binary recompilation" | 上游回滚一个 commit,openWarp 没合那个原 commit |
| `159a0bf` | ci: remove broken oz-for-oss adapter workflows | Warp 内部 workflow |
| `59fc1a9` | use multi-harness cloud agent icons + status | cloud agent UI |
| `e089051` | fix: point OSS desktop entry at package launcher (#9424) | 空 commit;与 `6eefa4b` 同类,openWarp 用 `warp-oss` 命名分叉 |
| `525dfb6` | Spec: per-tab theme overrides (GH478) (#9910) | 纯上游内部 spec 文档 `specs/GH478/*.md`,与 openWarp 无关 |
| `cabd329` | docs: replace Becoming a Collaborator with Code Review (#9982) | 上游 Oz review 流程,zerx-lab fork 无 Oz |

## openWarp 已删除/特化的模块(合并时若被恢复,需手工删除)

| 模块 / 路径 | 删除原因 | 处理方式 |
|---|---|---|
| `cloud_conversations` 全家桶 | openWarp BYOP 不接 Warp 云 | 上游若新增此目录文件,直接 `git rm` |
| `app/src/server/cloud_objects/**` | OpenWarp 不接云对象 RTC/初始加载/服务端 fan-in;本地写入入口迁到 `app/src/cloud_object/update_manager.rs` | 上游若恢复此目录,直接 `git rm`;保留 `server/mod.rs` 内兼容转发模块 |
| `app/src/server/server_api/object.rs` | `ObjectClient` 云对象 RPC 已物理删除,本地对象 create/update 只走 CloudModel/SQLite | 上游若恢复此文件或 `get_cloud_objects_client()`,直接删除并保留本地路径 |
| `app/src/workspaces/update_manager.rs` | `TeamUpdateManager` 云端 workspace metadata/team polling 壳已物理删除;OpenWarp 的 agent SDK metadata refresh 是本地立即成功,Drive workspace 切换直接写 `UserWorkspaces` | 上游若恢复该文件或 `TeamUpdateManager` singleton,直接删除并改调用点走本地 `UserWorkspaces` / no-op |
| `app/src/workspaces/team_tester.rs` | 该 singleton 只负责触发旧 cloud object / workspace metadata pollers,在 OpenWarp 本地化后无本地语义 | 上游若恢复 `TeamTesterStatus` 或 `InitiateDataPollers` 事件,直接删除 |
| `app/src/terminal/shared_session/sharer/**` + `app/src/terminal/shared_session/viewer/{network,event_loop,terminal_manager}.rs` + `app/src/terminal/shared_session/network/**` | shared-session RTC/WebSocket 协议层与 heartbeat network 壳已删除;OpenWarp 不创建、恢复或观看云端 shared session | 上游若恢复这些协议模块、`Network` 模型、heartbeat 或 session-sharing endpoint 连接,直接删除;仅保留本地兼容展示/历史壳直到后续 shared-session 命名收尾 |
| `app/Cargo.toml` 中 `session_sharing` / `ambient_agents_rtc` features | 旧 cloud shared-session / ambient RTC feature 开关已无代码消费,OpenWarp 不再暴露这些构建面 | 上游若恢复 feature 或 default feature 条目,先 grep 消费点;无本地语义则删除 |
| `crates/warp_core/src/channel/{config,state}.rs` 中的 `WarpServerConfig` / `OzConfig` / `TelemetryConfig` | OpenWarp 不接 Warp server / Oz workload identity / telemetry 发送配置,channel state 直接返回本地禁云语义 | 上游若恢复这些配置字段或 real endpoint 读取路径,删除并保留 `ChannelState` 的 disabled 常量返回 |
| `crates/warp_core/src/channel/{config,state}.rs` 中的 `CrashReportingConfig` / `ChannelState::sentry_url` | OpenWarp 不接 Warp Sentry DSN;crash reporting 只保留本地 panic/log 能力 | 上游若恢复 channel DSN 字段、`sentry::init` 或 cocoa/minidump 远端初始化,删除或改回本地 no-op |
| `crates/managed_secrets/src/manager.rs::get_task_secrets` workload token 获取 | OpenWarp 的 managed secrets client 是本地 disabled facade,不应在返回空 secrets 前申请 Namespace/Oz workload token | 上游若恢复 `warp_isolation_platform::issue_workload_token(...)` 前置调用,删除并保持 disabled client 直接返回空集合 |
| `crates/graphql/src/api/queries/get_updated_cloud_objects.rs` / `get_oauth_connect_tx_status.rs` | 云对象更新轮询与 OAuth cloud poll 已删除 | 上游若恢复,直接 `git rm`;BYOP OAuth 只保留本地禁用语义 |
| AI 回复 footer 点赞/点踩(`render_response_footer` 中的 thumbs up/down) | 移除 telemetry 反馈链路 | 上游若改 output.rs 这段,保留 openWarp 版 |
| 智能体署名 `AgentAttributionWidget` + `AISettings.agent_attribution_enabled` | 不需要 | 上游若修改,丢弃 |
| Oz 更新日志 toggle UI | 仅删 UI/action/keybinding,字段保留 | 同上 |
| `app/src/pane_group/mod_tests.rs` 等 9 个 _tests.rs(b120bbe 配套删除) | 类型已删 | 上游 typo fix 触及时 `git rm` |
| `conversation_is_cloud_oz_for_slash_command` 函数 | cloud_oz 路径已删 | 上游引入时丢弃 |

## 合并流程

1. `git fetch origin master`
2. 创建 worktree:`git worktree add ../warp-merge -b merge-upstream-<date> openWarp`
3. 在 worktree 内:
   - `git log --reverse --oneline openWarp..origin/master` 列出待评估 commit
   - 跳过黑名单中的(若有)
   - 按拓扑顺序 cherry-pick
4. `merge=openwarp-ours` 路径自动保留本地版本,无需手工解决
5. modify/delete 类冲突直接 `git rm`(参考上表)
6. 其它冲突手工解决;rerere 会记下来
7. `cargo check -p warp` 验证后合回 openWarp

---

## openWarp 独有特性:SSH 管理器(2026-05-04 新增)

完整模块,**上游不会有**。所有自治区路径已写进 `.gitattributes` 末尾。

### 整片自治区(merge=openwarp-ours,合并时永远保留我们版本)

| 路径 | 内容 |
|---|---|
| `crates/warp_ssh_manager/**` | 数据层 crate(Diesel CRUD / 类型 / SSH 命令拼装 / keychain wrapper / SecretInjector matcher) |
| `app/src/ssh_manager/**` | UI 层(panel / server_view / secret_injector / notifier) |
| `app/src/pane_group/pane/ssh_server_pane.rs` | SshServerPane(中央 pane,仿 GetStartedPane 极简) |
| `crates/persistence/migrations/2026-05-04-120000_add_ssh_manager_tables/**` | `ssh_nodes` + `ssh_servers` 表初创建 |
| `crates/persistence/migrations/2026-05-04-130000_add_ssh_nodes_is_collapsed/**` | `is_collapsed BOOLEAN NOT NULL DEFAULT 0` 列 |

### "嵌入到上游热点文件"的修改(**非自治区**,sync 时若上游也改了同位置要手工 merge)

下面这些文件 openWarp 改了,但**也在上游主线持续演进**,不能整体进自治区。每次 sync 上游若触及这些位置,需要手工保留我们的 SSH 接入代码。

| 文件 | 我们改了什么 | 关键 anchor |
|---|---|---|
| `app/src/lib.rs` | `mod ssh_manager;` 注册;启动后调 `warp_ssh_manager::set_database_path(...)`;`SshTreeChangedNotifier::new()` 加入 `add_singleton_model` 链 | `mod shell_indicator` 后;`persistence::initialize` 调用之后;`KeybindingChangedNotifier::new()` 旁 |
| `app/src/workspace/view/left_panel.rs` | `ToolPanelView::SshManager` enum 变体 + `LeftPanelAction::SshManager` + `MouseStateHandles.ssh_manager_button` + `LeftPanelView.ssh_manager_view` 字段 + `LeftPanelEvent::OpenSshServerEditor` / `OpenSshTerminal` 变体 + new() 构造 + 12+ 处 match 分支(`update_available_views`/`create_toolbelt_button_config`/`update_button_active_states`/`handle_action_with_force_open`/`View::on_focus`/`focus_active_view_on_entry`/render content_area+mouse_state_handles vec) + ssh_manager_view 事件 subscribe 转发 | grep `ssh_manager` / `SshManager` / `OpenSshServer` / `OpenSshTerminal` 找全 |
| `app/src/workspace/view.rs` | `compute_left_panel_views` 末尾 push SshManager;`restore_active_view_from_snapshot` match;`render_left_panel_button` + `render_tools_panel_button` 两处 ToolPanelView match;`handle_left_panel_event` 处理 OpenSshServerEditor/OpenSshTerminal;`handle_action` 加 `WorkspaceAction::OpenSshTerminal` 分支;新方法 `Workspace::open_ssh_server` 与 `Workspace::open_ssh_terminal` | grep `ssh` / `SshServer` / `OpenSshTerminal` |
| `app/src/workspace/action.rs` | `WorkspaceAction::OpenSshTerminal { node_id, server }` 变体 + `should_save_app_state_on_action` 加 false 分支 | grep `OpenSshTerminal` |
| `app/src/app_state.rs` | `LeafContents::SshServer { node_id }` 变体 + `is_persisted()` 返回 false | grep `LeafContents::SshServer` |
| `app/src/persistence/sqlite.rs` | save 路径两处 LeafContents match 加 SshServer 占位(都 unreachable) | grep `LeafContents::SshServer` |
| `app/src/pane_group/mod.rs` | restore match 加 SshServer arm 返回 Err(类似 NetworkLog,因 is_persisted=false) | grep `LeafContents::SshServer` |
| `app/src/pane_group/pane/mod.rs` | `IPaneType::SshServer` enum + Display + `from_ssh_server_pane_ctx/_view` PaneId 方法 + render() 分支 + `pub(crate) mod ssh_server_pane` | grep `IPaneType::SshServer` / `ssh_server_pane` |
| `app/src/launch_configs/launch_config.rs` | LeafContents match 加 SshServer 到 `Err(())` 分支(SSH pane 不能存 launch config) | grep `LeafContents::SshServer` |
| `app/src/workspace/view/vertical_tabs.rs` | IPaneType match 加 SshServer 到 `TypedPane::Other` | grep `IPaneType::SshServer` |
| `app/src/terminal/recorder.rs` | `pub fn inactive_pty_reads_rx(&self)` getter 暴露(给 SecretInjector 订阅 PTY 用) | grep `inactive_pty_reads_rx` 找方法 |
| `app/src/terminal/view.rs` | `pub fn inactive_pty_reads_rx(&self, ctx)` 包装 pty_recorder | grep `inactive_pty_reads_rx` |
| `crates/persistence/src/schema.rs` | `ssh_nodes` + `ssh_servers` 两个 `diesel::table!` 块 + joinable | 按字母序在 `settings_panes` 与 `tabs` 之间 |
| `crates/persistence/src/model.rs` | 4 个 ORM struct(`SshNodeRow` / `NewSshNode` / `SshServerRow` / `NewSshServer`)+ schema import 列表加 `ssh_nodes, ssh_servers` | 文件末尾;import 块按字母序 |
| `Cargo.toml`(workspace 根) | `keyring 3.6` / `shell-escape 0.1.5` workspace deps;`warp_ssh_manager = { path = "crates/warp_ssh_manager" }` workspace dep | 字母序插入对应位置 |
| `app/Cargo.toml` | `warp_ssh_manager.workspace = true` + `zeroize = "1.8"` 直接 dep | warp_server_client 旁;字母序 |
| `crates/warp_ssh_manager/Cargo.toml` | 自有 dev-dependencies 包含 `libsqlite3-sys = { features = ["bundled"] }`(Windows 测试链接器需要) | (整文件就是我们的) |
| `app/i18n/en/warp.ftl` 和 `app/i18n/zh-CN/warp.ftl` | ~25 条 `workspace-left-panel-ssh-manager-*` key 全部新增 | grep `ssh-manager` |

### 处理建议

- **整片自治区已通过 `.gitattributes` 自动保留**,这部分省心。
- **嵌入式修改的冲突**:重 sync 时若上游改了 `left_panel.rs` 的 `ToolPanelView` 或 `workspace/view.rs` 的 `compute_left_panel_views` 之类,git 会标记冲突。手工解决时**保留我们的 SSH 分支** + **吸收上游对其他分支的更新**。
- 上游不太可能新增同名 `LeafContents::SshServer`/`IPaneType::SshServer`/`WorkspaceAction::OpenSshTerminal` 变体(语义太具体),冲突场景主要是**新增其他变体**到同一个 enum,git 一般能自动 merge。
- **SecretInjector + keychain** 完全本地化,不依赖任何 cloud 路径,sync 上游 cloud 改动时不会被波及。

### 验收

sync 完成后跑一遍:
- `cargo test -p warp_ssh_manager`(16 个数据层单测)
- `cargo check -p warp --lib` 干净
- 启动 openWarp,工具条最末仍能看到 SSH 管理器图标(钥匙图标),点开树结构、folder 折叠、拖拽、连接、密码注入这条 e2e 链路保持工作
