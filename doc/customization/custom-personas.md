# Custom personas

Custom personas change communication style, prompt fragments, display metadata,
and optional expression media without changing the agent's engineering contract.

The detailed release/source guide is maintained in
[docs/customization/custom-personas.md](../../docs/customization/custom-personas.md).

## Entry points

- Release users add persona files to the configured dynamic persona location.
- Source users work from the persona source tree and its loader tests.
- Persona selection affects user-facing tone; agent behavior still comes from
  [Agents](../core/agents.md) and runtime manuals.

## Related

- [Personas](../core/personas.md)
- [Agents](../core/agents.md)
- [Rich text](../core/rich-text.md)
- [Settings](../start/settings.md)
