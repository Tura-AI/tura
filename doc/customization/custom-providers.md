# Custom providers

Custom providers extend the provider catalog, auth metadata, model tiers,
fallback routes, and client-visible provider choices.

The detailed release/source guide is maintained in
[docs/customization/custom-providers.md](../../docs/customization/custom-providers.md).

## Entry points

- Release users configure the packaged provider config under the installed
  release layout or via `TURA_PROVIDER_CONFIG`.
- Source users edit the source checkout provider config and validate through the
  gateway/provider tests.
- Runtime selection flows through [Providers](../start/providers.md),
  [Settings](../start/settings.md), and the provider architecture owner.

## Related

- [Providers](../start/providers.md)
- [Settings](../start/settings.md)
- [Custom agents](custom-agents.md)
- [Development environment](../development/environment.md)
