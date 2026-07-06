# Agents

Agent 定义“由谁来做任务”：默认 provider、能力、prompt、validator、是否启用 operation manual 等。

## 文件结构

Agent 放在：

```text
agents/src/<agent_id>/
  agent_config.json
  prompt.md
```

内置 agent 示例包括 `balanced`、`direct`、`direct-text-only`。

## agent_config.json 主要字段

| 字段 | 作用 |
| --- | --- |
| `agent_name` | agent id/name |
| `description` | 描述 |
| `aliases` | 别名 |
| `provider` | 默认 provider/model 参数 |
| `agent_prompt` | prompt 文件绑定 |
| `agent_capabilities` | 能力列表 |
| `op_manual` | 是否启用 operation manual |
| `validator` | validator 配置 |

## 创建/更新 agent

```bash
tura agent list --json
tura agent show balanced --json
tura agent create my-agent --prompt "Be strict and verify everything."
tura run -a my-agent "Inspect this repo"
```

## Router 如何解析 agent

Router 有静态 agent 表，也会加载 `agents/src` 下的动态/内置 agent。显式 agent 优先；没有显式 agent 时按 session type 回退。

代码引用：

- `agents/src/store.rs`，函数 `discover_agents`、`load_agent`、`save_dynamic_agent`、`default_agent_config`。
- `crates/router/src/registry/agent.rs`，函数 `AgentRegistry::from_static`、`resolve`、`resolve_by_session_type`、`upsert`。

## 例子：什么时候自定义 agent

| 需求 | 做法 |
| --- | --- |
| 想固定用某个 provider/model | 新建 agent，把 `provider` 默认值写进去 |
| 想改变沟通风格 | 优先用 persona，不一定要 agent |
| 想添加任务规则 | 修改 agent prompt 或 runtime prompt，看规则是否是通用任务类型 |
| 想扩展工具能力 | agent capabilities + runtime/manual capabilities 都要对齐 |
