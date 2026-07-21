<p align="center">
  <a href="https://turaai.net/">
    <img src="assets/tura/icon.svg" alt="Tura icon" width="96">
  </a>
</p>

<p align="center">
  <a href="https://turaai.net/"><img alt="Website" title="Tura 官方网站" src="https://img.shields.io/badge/Website-turaai.net-40e0d0?style=flat-square&amp;labelColor=555555"></a>
  <a href="https://turaai.net/benchmark"><img alt="Benchmark: 348 sessions" title="Tura benchmark: 348 会话" src="https://img.shields.io/badge/Benchmark-348_sessions-9b59b6?style=flat-square&amp;labelColor=555555"></a>
  <a href="https://www.npmjs.com/package/tura-ai"><img alt="npm package" title="Tura npm 包" src="https://img.shields.io/npm/v/tura-ai?style=flat-square&amp;logo=npm&amp;label=npm&amp;labelColor=555555&amp;color=cb3837"></a>
</p>

<p align="center"><a href="README.md">English</a> | <strong>简体中文</strong></p>

<h1 align="center">Tura：少 83.1% 的交互轮次，高 16.7 个百分点的成功率</h1>

Tura 是一个本地、开源、面向开发者的 AI 编程助手。如果你厌倦了空洞的技能宣称、毫无证据的 token 节省扩展，以及没有判断力、肆意破坏你仓库的 AI 代理——Tura 就是为你设计的。

在 20 个 DeepSWE v1.1 任务中，使用 GPT-5.6 SOL 高推理强度进行了 60 轮会话测试，Tura 通过减少重复上下文和模型往返，创造了显著的 token 预算优势。你可以通过两种方式利用这一优势：**Direct** 模式将大部分节省转化为更低的成本——比官方 Codex CLI High 配置少 83.5% 的总 token 量，验证器成功率为 65.0%，对比 Codex 的 60.0%；**Balanced** 模式则将更多节省的预算投入到推理、调查和验证中，达到了 80.0% 的成功率——比 Codex CLI High 高出 20 个百分点——同时仍使用少 49.6% 的 token。[^test-set-record][^debug-manifests]

### 基准测试

长周期任务[基准测试](https://turaai.net/benchmark)是看透精心打磨的孤立提示，了解代理如何处理真实工作的一种方式。已发布的对比使用基于测试框架的开发任务，包含存档的提示、每轮工具调用、token 用量、补丁和验证器结果。

> 以下主要对比固定了模型和推理标签：Tura Balanced High、Tura Direct High 和官方 Codex CLI High 配置在 20 个 DeepSWE 任务和 5 个重构任务上的表现。证据记录也保留了 Codex CLI Medium 作为单独的第二配置；基准测试方法将 2 个单独评审的设计任务排除在测试框架评分群体之外。[^test-set-record]

[GitHub 完整报告](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)

已发布的结果并不代表每个配置的提供者都具有同等质量或性能。更广泛的 Anthropic/Claude、Google/Gemini、OpenAI 兼容、本地提供者、UI 延迟、运行时/会话解析以及跨操作系统测量仍在[路线图](ROADMAP.md)和[已知证据缺口](docs/KNOWN_ISSUES.md)的规划范围内。

<details>
<summary><strong>完整基准测试报告</strong></summary>

<img src="assets/data/benchmark-agent-comparison.svg" alt="High-to-High 基准对比" width="800">
</details>

### 截图

<p align="center">
  <img src="assets/screenshot/gui-ci-quality-demo.svg" alt="Tura GUI" width="800">
</p>

<p align="center"><em>GUI 页面，支持多会话并发工作和 HTML 富文本渲染。</em></p>

<p align="center">
  <img src="assets/screenshot/tui-ci-quality-demo.svg" alt="Tura TUI" width="800">
</p>

<p align="center"><em>TUI 页面，支持多会话并发工作和 HTML 富文本渲染。</em></p>

以下结果来自已发布的基准测试工件，而非未经引用的聚合数据。三项核心系统承担了大部分工作：

## 宏 CLI 命令运行

大多数编程助手仍然依赖重复的工具调用循环：检查、等待、打补丁、等待、构建、等待、测试、等待。

_**传统工具调用型编程助手：**_

```bash
# 第 1 轮 — 检查环境

rg -n "TODO|command_run|handler" crates/
rg --files crates/runtime/src crates/tools/src
```

```bash
# 第 2 轮 — 应用补丁

*** Begin Patch
*** Update File: crates/tools/src/command_run/handler.rs
@@
-    // old command handler logic
+    // patched command handler logic
*** End Patch
```

```bash
# 第 3 轮 — 构建

cargo build -p runtime
```

```bash
# 第 4 轮 — 运行测试

cargo test -p runtime --lib
```

```bash
# 第 5 轮 — 运行 lint 验证

cargo clippy -p runtime --all-targets
```

Tura 走了一条不同的路。它不是向模型暴露几十个小工具，而是暴露一个宏工具：`command_run`。这样，代理可以构建一个多步执行树，在一轮 LLM 调用中完成相关操作。

在下面的例子中，两个代理运行相同的命令。传统工具调用型代理需要五轮 LLM 交互；Tura 将整个序列作为一个结构化的宏工作流处理。节省的是对话开销，而非工程纪律。

_**Tura 宏 CLI 命令：**_

```json
{
  "name": "command_run",
  "arguments": {
    "commands": [
      {
        "step": 1,
        "command_type": "shell_command",
        "command_line": "rg -n \"TODO|command_run|handler\" crates/"
      },
      {
        "step": 1,
        "command_type": "shell_command",
        "command_line": "rg --files crates/runtime/src crates/tools/src"
      },
      {
        "step": 2,
        "command_type": "apply_patch",
        "command_line": "*** Begin Patch\n*** Update File: crates/tools/src/command_run/handler.rs\n@@\n-    // old command handler logic\n+    // patched command handler logic\n*** End Patch"
      },
      {
        "step": 3,
        "command_type": "shell_command",
        "command_line": "cargo build -p runtime"
      },
      {
        "step": 4,
        "command_type": "shell_command",
        "command_line": "cargo test -p runtime --lib"
      },
      {
        "step": 4,
        "command_type": "shell_command",
        "command_line": "cargo clippy -p runtime --all-targets"
      }
    ]
  }
}
```

目前没有消融实验证明仅凭 `command_run` 就能带来 Tura 更低的轮次和 token 用量。但在匹配 High 配置的 DeepSWE 对比中，Balanced 比 Codex CLI High 少用了 66.8% 的模型轮次和 49.6% 的 token，而 Direct 则少用了 84.0% 的轮次和 83.5% 的 token。[^test-set-record][^debug-manifests]

## 反向推理

无论 LLM 多么令人印象深刻，其核心仍然是一个基于文本 token 概率的统计归纳模型。

例如，让 LLM 在石头、剪刀、布中做选择并不能保证均匀随机的结果。如果真正三分之一的分布很重要，则需要外部随机数源，而不是对模型输出概率的未经引证的假设。

在编程任务中，这往往是致命的。

代理更倾向于执行和生成统计上更常见的代码和逻辑。但常见的代码和逻辑往往平庸且欠考虑。

Tura 采用了一种不同的策略。

在推理过程中，普通代理从当前状态推理到提示目标。即，$s_1$ 是当前状态，$s_n$ 是用户提示给出的目标。

$$
s_1 \rightarrow s_2 \rightarrow s_3 \rightarrow \cdots \rightarrow s_n
$$

相反，Tura 引导 LLM 先统计估计 $s_{n-1}$，然后从 $s_{n-1}$ 的状态反向推理到 $s_{n-2}$。

在下面的例子中，LLM 能够正确推导出玩石头-剪刀-布的最优策略。

```
> 为了让石头-剪刀-布公平且具有挑战性，
> 我们需要无偏的游戏。
> 每一步必须拥有真正的三分之一概率。
> LLM 仅凭文本概率无法保证这一点。
> 使用随机数生成器脚本生成 randint(1, 3)
> 然后将石头、剪刀、布映射到数字。
```

在编程任务中，这意味着当代理看到一个目标（如修复前端 bug）时，它被引导去推理完整的执行路径，重构失败状态，并在编写代码之前识别根本原因。在匹配 High 配置的 DeepSWE 对比中，Tura Balanced 比 Codex CLI High 多通过了 60 个二元任务验证器中的 12 个。

该对比中的两种配置都使用 GPT-5.6 SOL 和 High 推理标签，因此 High 与 Medium 的推理强度差异无法解释这 20 个百分点的通过率差异。该结果仍然是一个系统层面的关联，而不是对反向推理或任何其他个体特征的因果估计。[^test-set-record][^debug-manifests]

## 运行时上下文与提示管理器

所谓技能，往往只是加载到上下文中的较弱提示。

在许多代理框架中，长期运行的会话会不断累积技能文件、工具输出和陈旧的任务历史。当上下文变得过大时，代理会进入一个单独的压缩轮次，但这种压缩通常只保留一个压缩摘要。重要的执行细节可能会变得模糊或丢失。

Tura 将上下文视为运行时状态机的一部分。

Tura 不依赖用户手动重置会话或让 Markdown 技能堆积，而是使用 `task_status`、运行时提示和递归执行手册将活动上下文限定在当前任务范围内。

传统基于技能的代理通常保持一个会话运行直到用户启动另一个会话，将宽泛的 Markdown 技能加载到该会话中，并使其保持激活直到重置或压缩。Tura 则将运行时提示绑定到显式的任务状态：会话可以被重命名、刷新和自动管理；任务特定的手册和 CLI 命令通过递归任务树加载；不相关的上下文可以被移除、替换或通过 CLI 压缩。检查点可以保留代码位置、补丁、测试和任务状态，而不仅仅是松散摘要。在实践中，这意味着更少的过期上下文、更低的任务范围 token 成本，以及更少的旧技能或模糊摘要误导当前工作的机会。

由于压缩是一个 CLI 操作，Tura 可以在 `task_status.compact_context` 中保留精确的执行状态。在已发布的基准测试会话中，Tura 超越了只读检查，在压缩后平均恢复了 2.6 轮执行，而 Codex 估计需要 5.4 轮。[^compact-dynamodb][^compact-wasmi-r1][^compact-wasmi-r2][^compact-wasmi-r3][^compact-eza]

Tura 的 2.6 轮结果是根据其存档的轮次合约中的显式 `compact_context` 事件计算的。Codex 不暴露等效的压缩事件，因此其 5.4 轮结果是通过输入 token 用量急剧下降的点（排除可识别的媒体读取边界）估算得出的。

## 安装与运行

### NPM 发布版

Mac 和 Linux：

```bash
npm install tura-ai
tura
```

Windows：

```powershell
npm install -g tura-ai
tura
```

相同的主包也发布到 GitHub Packages 作为 `@tura-ai/tura`。为 `https://npm.pkg.github.com` 配置 `@tura-ai` 范围，使用具有 `read:packages` 权限的 token 进行身份验证，然后安装 `@tura-ai/tura`。npm 上的无范围 `tura-ai` 包仍然是最简单的公开安装方式。

Tura 不捆绑提供者凭证。首次启动时，在发送提示前配置一个 LLM 提供者并选择其模型。详见[提供者设置](docs/start/providers.md#first-run-configure-an-llm-provider)中的 CLI、TUI 和 GUI 流程。

### 源码安装

Windows PowerShell：

```powershell
git clone https://github.com/Tura-AI/tura.git
cd tura
.\scripts\install.ps1
tura
```

macOS 或 Linux shell：

```bash
git clone https://github.com/Tura-AI/tura.git
cd tura
./scripts/install.sh
tura
```

源码安装程序会执行完整的环境设置、发布构建和用户 PATH 注册流程。当你只想安装依赖而不构建或注册 Tura 时，在 PowerShell 上传 `-EnvironmentOnly` 或在 macOS/Linux 上传 `--environment-only`。

### 常用入口点

| 入口                                | 用途                                                 |
| ----------------------------------- | ---------------------------------------------------- |
| `tura`                               | 交互式终端 UI。                                      |
| `tura "提示"`                         | 使用初始提示打开 TUI。                                |
| `tura exec "提示"`                    | 直接 Rust CLI 提示运行器。                            |
| `tura run "提示"`                     | 支持流式传输/历史的网关提示。                          |
| `tura bash`、`tura zsh`、`tura shel` | 使用选定命令运行 shell 界面的提示。                    |
| `tura_gateway`                       | 本地 HTTP/SSE 网关及可选的 Web GUI 服务。             |
| `tura_gui`                           | 桌面 GUI 工作区客户端。                               |

有关操作系统特定的 PATH 要求、执行器安装以及如何在可执行文件不在 PATH 上时注册 CLI，请阅读[如何开始](docs/start/how-to-start.md)。有关命令标志和模式，请阅读[CLI 参数](docs/start/cli-parameters.md)。

## 文档

GitBook 风格的文档索引位于 [docs/SUMMARY.md](docs/SUMMARY.md)。完整导航页面位于 [docs/start/navigation.md](docs/start/navigation.md)。

### 开始

- [概览](docs/start/overview.md)
- [安装](docs/start/install.md)
- [如何开始](docs/start/how-to-start.md)
- [CLI 参数](docs/start/cli-parameters.md)
- [设置](docs/start/settings.md)
- [提供者](docs/start/providers.md)
- [会话](docs/start/sessions.md)
- [导航](docs/start/navigation.md)

### 核心

- [任务状态](docs/core/task-status.md)
- [上下文管理](docs/core/context-management.md)
- [运行时提示](docs/core/runtime-prompt.md)
- [命令运行](docs/core/command-run.md)
- [命令](docs/core/commands.md)
- [代理](docs/core/agents.md)
- [人格](docs/core/personas.md)
- [富文本](docs/core/html-rich-text.md)
- [动态提示注入](docs/core/prompt-style.md)

### 架构

- [Session DB](crates/session_log/ARCHITECTURE.md)
- [Gateway](crates/gateway/ARCHITECTURE.md)
- [Router](crates/router/ARCHITECTURE.md)
- [Runtime](crates/runtime/ARCHITECTURE.md)
- [Tool](crates/tools/ARCHITECTURE.md)
- [终端用户界面](apps/tui/ARCHITECTURE.md)
- [图形用户界面](apps/gui/ARCHITECTURE.md)

### 自定义

- [自定义提供者](docs/customization/custom-providers.md)
- [自定义人格](docs/customization/custom-personas.md)
- [自定义代理](docs/customization/custom-agents.md)
- [自定义运行时提示](docs/customization/custom-runtime-prompt.md)
- [自定义命令](docs/customization/custom-commands.md)

### 开发

- [脚本](scripts/ARCHITECTURE.md)
- [测试](scripts/ARCHITECTURE.md#xtask-test-collection-scripts)
- [环境](docs/start/settings.md)
- [架构](ARCHITECTURE.md)
- [基准测试方法](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md)
- [当前测试集证据记录](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)
- [基准测试工件](https://github.com/Tura-AI/benchmark/tree/main/results)

## 贡献与项目治理

贡献应该是有节制、可审查且有证据支持的，在负责受影响行为的测试层提供。选择匹配的问题和拉取请求类型，而不是为每次更改应用同一个清单。

- [贡献指南](.github/CONTRIBUTING.md) — 从这里开始了解贡献类型、开发环境设置、测试选择和拉取请求步骤。
- [贡献说明](docs/contributing-guide.md) — 测试所有权、影响矩阵、性能证据和工件清理规则。
- [路线图](ROADMAP.md) — 当前的 0.1.x 稳定化优先级和计划中的 0.2 任务规划工作区。
- [已知问题和证据缺口](docs/KNOWN_ISSUES.md) — 开放的架构、提供者、基准测试、性能和跨操作系统工作。
- [行为准则](.github/CODE_OF_CONDUCT.md) — 社区标准和开放代理测试框架原则。
- [安全策略](.github/SECURITY.md) — 受支持的版本和私有漏洞报告。
- [支持](.github/SUPPORT.md) — 报告 bug、请求功能或提出设置和使用问题的地方。

## 许可协议

Tura 采用 AGPL-3.0-or-later 许可。详见 [LICENSE](LICENSE)。

## 基准测试注释与来源

- [基准测试方法](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md)
- [当前测试集证据记录](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)
- [基准测试工件](https://github.com/Tura-AI/benchmark/tree/main/results)

[^test-set-record]: [`tura-benchmark` 当前测试集证据记录](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)，定义了 280 次运行的已发布群体、278 次运行的关系分析群体、配置来源、聚合公式、排除规则和识别限制。README 的主要 High-to-High 表格从该已发布群体中选择了 210 次 Tura Balanced High、Tura Direct High 和 Codex CLI High 会话。

[^debug-manifests]: Tura 的 DeepSWE 观测数据位于 [`tura-benchmark` 复现 1](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/manifest.json)、[复现 2](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/manifest.json) 和[复现 3](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/manifest.json)。匹配的 Codex CLI High 观测数据位于 [High 复现 1](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-codex-cli-high-r01/manifest.json)、[High 复现 2](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-codex-cli-high-r02/manifest.json) 和 [High 复现 3](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-codex-cli-high-r03/manifest.json)。每个配置在相同的 20 个任务 ID 上贡献了 60 次会话。

[^rewrite-manifest]: Tura 的重构观测数据位于 [`tura-benchmark` GPT-5.6 Rewrite Repo 规范清单](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/canonical-manifest.json)；官方 Codex CLI High 观测数据位于 [Codex High 规范清单](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260714-codex-cli-0.144.1-gpt56-sol-high/canonical-manifest.json)。每个 High 配置贡献了 10 次会话和 472 个测试框架检查项。

[^compact-dynamodb]: [`tura-benchmark` DynamoDB 第 107 轮压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0107.json)和[第 114 轮首次后续补丁](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0114.json)。

[^compact-wasmi-r1]: [`tura-benchmark` Wasmi 复现 1 第 43 轮压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0043.json)和[第 44 轮首次非读取操作](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0044.json)。该运行在第 46 轮结束，无后续补丁或测试操作。

[^compact-wasmi-r2]: [`tura-benchmark` Wasmi 复现 2 第 26 轮压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0026.json)、[第 28 轮首次非读取操作](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0028.json)和[第 39 轮首次后续补丁/测试](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0039.json)。

[^compact-wasmi-r3]: [`tura-benchmark` Wasmi 复现 3 第 39 轮压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0039.json)和[第 41 轮首次后续补丁/测试](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0041.json)。

[^compact-eza]: [`tura-benchmark` eza 第 23 轮压缩](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0023.json)和[第 24 轮首次后续测试](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0024.json)。
