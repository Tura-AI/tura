<p align="center">
  <a href="https://turaai.net/">
    <img src="assets/tura/icon.svg" alt="Tura 图标" width="96">
  </a>
</p>

<p align="center">
  <a href="https://turaai.net/"><img alt="官网" title="Tura 官网" src="https://img.shields.io/badge/Website-turaai.net-40e0d0?style=flat-square&amp;labelColor=555555"></a>
  <a href="https://turaai.net/benchmark"><img alt="基准测试：8,243 轮" title="Tura 基准测试：8,243 轮智能体交互" src="https://img.shields.io/badge/Benchmark-8%2C243_turns-9b59b6?style=flat-square&amp;labelColor=555555"></a>
  <a href="https://www.npmjs.com/package/tura-ai"><img alt="npm 包" title="Tura npm 包" src="https://img.shields.io/npm/v/tura-ai?style=flat-square&amp;logo=npm&amp;label=npm&amp;labelColor=555555&amp;color=cb3837"></a>
</p>

<p align="center"><a href="README.md">English</a> | <strong>简体中文</strong></p>

<h1 align="center">Tura：成功率高 16.7 个百分点，Token 少 77.5%</h1>

Tura 是一个开源的智能体运行时框架：用更少的 Token，把事情做得更好。

在传统 ReAct 会话里，每返回一次工具结果，模型都要重新进入一轮推理，带着系统提示和越来越长的上下文再走一遍。Tura 把同一个任务组织成由运行时管理的命令图；只要后续步骤是确定的，就继续执行，不必每一步都再找模型来回确认。

<p align="center">
  <img src="assets/react-vs-tura-patch-workflow.svg" alt="ReAct 与 Tura 运行时执行循环的动态对比" width="1200">
</p>

<p align="center"><em>同一套工作流，ReAct 架构需要五轮，Tura 一轮即可完成。</em></p>

我们选取了 20 个 DeepSWE v1.1 任务，让每个智能体分别运行三次。Tura 减少了重复携带的上下文和模型往返，因此省下了一笔可观的 Token 预算。这笔预算有两种花法：Direct 尽可能把它变成更低的成本——总 Token 比 Codex CLI 少 77.5%，验证器成功率则相近，分别为 65.0% 和 63.3%；Balanced 会把更多预算重新投入调查、推理和验证，最终做到 80.0% 的成功率，比 Codex CLI 高 16.7 个百分点，同时仍少用 31.1% 的 Token。[^debug-figure][^debug-manifests]

### 基准测试

一个精心打磨的单轮提示很适合演示，却很难说明智能体能不能扛住真正的长任务。[长周期任务基准测试](https://turaai.net/benchmark)提供了另一种观察方式。我们发布的对比采用基于测试框架的开发任务，并保留了提示、每轮工具调用、Token 用量、补丁和验证器结果。

> 已发布的材料比较了 Tura Balanced、Tura Direct 和 Codex CLI 三种指定配置在 20 个 DeepSWE 任务、5 个代码重写任务和 2 个单独评审的设计任务中的表现。对比范围见[图表说明](assets/data/benchmark-agent-comparison.svg)，完整记录见[当前测试集证据记录](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)。[^debug-figure][^test-set-record]

这些结果只说明被测试的配置，并不意味着所有提供商、模型和环境都能得到同样的质量或性能。Anthropic/Claude、Google/Gemini、OpenAI 兼容接口、本地模型提供商、UI 延迟、运行时与会话解析，以及跨操作系统测试，都仍在[路线图](ROADMAP.md)和[已知问题与证据缺口](docs/KNOWN_ISSUES.md)中。

<details>
<summary><strong>完整基准测试报告</strong></summary>

<p align="center">
  <img src="assets/data/benchmark-agent-comparison.svg" alt="DeepSWE Debug 与 Rewrite Repo 基准测试对比" width="800">
</p>

<p align="center"><em>25 个高难度任务、6 种智能体与模型配置、270 个会话的验证器成功率与总 Token 用量。数据来源和计算说明见下文链接。</em></p>
</details>

### 界面截图

<p align="center">
  <img src="assets/screenshot/gui-ci-quality-demo.svg" alt="Tura GUI" width="800">
</p>

<p align="center"><em>GUI 支持多会话并行工作和 HTML 富文本。</em></p>

<p align="center">
  <img src="assets/screenshot/tui-ci-quality-demo.svg" alt="Tura TUI" width="800">
</p>

<p align="center"><em>TUI 同样支持多会话并行工作和 HTML 富文本。</em></p>

下面谈到的结果都来自已经公开的基准测试材料，不是一个没有出处的汇总数字。Tura 的主要工作方式可以归结为三个部分。

## 用一条宏命令跑完整个 CLI 工作流

今天的大多数编程智能体，仍在重复同一种工具调用循环：检查，等；打补丁，等；构建，等；测试，再等。

_**传统工具调用型编程智能体：**_

```bash
# 第 1 轮：检查环境

rg -n "TODO|command_run|handler" crates/
rg --files crates/runtime/src crates/tools/src
```

```bash
# 第 2 轮：应用补丁

*** Begin Patch
*** Update File: crates/tools/src/command_run/handler.rs
@@
-    // old command handler logic
+    // patched command handler logic
*** End Patch
```

```bash
# 第 3 轮：构建

cargo build -p runtime
```

```bash
# 第 4 轮：运行测试

cargo test -p runtime --lib
```

```bash
# 第 5 轮：运行 lint 检查

cargo clippy -p runtime --all-targets
```

Tura 走的是另一条路。它不把几十个零碎的小工具全都暴露给模型，而是提供一个宏工具：`command_run`。智能体可以据此搭出一棵多步骤执行树，在一轮 LLM 交互里把相关操作一起跑完。

下面这个例子里，两边实际执行的命令完全相同。普通工具调用型智能体需要五轮 LLM 交互，Tura 则把整段流程作为一个结构化的宏工作流来处理。省掉的是对话往返，不是必要的工程步骤。

_**Tura 的 CLI 宏命令：**_

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

目前还没有消融实验能够证明，单凭 `command_run` 就足以让 Tura 减少交互轮数和 Token 用量。不过在完整的 DeepSWE 对比中，Balanced 比 Codex CLI 少 35.8% 的交互轮数和 31.1% 的 Token；Direct 则分别少 69.1% 和 77.5%。[^debug-figure][^debug-manifests]

## 反向推理

LLM 再让人惊叹，归根结底仍是一个根据文本 Token 概率进行统计归纳的模型。

比如让 LLM 在石头、剪刀、布中随便选一个，并不能保证三个结果真正等概率。如果你在意严格的三分之一概率，就应该调用外部随机数源，而不是想当然地认为模型输出会均匀分布。

放到编程任务里，这个问题经常会带来致命后果。

智能体更容易执行和生成统计上常见的代码与逻辑，可“常见”并不等于“经过充分推敲”，很多时候只是平庸的默认答案。

Tura 换了一种推理方式。

普通智能体往往从当前状态一路向提示中的目标推演。这里，$s_1$ 是当前状态，$s_n$ 是用户给出的目标。

$$
s_1 \rightarrow s_2 \rightarrow s_3 \rightarrow \cdots \rightarrow s_n
$$

Tura 会引导 LLM 先估计目标达成前的状态 $s_{n-1}$，再从 $s_{n-1}$ 反向推到 $s_{n-2}$。

还是以石头、剪刀、布为例，LLM 可以由此得出正确的实现策略：

```
> 要让石头、剪刀、布既公平又有挑战性，
> 出招必须没有偏差。
> 每种选择都要有真正的三分之一概率。
> 只靠文本概率，LLM 无法保证这一点。
> 应该用随机数脚本生成 randint(1, 3)，
> 再把石头、剪刀、布分别映射到对应数字。
```

放到编程任务中，这意味着：当智能体收到“修复一个前端 Bug”这样的目标时，它会先梳理完整执行路径、还原故障状态、找到根因，然后才开始写代码。在已发布的 DeepSWE 对比中，Tura Balanced 在 60 个二元任务验证器里比 Codex CLI 多通过 10 个。

在同一组 20 个任务上，DeepSWE 官方 mini-swe-agent 的结果显示，GPT-5.6 SOL High 与 Medium 推理强度之间相差 8%；而 Tura Balanced 领先 Codex CLI 16.7 个百分点。这说明，更高的推理强度本身不足以解释 Tura 的优势。[^debug-manifests][^rewrite-manifest]

## 运行时上下文与提示管理

很多所谓的 Skill，本质上只是被塞进上下文的一段提示，而且往往还不如主提示有力。

许多智能体框架会让一个会话一直运行，Skill 文件、工具输出和过期的任务历史越积越多。上下文装不下了，就单独开一轮压缩；可压缩后通常只剩一份摘要，真正影响执行的细节很容易变模糊，甚至直接丢失。

Tura 把上下文本身当作运行时状态机的一部分。

它不要求用户反复手动重置会话，也不让 Markdown Skill 无止境地堆在上下文里，而是通过 `task_status`、运行时提示和递归执行手册，把当前上下文限制在手头任务真正需要的范围内。

传统的 Skill 型智能体通常会维持同一个会话，把宽泛的 Markdown 指令加载进去，直到用户重置会话或触发压缩。Tura 则把运行时提示绑定到明确的任务状态：会话可以自动重命名、刷新和管理；特定任务需要的操作手册与 CLI 命令通过递归任务树加载；无关上下文可以从 CLI 中移除、替换或压缩。检查点保留的不只是泛泛的摘要，还可以包括代码位置、补丁、测试和任务状态。这样一来，过期信息更少，单个任务的 Token 成本更低，旧 Skill 或含糊摘要把当前工作带偏的机会也更少。

由于上下文压缩是一个 CLI 操作，Tura 可以在 `task_status.compact_context` 中保留精确的执行状态。在已发布的基准测试会话里，Tura 在压缩后平均 2.6 轮就能走出只读检查、恢复实际执行；Codex 的估算值则是 5.4 轮。[^compact-dynamodb][^compact-wasmi-r1][^compact-wasmi-r2][^compact-wasmi-r3][^compact-eza]

Tura 的 2.6 轮来自存档轮次合约中明确记录的 `compact_context` 事件。Codex 没有暴露等价的压缩事件，因此 5.4 轮是根据输入 Token 用量骤降的位置估算出来的，并排除了可以识别的媒体读取边界。

## 安装与运行

### 通过 NPM 安装

macOS 和 Linux：

```bash
npm install tura-ai
tura
```

Windows：

```powershell
npm install -g tura-ai
tura
```

同一个主包也以 `@tura-ai/tura` 的名称发布在 GitHub Packages。你需要把 `@tura-ai` 作用域指向 `https://npm.pkg.github.com`，使用拥有 `read:packages` 权限的 Token 完成认证，然后安装 `@tura-ai/tura`。对大多数人来说，直接安装 npm 上不带作用域的 `tura-ai` 仍然是最省事的方式。

Tura 不会内置任何模型提供商凭证。第一次启动时，请先配置 LLM 提供商并选择模型，再发送提示。CLI、TUI 和 GUI 的具体流程见[提供商设置](docs/start/providers.md#first-run-configure-an-llm-provider)。

### 从源码安装

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

源码安装脚本会完成环境配置、Release 构建，并把 Tura 注册到当前用户的 PATH。只有在你明确只想安装依赖、不想构建或注册 Tura 时，才需要在 PowerShell 中传入 `-EnvironmentOnly`，或在 macOS/Linux 中传入 `--environment-only`。

### 常用入口

| 命令 | 用途 |
| --- | --- |
| `tura` | 打开交互式终端 UI。 |
| `tura "提示"` | 带着一条初始提示打开 TUI。 |
| `tura exec "提示"` | 直接使用 Rust CLI 运行提示。 |
| `tura run "提示"` | 通过网关运行提示，支持流式输出和历史记录。 |
| `tura bash`、`tura zsh`、`tura shel` | 选择对应的命令执行 Shell 后发送提示。 |
| `tura_gateway` | 启动本地 HTTP/SSE 网关，也可以提供 Web GUI。 |
| `tura_gui` | 打开桌面 GUI 工作区客户端。 |

不同操作系统的 PATH 要求、执行器安装方式，以及可执行文件不在 PATH 时如何注册 CLI，请阅读[如何启动](docs/start/how-to-start.md)。命令参数和运行模式见 [CLI 参数](docs/start/cli-parameters.md)。

## 文档

GitBook 风格的文档索引在 [docs/SUMMARY.md](docs/SUMMARY.md)，完整导航页在 [docs/start/navigation.md](docs/start/navigation.md)。

### 入门

- [概览](docs/start/overview.md)
- [安装](docs/start/install.md)
- [如何启动](docs/start/how-to-start.md)
- [CLI 参数](docs/start/cli-parameters.md)
- [设置](docs/start/settings.md)
- [模型提供商](docs/start/providers.md)
- [会话](docs/start/sessions.md)
- [导航](docs/start/navigation.md)

### 核心概念

- [任务状态](docs/core/task-status.md)
- [上下文管理](docs/core/context-management.md)
- [运行时提示](docs/core/runtime-prompt.md)
- [命令执行](docs/core/command-run.md)
- [命令](docs/core/commands.md)
- [智能体](docs/core/agents.md)
- [角色](docs/core/personas.md)
- [富文本](docs/core/html-rich-text.md)
- [动态提示注入](docs/core/prompt-style.md)

### 架构

- [系统架构](ARCHITECTURE.md)
- [运行时 / 会话等价性门禁](tests/equivalence/runtime_session/README.md)
- [会话数据库](crates/session_log/ARCHITECTURE.md)
- [网关](crates/gateway/ARCHITECTURE.md)
- [路由器](crates/router/ARCHITECTURE.md)
- [运行时](crates/runtime/ARCHITECTURE.md)
- [工具](crates/tools/ARCHITECTURE.md)
- [终端用户界面](apps/tui/ARCHITECTURE.md)
- [图形用户界面](apps/gui/ARCHITECTURE.md)

### 自定义

- [自定义模型提供商](docs/customization/custom-providers.md)
- [自定义角色](docs/customization/custom-personas.md)
- [自定义智能体](docs/customization/custom-agents.md)
- [自定义运行时提示](docs/customization/custom-runtime-prompt.md)
- [自定义命令](docs/customization/custom-commands.md)

### 开发

- [脚本](scripts/ARCHITECTURE.md)
- [测试](scripts/ARCHITECTURE.md#xtask-test-collection-scripts)
- [环境配置](docs/start/settings.md)
- [系统架构](ARCHITECTURE.md)
- [基准测试方法](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md)
- [当前测试集证据记录](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)
- [基准测试材料](https://github.com/Tura-AI/benchmark/tree/main/results)

## 参与贡献与项目治理

我们希望每一次贡献都足够聚焦、方便审查，并在真正负责这项行为的测试层提供证据。请根据改动类型选择对应的 Issue 和 Pull Request 模板，不必拿同一张检查清单套所有情况。

- [参与贡献](.github/CONTRIBUTING.md) — 从这里开始：改动类型、开发环境、测试选择和 Pull Request 流程。
- [贡献指南](docs/contributing-guide.md) — 测试归属、受影响矩阵、性能证据和材料脱敏规则。
- [路线图](ROADMAP.md) — 当前 0.1.x 稳定化重点，以及计划中的 0.2 任务规划工作区。
- [已知问题与证据缺口](docs/KNOWN_ISSUES.md) — 尚未完成的架构、模型提供商、基准测试、性能和跨操作系统工作。
- [行为准则](.github/CODE_OF_CONDUCT.md) — 社区规范与开放智能体框架原则。
- [安全政策](.github/SECURITY.md) — 受支持版本与漏洞私下报告方式。
- [支持](.github/SUPPORT.md) — 报告 Bug、提出功能需求或咨询安装与使用问题。

## 开源协议

Tura 使用 AGPL-3.0-or-later 许可证，详见 [LICENSE](LICENSE)。

## 基准测试说明与数据来源

- [基准测试方法](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md)
- [当前测试集证据记录](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)
- [基准测试材料](https://github.com/Tura-AI/benchmark/tree/main/results)

[^debug-figure]: [DeepSWE 与 Rewrite Repo 对比图](assets/data/benchmark-agent-comparison.svg)。图中注明了 README 所采用的任务、会话、验证器、交互轮数、Token 和汇总范围。

[^test-set-record]: [`tura-benchmark` 当前测试集证据记录](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)，其中包括全部 8 份已发布设计任务 HTML 材料及其运行合约的直接链接。

[^debug-manifests]: `tura-benchmark` DeepSWE 的[第 1 次重复实验](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/manifest.json)、[第 2 次重复实验](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/manifest.json)和[第 3 次重复实验](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/manifest.json)。每份清单都包含同一组 3 种智能体配置在 20 个任务上的结果，合计 180 个会话。

[^rewrite-manifest]: [`tura-benchmark` GPT-5.6 Rewrite Repo 标准清单](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/canonical-manifest.json)。两种配置各运行 10 个会话，文中引用的总计结果为：Tura Balanced 通过 389/472 项，Codex CLI 通过 351/472 项。

[^compact-dynamodb]: `tura-benchmark` DynamoDB 任务的[第 107 轮上下文压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0107.json)，以及之后[第 114 轮首次应用补丁](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0114.json)。

[^compact-wasmi-r1]: `tura-benchmark` Wasmi 第 1 次重复实验的[第 43 轮上下文压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0043.json)，以及[第 44 轮首次非只读操作](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0044.json)。该次运行在第 46 轮结束，之后没有再应用补丁或运行测试。

[^compact-wasmi-r2]: `tura-benchmark` Wasmi 第 2 次重复实验的[第 26 轮上下文压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0026.json)、[第 28 轮首次非只读操作](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0028.json)，以及[第 39 轮首次应用补丁或运行测试](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0039.json)。

[^compact-wasmi-r3]: `tura-benchmark` Wasmi 第 3 次重复实验的[第 39 轮上下文压缩](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0039.json)，以及[第 41 轮首次应用补丁或运行测试](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0041.json)。

[^compact-eza]: `tura-benchmark` eza 任务的[第 23 轮上下文压缩](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0023.json)，以及[第 24 轮首次运行后续测试](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0024.json)。
