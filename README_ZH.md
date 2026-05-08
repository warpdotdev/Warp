<a href="https://www.warp.dev">
    <img width="1024" alt="Warp Agentic Development Environment product preview" src="https://github.com/user-attachments/assets/9976b2da-2edd-4604-a36c-8fd53719c6d4" />
</a>
&nbsp;
<p align="center">
  <a href="https://www.warp.dev"><img height="20" alt="Built with Warp" src="https://raw.githubusercontent.com/warpdotdev/brand-assets/main/Github/Built-With-Warp-Export@2x.png" /></a>
  &nbsp;
  <a href="https://oz.warp.dev"><img height="20" alt="Powered by Oz" src="https://raw.githubusercontent.com/warpdotdev/brand-assets/main/Github/Powered-By-Oz-Export@2x.png" /></a>
</p>

<p align="center">
  <a href="https://www.warp.dev">官网</a>
  ·
  <a href="https://www.warp.dev/code">Code</a>
  ·
  <a href="https://www.warp.dev/agents">Agents</a>
  ·
  <a href="https://www.warp.dev/terminal">Terminal</a>
  ·
  <a href="https://www.warp.dev/drive">Drive</a>
  ·
  <a href="https://docs.warp.dev">文档</a>
  ·
  <a href="https://www.warp.dev/blog/how-warp-works">How Warp Works</a>
</p>

<p align="center">
  <a href="README.md">English</a>
  ·
  <a href="README_ZH.md">中文</a>
</p>

> [!IMPORTANT]
> 这个仓库包含 **Warp Refined**，它是一个修改版 Warp fork，不是官方上游 Warp 仓库。它基于 Warp，并加入了面向 BYOK 和 OpenAI-compatible 后端的自定义行为，让这些能力对所有用户都更灵活。你可以在 [Linux Do](https://linux.do/) 讨论这个 fork。

<h1></h1>

## 关于

[Warp](https://www.warp.dev) 是一个从终端出发的 agentic 开发环境。你可以使用 Warp 内置的 coding agent，也可以接入自己的 CLI agent，例如 Claude Code、Codex、Gemini CLI 等。

Warp Refined 是一个修改版 Warp 构建，目标是在尽可能贴近上游 Warp 的基础上，放宽上游围绕 BYOK 和 OpenAI-compatible 集成的一些限制。

## Warp Refined 相比官方 Warp 的差异

相比官方上游 Warp 构建，Warp Refined 当前额外提供以下能力：

* BYOK（Bring Your Own API Key，自带 API Key）对所有用户启用，而不是受原本的计费门槛限制。
* Warp Agent 可以使用自定义 OpenAI-compatible `base URL`，因此更容易接入自托管网关、代理或第三方兼容 provider。
* 启用本地 OpenAI-compatible 后端时，Warp Agent 请求可以直接从客户端发送到配置的 `/v1/responses` endpoint，而不是经过 Warp 托管的 `/ai/multi-agent` 服务。
* 多轮本地 OpenAI-compatible 会话会更准确地保留 reasoning context，包括继续工具调用和 reasoning 流程所需的 Responses API encrypted reasoning content。
* 本地 OpenAI-compatible 会话会保留更丰富的可回放历史，包括 assistant message、tool call、reasoning item 以及相关 response output item，从而提升与 Responses API 风格流程的兼容性。
* Warp Refined 新增了应用内显示语言切换设置，并支持中文界面。
* 这个分支会周期性同步上游 Warp 变更，包括近期来自 `byok` 和 `master` 的合并，因此能在保留自定义 BYOK/OpenAI-compatible 行为的同时，继续跟进较新的上游修复和维护更新。

相比许多其他 Warp fork 分支，本项目有意保持更小的改动范围，不引入复杂且无关的功能，优先保证代码质量和日常使用稳定性。
## 安装

你可以[下载 Warp](https://www.warp.dev/download)，并阅读[官方文档](https://docs.warp.dev/)获取不同平台的安装说明。

## Warp 贡献概览面板

你可以访问 [build.warp.dev](https://build.warp.dev)：

- 查看数千个 Oz agents 如何分拣 issue、编写 spec、实现变更以及 review PR
- 查看热门贡献者和正在进行的功能
- 使用 GitHub 登录跟踪自己的 issue
- 在 Web 编译版 Warp 终端中进入活跃的 agent session

## Oz for OSS

你在维护热门开源项目吗？可以[申请 Oz credits](https://tally.so/r/LZWxqG)，了解 [Oz for OSS](https://github.com/warpdotdev/oz-for-oss)。

Oz for OSS 是 Warp 的合作伙伴计划，旨在把这个仓库中使用的 agentic 开源管理工作流带给部分合作仓库。Warp 会直接与维护者合作，以适合项目自身的方式实现 issue 分拣、PR review、社区管理和贡献者协作流程。

## 许可证

Warp 的 UI framework（`warpui_core` 和 `warpui` crates）使用 [MIT license](LICENSE-MIT)。

这个仓库中的其余代码使用 [AGPL v3](LICENSE-AGPL)。

## 开源与贡献

Warp 的客户端代码库是开源的，并位于这个仓库中。Warp 欢迎社区贡献，并设计了一套轻量级流程帮助新贡献者上手。完整贡献流程请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

> [!TIP]
> **与贡献者和 Warp 团队交流**：可以加入 [`#oss-contributors`](https://warpcommunity.slack.com/archives/C0B0LM8N4DB) Slack 频道，这里适合临时提问、设计讨论和与维护者协作。新用户请先加入 [Warp Slack community](https://go.warp.dev/join-preview)，再进入 `#oss-contributors`。

### 从 Issue 到 PR

提交前，请先[搜索现有 issue](https://github.com/warpdotdev/warp/issues?q=is%3Aissue+is%3Aopen+sort%3Areactions-%2B1-desc)，确认你的 bug 或功能请求是否已经存在。如果没有，请使用模板[提交 issue](https://github.com/warpdotdev/warp/issues/new/choose)。安全漏洞应按照 [CONTRIBUTING.md](CONTRIBUTING.md#reporting-security-issues) 中的说明进行私下报告。

Issue 提交后，Warp 维护者会进行 review，并可能添加 readiness label：[`ready-to-spec`](https://github.com/warpdotdev/warp/issues?q=is%3Aissue+is%3Aopen+label%3Aready-to-spec) 表示设计已开放给贡献者编写 spec，[`ready-to-implement`](https://github.com/warpdotdev/warp/issues?q=is%3Aissue+is%3Aopen+label%3Aready-to-implement) 表示设计已确定，欢迎提交代码 PR。任何人都可以认领带有 label 的 issue；如果你希望某个 issue 被考虑添加 readiness label，可以在 issue 中 mention **@oss-maintainers**。

### 本地构建仓库

从源码构建并运行 Warp：

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run Warp
./script/presubmit   # fmt, clippy, and tests
```

完整工程指南请参阅 [WARP.md](WARP.md)，其中包括代码风格、测试以及平台相关说明。

## 加入团队

有兴趣加入 Warp 团队？请查看[开放职位](https://www.warp.dev/careers)。

## 支持与问题

1. 阅读[官方文档](https://docs.warp.dev/)，获取完整的 Warp 功能指南。
2. 加入 [Slack Community](https://go.warp.dev/join-preview)，与其他用户交流并获得 Warp 团队帮助；贡献者会在 [`#oss-contributors`](https://warpcommunity.slack.com/archives/C0B0LM8N4DB) 频道活动。
3. 尝试 [Preview build](https://www.warp.dev/download-preview)，测试最新实验性功能。
4. 在任意 issue 中 mention **@oss-maintainers** 以升级给团队处理，例如当你遇到自动化 agents 相关问题时。

## 行为准则

我们希望每个人都保持尊重与同理心。Warp 遵循 [Code of Conduct](CODE_OF_CONDUCT.md)。如需报告违规行为，请发送邮件至 warp-coc at warp.dev。

## 开源依赖

这里列出一些帮助 Warp 起步的[开源依赖](https://docs.warp.dev/help/licenses)：

- [Tokio](https://github.com/tokio-rs/tokio)
- [NuShell](https://github.com/nushell/nushell)
- [Fig Completion Specs](https://github.com/withfig/autocomplete)
- [Warp Server Framework](https://github.com/seanmonstar/warp)
- [Alacritty](https://github.com/alacritty/alacritty)
- [Hyper HTTP library](https://github.com/hyperium/hyper)
- [FontKit](https://github.com/servo/font-kit)
- [Core-foundation](https://github.com/servo/core-foundation-rs)
- [Smol](https://github.com/smol-rs/smol)
