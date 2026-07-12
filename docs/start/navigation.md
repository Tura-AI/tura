# Documentation navigation

This is the full human-readable documentation directory. It mirrors the GitBook
summary and points directly to the real owner documents, not thin placeholder
pages.

## Start

1. [Overview](overview.md) - what Tura is and why it exists.
2. [Install](install.md) - source, release, npm, uninstall, and cleanup paths.
3. [How to start](how-to-start.md) - CLI, TUI, GUI, gateway, source starts, and OS PATH requirements.
4. [CLI parameters](cli-parameters.md) - command-line options, binaries, and environment flags.
5. [Settings](settings.md) - settings map with links to TUI, GUI, providers, sessions, and CLI options.
6. [Providers](providers.md) - provider catalog, auth, model tiers, routes, and credentials.
7. [Sessions](sessions.md) - durable workspace-scoped session history.

## Core

1. [Task status](../core/task-status.md) - structured task state, task types, completion gates, and compact handoff state.
2. [Context management](../core/context-management.md) - session records, compaction, prompt reinsertion, and long-task continuity.
3. [Runtime prompt](../core/runtime-prompt.md) - operation manuals selected by `task_type`.
4. [Command run](../core/command-run.md) - batched shell, patch, web/media, and task-state command execution.
5. [Commands](../core/commands.md) - command registry, schemas, policies, and command implementation rules.
6. [Agents](../core/agents.md) - agent identities, capabilities, prompt resources, aliases, and model defaults.
7. [Personas](../core/personas.md) - communication style, persona prompt fragments, display metadata, and media expressions.
8. [Rich text](../core/html-rich-text.md) - messaging-app HTML subset, media tokens, and GUI/TUI rendering behavior.
9. [Dynamic prompt injection](../core/prompt-style.md) - runtime-owned prompt fragments for state, persona, task, retry, and compaction.

## Architecture

1. [Session DB](../../crates/session_log/ARCHITECTURE.md) - SQLite owner, workspace session logs, indexes, queues, and sockets.
2. [Gateway](../../crates/gateway/ARCHITECTURE.md) - HTTP/SSE API, config, sessions, provider, file, and GUI-serving surface.
3. [Router](../../crates/router/ARCHITECTURE.md) - per-home daemon, runtime dispatch, registry operations, and IPC routing.
4. [Runtime](../../crates/runtime/ARCHITECTURE.md) - agent loop, context building, provider streaming, tool callbacks, manuals, and checkpoints.
5. [Tool](../../crates/tools/ARCHITECTURE.md) - tool contracts, command-run execution, policies, locks, cancellation, and output shaping.
6. [Terminal user interface](../../apps/tui/ARCHITECTURE.md) - TypeScript terminal client, CLI flows, rendering, and gateway interaction.
7. [Graphic user interface](../../apps/gui/ARCHITECTURE.md) - Solid/Vite GUI architecture, workspace UI, settings, sessions, and gateway usage.

## Customization

1. [Custom providers](../customization/custom-providers.md) - release/source provider config, auth, model routes, and validation.
2. [Custom personas](../customization/custom-personas.md) - release/source persona files, prompt fragments, and media manifests.
3. [Custom agents](../customization/custom-agents.md) - release/source agent definitions, capabilities, prompts, and aliases.
4. [Custom runtime prompt](../customization/custom-runtime-prompt.md) - manual catalogs, `task_type`, dependencies, and capability injection.
5. [Custom commands](../customization/custom-commands.md) - command handlers, schemas, policies, prompts, tests, and agent exposure.

## Development

1. [Scripts](../../scripts/ARCHITECTURE.md) - installers, debug/release builds, CLI registration, CI, release packaging, and npm packaging.
2. [Testing](../../scripts/ARCHITECTURE.md#xtask-test-collection-scripts) - business, OS, live, release, performance, app, and benchmark test lanes.
3. [Environment](settings.md) - operational settings and environment references for providers, sessions, CLI, and clients.
4. [Architecture](../../ARCHITECTURE.md) - whole-project binary topology, repository layout, logs, services, and ownership.
5. [Benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md) - benchmark scope, task selection, scoring, and limitations.
6. [Current test-set evidence record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md) - data lineage, cohort boundaries, retained anomalies, design observations, and limitations.
7. [Benchmark repository](https://github.com/Tura-AI/benchmark) - task declarations, agent launch config, reports, and harnesses.
