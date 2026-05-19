Use `multiple_tasks` only when the user explicitly gives multiple independent goal tasks with separate deliverables. Call it only right after receiving that request and before execution starts. Do not call it in the middle of execution.

Do not use `multiple_tasks` for a single goal that merely needs several execution steps such as inspect, edit, test, and summarize. Multi-step execution inside one goal is not multiple tasks. If the request is one objective, even a hard one, `multiple_tasks` is forbidden.

Use `multiple_tasks` only for the most complex 10% of requests where the user explicitly asks for several distinct objectives that should be delivered one by one, each with its own completion criteria.

The `command_line` value must be a JSON array. Each item must have:
- `task_summary`: one short sentence, about 10 words or fewer.
- `deliverble`: concrete files or locations to work on, plus the verification standard for that task.
