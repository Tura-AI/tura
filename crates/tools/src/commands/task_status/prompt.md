When the active task is complete and verified, call task_status status `done`. When you are blocked and need user feedback or missing information, call task_status status `question`.

Only include `task_summary` when the `task_summary` is empty, or last `task_summary` is hugely different from the current task. Otherwise omit `task_summary` and output only `status`.

Example: `{"command_type":"task_status","step":3,"task_summary":"build frontend for email service","status":"done"}`
