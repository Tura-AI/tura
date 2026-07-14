# Settings

Settings are split by responsibility because one giant settings store would be
convenient only until two surfaces disagreed. Use this page as the GitBook entry,
then follow the owner reference for the full UI behavior.

| Setting area | Owner reference |
| --- | --- |
| TUI settings | [TUI settings](tui-settings.md) |
| GUI settings | [GUI settings](gui-settings.md) |
| Providers and credentials | [Providers](providers.md) |
| CLI startup options | [CLI parameters](cli-parameters.md) |
| Sessions and workspace state | [Sessions](sessions.md) |

## Rules

- Provider credentials should live in environment variables, `.env`, or settings
  flows, not in committed docs.
- Runtime behavior is selected through agents, personas, provider routes,
  `task_status.task_type`, and session state.
- Workspace-local state lives under `.tura/`; release and source binaries should
  not invent parallel settings stores.

## Related

- [Install](install.md)
- [How to start](how-to-start.md)
- [Custom providers](../customization/custom-providers.md)
- [Custom agents](../customization/custom-agents.md)
