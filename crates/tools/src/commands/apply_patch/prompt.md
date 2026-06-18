Use `apply_patch` for focused workspace edits. The `command_line` value is the raw patch body beginning with `*** Begin Patch`; do not wrap it in shell heredocs, `apply_patch` commands, or explanatory text.

Rules:
- Keep each `apply_patch` `command_line` focused on one file or one code block.
- When editing different files or separate code blocks, put multiple `apply_patch` commands in the same `command_run` batch, usually with the same `step`, instead of making one oversized patch.
- Use separate `apply_patch` commands for unrelated files even if the edits are small; batching keeps the turn efficient while keeping each patch easy to review and recover.

Example: update two files in one batch with multiple focused patches.

```json
{
  "requests": {
    "commands": [
      {
        "command": "apply_patch",
        "step": 1,
        "command_line": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn label() -> &'static str {\n-    \"old\"\n-}\n+pub fn label() -> &'static str {\n+    \"new\"\n+}\n*** End Patch\n"
      },
      {
        "command": "apply_patch",
        "step": 1,
        "command_line": "*** Begin Patch\n*** Update File: README.md\n@@\n-Old label\n+New label\n*** End Patch\n"
      }
    ]
  }
}
```

Example: change two separate code blocks in the same file as two `apply_patch` commands in one batch.

```json
{
  "requests": {
    "commands": [
      {
        "command": "apply_patch",
        "step": 1,
        "command_line": "*** Begin Patch\n*** Update File: src/config.rs\n@@\n-pub const RETRIES: usize = 2;\n+pub const RETRIES: usize = 3;\n*** End Patch\n"
      },
      {
        "command": "apply_patch",
        "step": 1,
        "command_line": "*** Begin Patch\n*** Update File: src/config.rs\n@@\n-pub const TIMEOUT_MS: u64 = 1000;\n+pub const TIMEOUT_MS: u64 = 1500;\n*** End Patch\n"
      }
    ]
  }
}
```
