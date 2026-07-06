# Environment

Environment variables select release binaries, project roots, provider config,
provider keys, logs, benchmark agent executables, session homes, and command-run
shell behavior.

## Common variables

| Variable | Purpose |
| --- | --- |
| `TURA_HOME` | Runtime home for sockets, locks, per-home indexes, and local state. |
| `TURA_PROJECT_ROOT` | Source or packaged project root used by wrappers and runtime lookup. |
| `TURA_RELEASE_BIN_DIR` | Release binary directory used by npm and packaged launches. |
| `TURA_PROVIDER_CONFIG` | Provider configuration file override. |
| `LOG_PATH` | Provider call log root. |
| `TURA_COMMAND_RUN_SHELL` | Command-run shell surface override. |
| `TURA_LANG` | TUI language selection in client tests and rendering. |

Provider keys are provider-specific and should remain in the user's environment
or local secret manager, not in repository files.

## Release vs source

- Release launches usually inherit `TURA_PROJECT_ROOT` and
  `TURA_RELEASE_BIN_DIR` from the npm wrapper or installed launcher.
- Source launches usually rely on the checkout root and `target/debug` or
  `target/release` binaries.

## Related

- [Install](../start/install.md)
- [Providers](../start/providers.md)
- [Scripts](scripts.md)
- [Custom providers](../customization/custom-providers.md)
