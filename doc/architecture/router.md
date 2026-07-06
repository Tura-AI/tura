# Router architecture

Router 是 Tura 的调度和 registry 层。它不是模型运行时，也不是 provider；它负责把请求送到 runtime worker，并管理 agent/persona/command registry。

## Router 命令

`tura_router` 默认运行 `serve`。支持的命令包括：

- `serve`
- `serve-socket`
- `run-agent`
- `registry-agents-list`
- `registry-agent-get`
- `registry-agent-create`
- `registry-agent-update`
- `registry-agent-delete`
- `registry-personas-list`
- `registry-persona-get`
- `registry-persona-create`
- `registry-persona-update`
- `registry-persona-delete`
- `registry-commands-list`
- `registry-command-execute`

代码引用：`crates/router/src/cli.rs` 的 `run_router_command`。

## Agent dispatch

Router 接收 `RunAgentRequest`，构建 state，然后分派 runtime worker。

代码引用：

- `crates/router/src/cli.rs`，函数 `run_agent_cli`。
- `crates/router/src/runtime_dispatch.rs`，函数 `dispatch_run_agent`。

## Registry

Router 拥有三类 registry：

| Registry | 文件 | 作用 |
| --- | --- | --- |
| Agent | `crates/router/src/registry/agent.rs` | agent 发现、解析、upsert/delete |
| Persona | `crates/router/src/registry/persona.rs` | persona 发现、upsert/delete |
| Command | `crates/router/src/registry/command.rs` | workspace command 发现和模板渲染 |

## 和 runtime 的边界

Router 不组 prompt，不解析 provider 响应，不执行 agent loop。Runtime 负责这些。Router 只负责调度、生命周期和 registry。边界清楚，bug 才不会像杂草一样长。
