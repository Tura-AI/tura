# Custom runtime prompt

Runtime prompt customization changes operation manuals, task type routing,
manual dependency rules, capability injection, and completion discipline.

The detailed release/source guide is maintained in
[docs/customization/custom-runtime-prompt.md](../../docs/customization/custom-runtime-prompt.md).

## Entry points

- Release users configure only supported prompt resources exposed by the release
  layout.
- Source users edit runtime prompt code and manual resources, then verify task
  type selection and injected capability text.
- Runtime prompt changes are high leverage; test them against real task flows,
  not just string snapshots.

## Related

- [Runtime prompt](../core/runtime-prompt.md)
- [Task status](../core/task-status.md)
- [Context management](../core/context-management.md)
- [Runtime architecture](../architecture/runtime.md)
