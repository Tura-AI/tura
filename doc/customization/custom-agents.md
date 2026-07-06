# Custom agents

Custom agents define model defaults, aliases, capabilities, prompt resources,
validator behavior, operation-manual policy, and reporting style.

The detailed release/source guide is maintained in
[docs/customization/custom-agents.md](../../docs/customization/custom-agents.md).

## Entry points

- Release users add or override agent definitions in the runtime-visible agent
  store.
- Source users edit the agent source tree and verify store loading plus client
  selection behavior.
- Agent configuration is intentionally separate from persona style and provider
  credentials.

## Related

- [Agents](../core/agents.md)
- [Providers](../start/providers.md)
- [Runtime prompt](../core/runtime-prompt.md)
- [Custom runtime prompt](custom-runtime-prompt.md)
