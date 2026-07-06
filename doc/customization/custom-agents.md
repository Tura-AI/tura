# Custom agents

Agent 用来定制执行策略、默认模型、capabilities、validator 和 agent prompt。

## 用 CLI 创建

```bash
tura agent create strict-reviewer --prompt "Review code for bugs first. Always ask for tests."
tura agent show strict-reviewer --json
tura run -a strict-reviewer "Review current diff"
```

## 文件结构

```text
agents/src/<agent_id>/
  agent_config.json
  prompt.md
```

## 配置例子

```json
{
  "agent_name": "strict-reviewer",
  "description": "Strict code reviewer",
  "aliases": ["reviewer"],
  "provider": {
    "default_model_tier": "thinking",
    "stream": true,
    "temperature": 0.2
  },
  "agent_capabilities": [
    { "capability_name": "command_run" },
    { "capability_name": "task_status" }
  ],
  "validator": { "need_validator": false }
}
```

## ID 规则

Agent id 只能包含 ASCII 字母、数字、`_`、`-`，不能包含路径分隔符，也不能是 `.` 或 `..`。

代码引用：`agents/src/store.rs` 的 `dynamic_agent_path`。

## 加载和解析

代码引用：

- `agents/src/store.rs`，函数 `discover_agents`、`load_agent`、`save_dynamic_agent`。
- `crates/router/src/registry/agent.rs`，函数 `AgentRegistry::resolve`、`AgentRegistry::list_catalog`。
