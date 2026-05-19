pub const TASK_DELIVERED: &str = r#"Multiple-tasks delivery.
When multiple_tasks mode is active, each planned task must be completed in order.
Use command_run only while work remains.
When the active planned task is complete and verified, call task_delivered with task_delivered true instead of rerunning verification.
Do not call task_delivered for partial progress, intent, elapsed effort, or an unverified result.
After task_delivered succeeds, continue with the next planned task if one remains. If no planned task remains, provide the final user-facing answer."#;
