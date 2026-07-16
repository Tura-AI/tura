# GitHub Copilot

Tura's `github-copilot` provider uses the official [GitHub Copilot SDK](https://github.com/github/copilot-sdk) rather than the generic OpenAI-compatible HTTP adapter.

## Authentication

Authenticate the provider through Tura's existing GitHub Copilot device-code flow, or provide a token through:

```text
COPILOT_GITHUB_TOKEN
```

Tura resolves the existing Copilot CLI credential through its auth registry and passes the token explicitly to the SDK client and session.

## Runtime behaviour

The adapter starts the SDK in `ClientMode::Empty`. This disables Copilot CLI's ambient coding-agent tools, configuration discovery, host integration, and default environment context. Only tools declared by Tura for the current model call are exposed.

Tura remains responsible for the agent loop. Each provider call creates a temporary SDK session and supplies the ordered Tura message history as a transport envelope:

- Copilot `assistant.message_delta` events become Tura text stream events.
- Copilot `assistant.message.toolRequests` become canonical Tura `tool_calls`.
- Tura executes requested tools and supplies their results on the next provider call.
- Copilot usage events populate Tura token metrics.
- The temporary SDK session is disconnected and deleted after each call.

The stable Rust SDK bundles the matching Copilot CLI runtime by default. `COPILOT_CLI_PATH` can still be used to select an externally managed, version-compatible CLI.

## Configuration

Authenticate the existing provider entry, then select a model available to your Copilot account:

```bash
tura provider login github-copilot
tura provider status github-copilot
tura config set model=github-copilot/MODEL_ID
tura exec -m github-copilot/MODEL_ID "Reply with OK"
```

Replace `MODEL_ID` with a model exposed by the provider catalog. Model availability is controlled by the authenticated Copilot account; the SDK returns a provider error when the requested model is unavailable.

The provider supports streaming text, structured tool calls, forced/required tool choice, and reasoning effort. OpenAI-specific sampling, cache, service-tier, and response transport parameters are not forwarded because the Copilot SDK does not expose equivalent session options.

The default call timeout is 120 seconds. Override it with:

```text
TURA_GITHUB_COPILOT_TIMEOUT_SECONDS
```
