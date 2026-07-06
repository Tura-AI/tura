# Custom runtime prompt

Runtime prompt manual 是按任务类型注入的操作规程。新增它等于新增一种任务类型或扩展已有任务类型的工作纪律。

## 文件结构

```text
crates/runtime/src/runtime_prompt/<task_type>/
  prompt_identity.json
  prompt.md
```

`prompt_identity.json` 例子：

```json
{
  "id": "debug",
  "display_name": "Debug Operation Manual",
  "description": "Use for bugs, failing tests, regressions, incorrect behavior, brittle edge cases, and failure analysis that requires reproduction before patching.",
  "father_ids": [],
  "capabilities": []
}
```

`prompt.md` 写具体操作规程，例如：先复现、定位边界、最小修改、验证、完成审计。

## 加载方式

构建时会嵌入 runtime prompt manuals；运行时也可以从 runtime prompt root 读取。

代码引用：

- `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`，函数 `available_manuals`、`read_manuals_from_dir`、`static_manuals`。
- `crates/runtime/build.rs`，生成 embedded manuals。

## 触发方式

通过 `task_status.task_type`：

```json
{
  "status": "doing",
  "task_group": "release automation",
  "task_type": ["devops"]
}
```

## 能力扩展

如果 manual 的 `capabilities` 包含额外 command，runtime 会把这些 command format 注入 `command_run` 上下文。

代码引用：`crates/runtime/src/prompt_style/runtime_prompt_manual.rs` 的 `command_run_capability_content`。

## 注意

不要把非常具体的一次性任务写成 runtime manual。Manual 是任务类型的通用规程，不是“今天下午修那个按钮”的备忘录。
