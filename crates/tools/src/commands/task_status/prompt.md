Use `task_status` only to update internal task-management state. It is never a substitute for the user-visible assistant message.
Every time you change `status` to `done` or `question`, first send a normal assistant-channel natural language reply containing the actual answer, explanation, completion summary, blocker, or question for the user. Then call `task_status` in the same assistant response.
For simple questions, greetings, acknowledgements, or ordinary conversation, answer the user naturally in the assistant channel before any terminal `task_status` update. Do not use `task_status` as the only response. If the conversation is answered, mark `done`; if you need user input, mark `question`. Do not mark `doing` for ordinary conversation.
Put the explanation, question, completion summary, modified files, artifacts, validation, risks, and follow-up notes in the assistant reply, not in `task_status` arguments.

Use the user's input language for `task_group`.
Keep `task_group` and `task_type` available as the internal code work area for the active task. Use it before execution of a newly recognized task, unless the current task group already accurately names the broad code work area.
`task_group` should be a few words describing the implementation area, not a concrete task detail, progress report, completion summary, or user reply.
Runtime may inject an additional state-aware startup gate when the current session has no `task_type`.

Correct examples:
- `PDF editing`
- `storefront frontend`
- `order settlement service`

Wrong examples:
- `Create a slide deck about OGAS system`
- `Add cart button animation`
- `Check order system logs`

Use `status` separately when the task state changes to `doing`, `question`, or `done`.
Use `task_type` to update the complete set of prompt and Operation Manual types needed by the current task. Update it as soon as you identify the task type. `task_type` is an array, so include multiple ids when multiple manuals apply. 
When updating `task_group` for active work, include `task_type` in the same update unless the session already has the correct task type.
You can update `task_type` without changing `task_group`, or give `compact_context` without updating `task_group`.
The available `task_type` values are injected dynamically from runtime prompt identities. You can remove `task_type` when you think it is not related anymore.

Only call `status: "doing"` when the task cannot be completed without additional command_run calls.
Call `status: "done"` only after the task is complete, verified, and every media file you plan to send or show to the user has been read and inspected with `read_media`.
Do not call `status: "done"` when a required or reasonably runnable verification command failed, timed out, was skipped, or could not start. Keep working to install missing dependencies, start required services, fix environment setup, and rerun the validation until it passes. This includes builds, unit tests, integration tests, Playwright/browser tests, runtime smoke checks, harnesses, and any user-requested verifier.

If verification should be runnable but the current environment truly cannot run it after reasonable setup effort, clearly explain the environment blocker to the user in the normal assistant reply and call `status: "question"`.
If user feedback, missing information, permissions, credentials, or keys are required, first send the user-facing assistant reply with the question or blocker, then call `status: "question"` in the same assistant response.
Use `compact_context` on `task_status` to create a context checkpoint when a meaningful phase is complete, when most of the previous context is no longer relevant to the next task, or when the active context reaches the 255,000 tokens hard cap.
Only use `compact_context` when the new task no longer depends on the current main context and a handoff is needed. The user will receive all conversation from the current task and any previous summary; include only details that are not already covered by that conversation or prior summary. Do not duplicate obvious dialogue history.
When useful work should happen before the checkpoint, put those required commands in earlier steps of the same `command_run`, then put the `task_status` command carrying `compact_context` after them. The results from earlier commands will still be executed and returned normally before the compacted context is used on the next turn.

The `compact_context` value is one handoff text for the next model turn. Include:
- current user goal and Operation Manual
- still-relevant user requirements and preferences
- workflow or process rules that must continue to be followed
- current task status, including completed and incomplete parts
- key decisions and constraints
- deliverables, file paths, and validation standards
- reference files, relevant code paths, architecture docs, test docs, or other documentation paths that should be read or kept in mind
- relevant steps already taken and important command results
- directory or file requirements needed to continue
- related process id and what the process is for
- exactly what to do next

Keep `compact_context` concise and structured. Do not exceed 15 sentences. Use plain text English, and do not use quotation marks or brackets.

Example `command_line` values:
- Update task group and task types: `{"task_group":"Storefront Frontend","task_type":["refactoring","frontend"],"status":"doing"}`
- Add new capability to current task: `{"task_type":["debug"]}`
- Finish after first sending a user-facing assistant reply in the same response: `{"status":"done"}`
- Ask for required user input after first sending the user-facing question: `{"status":"question"}`
- Checkpoint via task_status: `{"task_group":"storefront frontend","compact_context":"Need to continue by running cargo test -p runtime after the task_status prompt/schema edits. Existing unrelated git changes were present before this work..."}`
