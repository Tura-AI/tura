# Settings

Settings are split by owner. The GUI and TUI expose overlapping runtime settings,
but provider routes, workspace config, and secrets are stored in different
places.

| Setting group | Owner/reference |
| --- | --- |
| Workspace runtime settings | `<workspace>/.tura/config.conf`; see [TUI settings](../../docs/start/tui-settings.md) and [GUI settings](../../docs/start/gui-settings.md). |
| Provider catalog and model tiers | `provider_config.json`; see [Providers](providers.md). |
| Provider credentials | `.env` resolved by `TURA_ENV_PATH`; see [Environment](../development/environment.md). |
| Agents | `agents/src/<agent-id>/agent_config.json`; see [Custom agents](../customization/custom-agents.md). |
| Personas | `personas/<persona-id>` or `personas/src/<persona-id>`; see [Custom personas](../customization/custom-personas.md). |
| Appearance and language | GUI/TUI config paths; see the detailed settings references above. |

## Related pages

- [Providers](providers.md)
- [Sessions](sessions.md)
- [Custom providers](../customization/custom-providers.md)
- [Custom agents](../customization/custom-agents.md)
- [Custom personas](../customization/custom-personas.md)
