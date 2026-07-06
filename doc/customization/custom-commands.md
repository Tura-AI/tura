# Custom commands

Custom commands add command-run handlers, schemas, policies, prompts, timeout
behavior, router metadata, tests, and agent capability exposure.

The detailed release/source guide is maintained in
[docs/customization/custom-commands.md](../../docs/customization/custom-commands.md).

## Entry points

- Release users can use packaged commands and command-level configuration.
- Source users add command implementations in the tools/command tree and expose
  them through command-run registration.
- New commands must include policy, output shaping, focused tests, and runtime
  prompt capability text when agents need to know about them.

## Related

- [Command run](../core/command-run.md)
- [Commands](../core/commands.md)
- [Tool architecture](../architecture/tool.md)
- [Testing](../development/testing.md)
