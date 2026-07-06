# Providers

Providers are the services Tura can call at runtime. For coding sessions, the
important provider settings are the model catalog, model-tier routes, base URLs,
auth metadata, and latency policy.

The full start reference is [docs/start/providers.md](../../docs/start/providers.md).
Customization details are in [Custom providers](../customization/custom-providers.md).

## Config resolution

1. `TURA_PROVIDER_CONFIG`
2. `<TURA_PROJECT_ROOT>/config/provider_config.json`
3. `<TURA_PROJECT_ROOT>/crates/provider/config/provider_config.json`
4. `crates/provider/config/provider_config.json` under the current repo root

Credentials come from process environment variables and the `.env` file resolved
by `TURA_ENV_PATH`.

## Related pages

- [Settings](settings.md)
- [Agents](../core/agents.md)
- [Environment](../development/environment.md)
