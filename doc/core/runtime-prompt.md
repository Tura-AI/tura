# Runtime prompt

Runtime prompt 是 Tura 的“操作手册”系统。它不同于普通 agent prompt，也不同于外部 skills。

## 它是什么

当任务被标记为某种 `task_type`，runtime 会把对应操作手册注入 system context。例如：

- `debug`：要求先复现 bug，再修复，再验证。
- `frontend`：要求关注交互、布局、状态、可访问性。
- `refactoring`：要求学习原实现、保持兼容、做差分验证。
- `visual`：要求检查媒体和视觉结果。

操作手册文件在：

```text
crates/runtime/src/runtime_prompt/<task_type>/prompt.md
crates/runtime/src/runtime_prompt/<task_type>/prompt_identity.json
```

## 可用 task_type

当前内置类型包括：

- `data_research`
- `debug`
- `devops`
- `editorial`
- `frontend`
- `interactive_and_3d`
- `new_build`
- `refactoring`
- `visual`
- `website`

代码引用：`crates/runtime/src/prompt_style/runtime_prompt_manual.rs` 的 `available_manuals`、`valid_task_type_ids`。

## 如何触发

通过 `task_status` 设置：

```json
{
  "status": "doing",
  "task_group": "checkout frontend",
  "task_type": ["frontend"]
}
```

runtime 会归一化 task type，并把父级 manual 一并加入。

代码引用：`crates/runtime/src/prompt_style/runtime_prompt_manual.rs` 的 `normalize_task_type_ids`、`task_type_ids_from_value`。

## Capability 扩展

某些操作手册会带 capabilities。runtime 会把这些 capability 转成额外的 `command_run` command format 提示。

代码引用：

- `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`，函数 `capabilities_for_task_type_ids`。
- `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`，函数 `command_run_capability_content`。

## 和 agent prompt 的区别

| 类型 | 存放位置 | 作用 |
| --- | --- | --- |
| agent prompt | `agents/src/<agent_id>/prompt.md` | 定义 agent 风格、默认行为、能力倾向 |
| persona prompt | `personas/.../prompt/persona.md` | 定义沟通人格和表达方式 |
| runtime prompt manual | `crates/runtime/src/runtime_prompt/<type>/prompt.md` | 针对任务类型注入操作规程 |

简单说：agent 是“谁来做”，persona 是“怎么说”，runtime prompt 是“这类任务必须怎么做”。
