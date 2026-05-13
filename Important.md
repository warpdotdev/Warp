# 常用命令

```pwsh
.\script\windows\bundle.ps1 -Channel oss -Arch x64 -ReleaseTag "$(git describe --tags --always)"

.\script\create_github_release.ps1 -AssetPath .\script\windows\Output\WarpOssSetup.exe -DryRun
```

# Local OpenAI Web Search 约定

`app/src/ai/agent/api/local_openai` 现在支持 OpenAI Responses API 的 built-in `web_search` tool，但这条能力不是通过 Warp 的 `ToolType` / `tool_calls.rs` 内建函数分发实现的，而是直接把 `{ "type": "web_search" }` 放进 Responses 请求的 `tools` 里。后续如果再看这块，不要去 `built_in_tool_schema()` 或 `parse_tool_call()` 里找 `web_search` 的 function tool 分支。

`response.completed.output` 不能当成 web search 的主数据源。实际接入里，大部分关键 output item，尤其是 `web_search_call`，可能只会在 SSE 过程中通过 `response.output_item.done` 出现；`response.completed` 更适合作为兜底，而不是主路径。以后如果发现“UI 有流式输出但 completed 里是空的”，先检查 SSE 处理，不要先怀疑上层逻辑。

对 assistant 文本和 citations，最终真相也应该以 `response.output_item.done` 为准，而不是只依赖 `response.output_text.delta`。`delta` 负责让 UI 尽早看到文本，`output_item.done` 负责补齐最终文本、annotations、citations；否则很容易出现正文有了但网页引用丢失的情况。

如果继续使用 `store: false` 的本地手动回放模式，上一轮的 `web_search_call` 应该回传给 OpenAI。原因不是“没有它一定报错”，而是为了尽量复刻 Responses API 的真实会话状态，避免模型丢失“上一轮已经搜索过什么”的轨迹，减少重复搜索和上下文漂移。

回放时不要只保存 Warp UI 层抽象后的消息，尽量保留可重建的 Responses output item 语义。当前已经把 assistant `message`、`reasoning`、`function_call`、`web_search_call` 都纳入了 replayable history；其中 assistant `message` 还需要连 `output_text.annotations` 一起保留，否则网页 citations 无法在下一轮完整回放。

当 `response.completed.output` 为空时，不能只靠已有的文本缓存兜底，因为那样会丢 output item 的类型信息和顺序。现在的做法是按 SSE 到达顺序记录 replayable history item，再在 finalize 阶段优先从这份记录回放；后续如果新增新的 Responses output item，也优先接入这套 replay 机制，而不是临时拼字符串。

`response.web_search_call.searching` 目前主要用于 UI 上先展示 searching 状态，真正需要持久化回放的是最终完成态或失败态的 `web_search_call`。也就是说，searching 阶段的即时状态可以更新 UI，但长期上下文里更重要的是最终 `output_item.done` 产出的结果。

# Windows OSS 安装包约定

本地如果要产出和 `.github/workflows/release_master_from_upstream_stable.yml` 那条 OSS release 流程同类型的 Windows 安装包，不要直接猜 `cargo bundle` 或手搓 `ISCC` 参数，优先走 `script/windows/bundle.ps1 -Channel oss -Arch x64`。这条链路和 CI 对齐，最终产物名就是 `script/windows/Output/WarpOssSetup.exe`。

CI 的 Windows release 实际是两段式调用同一个脚本：先 `-SkipBuildInstaller` 编二进制，再 `-SkipBuildBinary` 组装 installer；但本地如果只是想拿到可安装的 `WarpOssSetup.exe`，单次运行 `bundle.ps1` 就够了。是否签名不是这条 OSS 本地构建的重点，默认不签名也和当前 OSS release workflow 保持一致。

OSS 渠道当前采用保守改名：Linux `.desktop`、Windows 快捷方式/启动项、以及 macOS plist 显示名可以显示 `Warp Refined`，但 macOS 的 bundle 文件名、DMG 文件名和其他内部兼容标识仍保留 `WarpOss`。后续如果再处理这类改名，先区分“显示名”和“产物/内部名”，不要一次性全改。

# Local OpenAI Responses API 约定

本地 `local_openai` 后端要显示 Warp 里的 thinking / `Thought for N seconds`，关键不是“拿到最终 answer”就够了，而是要主动请求并解析 reasoning 流。请求侧对 reasoning 模型补 `reasoning.summary = "auto"`，流式侧同时接 `response.reasoning_summary_text.*`、兼容 `response.reasoning_text.*`，并把完成时长落到 Warp 的 `AgentReasoning`，这样 UI 才能稳定显示思考过程和耗时。

如果发现 Local OpenAI 在普通终端能用、切到 SSH Agent 就失效，先检查 `app/src/ai/agent/api/impl.rs` 里的 `should_use_local_openai_responses_backend`。这里以前把 `WarpifiedRemote` session 全排除了，导致 SSH 场景不会走本地 provider，而会落回服务端请求链路；服务端链路又不会使用本地配置的 `openai_base_url`，所以看起来就像“SSH 下 Local OpenAI 坏了”。后续如果再改这条 gating，记得同时保住远端 session 的单测。

`encrypted_content` 不是可有可无的附带字段；在 `store: false` 的本地手动回放模式里，它是多轮 reasoning / tool use 续上下文的关键。请求里要带 `include: ["reasoning.encrypted_content"]`，回放时要把 `reasoning` item 连同 `encrypted_content` 一起传回去，而且不要回传 `id`；`summary` 即使是空数组 `[]` 也不能省略。

不要只信 `response.completed.output`。实际 provider 可能会在 SSE 的 `response.output_item.done` 里给出 reasoning 和 `encrypted_content`，但在最终 completed 里漏掉它；如果只在 completed 阶段建 history，下一轮就会丢推理上下文。更稳妥的做法是流式阶段先按顺序缓存 replayable history，再在 finalize 时和 completed 输出合并。

`prompt_cache_key` 在这条链路里更适合直接用 Warp 自己的 `conversation_id` 字符串，而不是根据 prompt 内容再算一层 hash。除此之外，后面确认过真正让缓存更稳定命中的关键，是把同样的值再放进请求头 `Session_id`；只在 body 里带 `prompt_cache_key`，命中率未必稳定。

如果怀疑 `local_openai` 里有“只剩测试在用”的死函数，不要只搜 `foo(` 这种直接调用。这个目录里有些 helper 会以 `map(foo)`、`filter_map(foo)`、`or_insert_with(foo)` 这类函数指针形式出现在生产代码里，先做目录级引用检查，再决定删不删。

Windows 上跑 `cargo test local_openai --lib` 时，如果卡住或异常，不一定是这块逻辑坏了。这个仓库的 `app/build.rs` 可能会因为 `target/debug/conpty.dll` 被占用而失败或超时；遇到这种情况，先排查是否有正在运行的 Warp 开发实例或别的进程锁住了这个文件。

本地 `local_openai` 这条路的定位一直是“本地 provider 调用 + `ResponseEvent` 兼容层”，不是把 Warp 现有的工具执行 loop 整套搬出来重写。也就是说，尽量复用 Warp 自己的 `ClientActions -> tool_call_result -> 下一轮请求` 链路；只有在必须和 OpenAI Responses 协议对接的边界处，才自己做适配。

本地新会话第一轮一定要先发 `CreateTask`，再发后续消息；如果直接上 `AddMessagesToTask`，Warp 侧会报 `TaskNotInitialized / Task not found / Exchange not found`，UI 还会一直停在 `warping`。以后如果再看到“回答似乎出来了，但状态死活不结束”，先检查事件顺序，不要先怀疑模型或网络。

Responses 的流式事件解析要以 OpenAI 官方文档为基线，但实现上必须对 `OpenAI-compatible` 后端保持宽容。像 `call_id`、`name`、甚至部分最终 output item，兼容后端可能不会在第一条事件里就给全；缺字段时优先缓冲状态，等 `response.output_item.done` 或 `response.completed` 补齐，而不是立刻把整条流打成内部错误。

`response.completed.output` 不能当成唯一真相。实际接入里，assistant 文本、tool call 元数据、reasoning 甚至 web search 相关 item，都可能只在 SSE 过程中完整出现；`completed` 更适合作为兜底和收尾，而不是唯一数据源。以后如果“前面都正常，最后突然报 internal error”，优先排查 finalize 阶段是不是对 completed 过度乐观。

Warp 原项目对 built-in tools 没有现成的 OpenAI function schema 存储，后续 agent 不要再去找一个并不存在的“官方 schema 表”。built-in schema 需要从 `ToolType`、`task.proto`、以及 action/convert 逻辑推出来；反过来，MCP tools 是有现成 `description + input_schema` 的，这部分应该直接复用，不要手写一份平替。

`strict` 不能默认全开。只要一个 tool schema 里存在可选字段，或者嵌套 object 的 `required` 不能覆盖全部 `properties`，OpenAI 的严格校验就可能直接拒绝这条 tool 定义；当前约定是：确实满足 strict 约束的 built-in tool 才开 `strict: true`，有可选字段的工具宁可关掉 strict，也不要为了“看起来更规范”把请求直接打挂。

`app/src/ai/agent/api/local_openai/tool_schemas.rs` 里的 built-in tool schema 需要优先向 Warp 官方实际产出的 function schema 靠拢，而不是只围着本地 proto 旧字段打转。这次已经把 `run_shell_command`、`read_files`、`file_glob`、`read_skill` 等描述和参数形状往官方口径收拢，同时在 `tool_calls.rs` / `request.rs` 保留了必要的兼容解析与序列化；以后再改这块，尽量成套同步 schema、parse、serialize，不要只改一边。

`local_openai` 里的 built-in schema 不能为了“向官方名字看齐”就随便引入 alias tool 名。当前约定是：如果仓库里没有同名本地 tool，就不要在 `tool_schemas.rs` 里硬加一个官方别名；对于功能相近但名字不同的本地 tool，只参考官方 schema 去完善字段说明和参数约束，不额外扩一层 alias 映射，避免平白增加上下文和维护面。

`release_master_from_upstream_stable.yml` 这条 OSS 发布入口现在单独暴露了 `build_cli` 开关，而且默认是 `false`。如果只是想从 `master` 发 OSS 图形端安装包，不要默认以为 Linux/macOS 的 CLI 会跟着一起构建；只有显式打开 `build_cli`，`create_release.yml` 里的 macOS/Linux CLI jobs 才会跑。

`ask_user_question` 在 `app/src/ai/agent/api/local_openai` 里回传给 Responses API 的 `function_call_output.output`，不能只用 `Ask user question completed with N answer(s)` 这种摘要字符串。当前约定是至少对这个 tool 回传结构化 JSON 字符串，把 `status`、`answers`、`question_id`、`selected_options`、`other_text`/`skipped` 带回去，否则模型下一轮拿不到用户真实选择，只能看到“提问完成了”。

评估 `app/src/ai/agent/api/local_openai/request.rs` 里的 `serialize_tool_result_output` 时，不要默认除了 `ask_user_question` 之外其他 tool 都能直接 `to_string()`。仓库里其实已经有一份更成熟的“哪些结果要结构化序列化”的先例：`app/src/ai/agent/conversation_yaml.rs` 的 `write_tool_call_result_content`。像 `read_files`、`search_codebase`、`read_skill`、`read/edit/create_documents`、shell command 相关结果、MCP 结果等，`Display` 往往只有摘要或 debug 文本；如果要补齐 local_openai 的 tool output，优先参考这份现成分类，而不是逐个拍脑袋判断。

`Use local OpenAI-compatible backend` 打开后，不要再把“当前所选模型的 provider 必须是 OpenAI”当成前置条件。现在这条开关的语义已经变成“所有 Warp Agent 请求都优先走本地 `/v1/responses`”，无论 UI 里选的是 GPT、Claude 还是 Gemini；后续如果再看到只有 GPT 能走本地 provider，先检查 `app/src/ai/agent/api/impl.rs` 里是不是又把 provider gating 加回去了。

如果本地兼容后端不接受 Warp 内置模型 ID，不要急着去改整个模型选择器。当前约定是在 `ApiKeyManager` 的 secure storage 里维护一个 `local_openai_model_override`，由设置页的 `OpenAI-Compatible Model ID` 输入框写入；`app/src/ai/agent/api/local_openai/request.rs` 会在构请求时优先用这个 override，再回落到 Warp 当前选中的模型 ID。这样既保住现有 profile / picker 逻辑，也能兼容自建网关或聚合网关的自定义模型名。

设置页的 API Keys 区块现在按 `Use local OpenAI-compatible backend` 开关分两套显示：开启时显示 `OpenAI API Key`、`OpenAI Base URL`、`OpenAI-Compatible Model ID`，并隐藏 `Anthropic API Key` / `Google API Key`；关闭时保留 `OpenAI API Key`，同时显示 `Anthropic API Key` / `Google API Key`，并隐藏 `OpenAI Base URL` / `OpenAI-Compatible Model ID`。后续如果再调整 BYOK UI，先保住这套条件显示关系，不要把两组配置重新同时摊在页面上。

`warp-oss` 现在走 GitHub Releases 形式的自托管更新源：客户端从 `releases/latest/download/channel_versions.json` 读最新 manifest，再从 `releases/download/<tag>/...` 下载各平台安装包。后续如果再改 OSS 自动更新，优先保住这两个约定，同时别漏掉 `.github/workflows/release_refined.yml` 里对 `channel_versions.json` 这个 release asset 的发布步骤。

对 OSS / 自托管更新源来说，`channel_versions_url` 不能只当成“失败后的兜底回退”。只要当前渠道显式配置了 `channel_versions_url`，客户端就应当直接绕过官方 `/client_version` / `/client_version/daily`，优先从自定义 manifest 拉版本信息；另外安装包文件名也要以 workflow 打包脚本为准：Windows 的 `script/windows/bundle.ps1` 用 `APP_NAME='WarpOss'` 产出 `WarpOssSetup(.exe)`，macOS 的 `script/macos/bundle` 用 `WARP_APP_NAME='WarpOss'` 产出 `WarpOss(.dmg)`，所以自动更新里的 OSS 资产基名也必须保持 `WarpOss`，不要写成 `warp-oss`。

`app/src/ai/agent/api/local_openai/request.rs` 里的 `normalize_responses_endpoint` 现在把末尾形如 `/vN` 的版本段都视为“已经带版本前缀”，会直接拼成 `.../vN/responses`；只有完全没带版本段时，才默认补 `/v1/responses`。以后如果再改这块，先保住“识别任意数字版本后缀，而不是只认 `/v1`”这个约定。

`app/src/ai/agent/api/local_openai/request.rs` 在把 `AIAgentContext::Block` 序列化进用户上下文时，会把 `block.output` 控制在最多 20000 个字符；如果真的发生截断，尾部还会追加一段 `[truncated: shell block output exceeded 20000 characters]` 提示。这里的目标是防止单个 shell block 输出把 prompt 撑得过大；如果后续觉得上下文“莫名少了一截”，先确认是不是命中了这条上限和尾注逻辑。

Warp 原版对“并行工具调用”的支持要分两层看：协议层会声明 `supports_parallel_tool_calls: true`，`local_openai` 这边也会发 `parallel_tool_calls: true`；但真正的执行层只把 `read_files`、`search_codebase`、`read_skill` 以及部分可并行的 `grep/file_glob` 归到并行 phase。`run_shell_command` 走的仍是通用串行命令执行链路，而且一旦某个命令进入 long-running snapshot，剩余 pending 的命令类 action 还会被取消。以后如果再看到“一次回两个 shell tool calls，只跑了一个”，先按 Warp 原版执行约束排查，不要先怀疑 local_openai 自己单独实现坏了。

基于上面这个执行约束，`app/src/ai/agent/api/local_openai/request.rs` 里当前显式把 Responses 请求的 `parallel_tool_calls` 关成了 `false`。这是个有意的保护措施，不是遗漏：目的是避免兼容后端一次返回多个 `run_shell_command` 时，Warp 原版命令执行链把 action 标成 cancelled、同时还可能残留 agent-control active block，进而出现输入框消失、要靠 `Ctrl+C` 才恢复的异常体验。

`local_openai` 对 fork / restore conversation 的支持不能只看“task messages 有没有复制过去”。真正影响多轮质量的是 reasoning replay：现在约定把可回放的 Responses reasoning item JSON 存进 `AgentReasoning` message 的 `server_message_data`，restore 时再优先从这里还原 `{"type":"reasoning","encrypted_content":...}`。后续如果再改 `app/src/ai/agent/api/local_openai/stream.rs` 或 `request.rs` 里的 reasoning 流程，记得同时维护这份持久化 payload；否则 UI 虽然还能看到 thinking 文本，但 fork/restore 后 provider 侧的 reasoning 上下文还是会丢。

NLD 相关的 Cargo feature 现在以 upstream 的 `nld_classifier_v1` / `nld_classifier_v2` 为准，不要再把旧的 `nld_onnx_model` 或 `nld_improvements` 加回 `app/Cargo.toml` 或打包脚本里。当前这两个旧名字在仓库里已经没有有效定义，重新带回来会直接让 `cargo metadata` 或打包构建失败。
