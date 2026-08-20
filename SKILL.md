---
name: tura
description: Work in the Tura agent-runtime repository. Use for Tura architecture, Rust backend, GUI/TUI, prompts, commands, providers, sessions, documentation, tests, packaging, and release work in this directory.
---

# Tura Repository

Tura is a Rust and TypeScript agent-runtime harness that executes model-planned command graphs while reducing repeated model round trips. Use this file to load only the repository documentation relevant to the current task.

## Start Here

- Read [README.md](README.md) for the product overview and primary workflows; use [README.zh-CN.md](README.zh-CN.md) for the Chinese version.
- Read [docs/README.md](docs/README.md) for the documentation entry point and [docs/SUMMARY.md](docs/SUMMARY.md) for the canonical user-documentation table of contents.
- Read [ARCHITECTURE.md](ARCHITECTURE.md) before changes that cross crates, processes, apps, command routing, prompts, or persistence boundaries.
- Consult [ROADMAP.md](ROADMAP.md), [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md), and [CHANGELOG.md](CHANGELOG.md) when evaluating intended scope, known gaps, or released behavior.

## Route by Task

### Installation, configuration, and usage

- [Overview](docs/start/overview.md)
- [Install and uninstall](docs/start/install.md)
- [Release packages](docs/start/release-packages.md)
- [Providers](docs/start/providers.md)
- [How to start](docs/start/how-to-start.md)
- [CLI parameters](docs/start/cli-parameters.md)
- [Settings](docs/start/settings.md)
- [Sessions](docs/start/sessions.md)
- [Documentation navigation](docs/start/navigation.md)
- [GUI settings](docs/start/gui-settings.md)
- [TUI settings](docs/start/tui-settings.md)

### Runtime concepts and user-facing behavior

- [Task status](docs/core/task-status.md)
- [Context management](docs/core/context-management.md)
- [Runtime prompt](docs/core/runtime-prompt.md)
- [Command run](docs/core/command-run.md)
- [Commands](docs/core/commands.md)
- [Agents](docs/core/agents.md)
- [Personas](docs/core/personas.md)
- [HTML rich text](docs/core/html-rich-text.md)
- [Prompt style and dynamic injection](docs/core/prompt-style.md)

### Customization

- [Custom providers](docs/customization/custom-providers.md)
- [Custom personas](docs/customization/custom-personas.md)
- [Custom agents](docs/customization/custom-agents.md)
- [Custom runtime prompts](docs/customization/custom-runtime-prompt.md)
- [Custom commands](docs/customization/custom-commands.md)

### Component architecture

- Agents: [agents/ARCHITECTURE.md](agents/ARCHITECTURE.md)
- Personas: [personas/ARCHITECTURE.md](personas/ARCHITECTURE.md)
- GUI: [apps/gui/README.md](apps/gui/README.md), [apps/gui/ARCHITECTURE.md](apps/gui/ARCHITECTURE.md), and [gateway adjustment notes](apps/gui/docs/gateway-adjustments.md)
- TUI: [apps/tui/README.md](apps/tui/README.md) and [apps/tui/ARCHITECTURE.md](apps/tui/ARCHITECTURE.md)
- Tauri shell: [apps/tauri/README.md](apps/tauri/README.md)
- Gateway: [crates/gateway/ARCHITECTURE.md](crates/gateway/ARCHITECTURE.md)
- Router: [crates/router/README.md](crates/router/README.md) and [crates/router/ARCHITECTURE.md](crates/router/ARCHITECTURE.md)
- Runtime: [crates/runtime/ARCHITECTURE.md](crates/runtime/ARCHITECTURE.md)
- Provider layer: [crates/provider/ARCHITECTURE.md](crates/provider/ARCHITECTURE.md)
- Tools: [crates/tools/ARCHITECTURE.md](crates/tools/ARCHITECTURE.md)
- Session log: [crates/session_log/README.md](crates/session_log/README.md) and [crates/session_log/ARCHITECTURE.md](crates/session_log/ARCHITECTURE.md)
- Paths and process hardening: [crates/path/ARCHITECTURE.md](crates/path/ARCHITECTURE.md)
- Build and maintenance scripts: [scripts/ARCHITECTURE.md](scripts/ARCHITECTURE.md)

### Testing and contribution

Read [tests/README.md](tests/README.md) before choosing or adding a test. Test classes are peers with different isolation, cost, and external-dependency rules:

- [Business tests](tests/business/README.md)
- [Backend end-to-end tests](tests/e2e/README.md)
- [OS tests](tests/os_testing/README.md)
- [Live tests](tests/live/README.md)
- [Release tests](tests/release/README.md)
- [Runtime/session equivalence gate](tests/equivalence/runtime_session/README.md)
- [Behavior-test quality gate](tests/equivalence/test_quality/README.md)

For contribution conventions, read [docs/contributing-guide.md](docs/contributing-guide.md). [architect.md](architect.md) records the behavior-test refactor plan and is relevant only to that migration.

### Prompt implementation sources

Read [the runtime-prompt guide](docs/core/runtime-prompt.md), [the prompt-style guide](docs/core/prompt-style.md), and the owning component architecture before editing prompt text. The main checked-in prompt sources are:

- Agent modes: [balanced](agents/src/balanced/prompt.md), [direct](agents/src/direct/prompt.md), and [direct text only](agents/src/direct-text-only/prompt.md)
- Persona and communication styles: [Tura](personas/src/tura/prompt/persona.md), [Pidan](personas/src/pidan/prompt/persona.md), [Wonderful](personas/src/wonderful/prompt/persona.md), [communication](personas/src/communication_style/communication_style.md), and [CLI communication](personas/src/communication_style/cli_communication_style.md)
- Built-in commands: [generate media](commands/generate_media/prompt.md), [read media](commands/read_media/prompt.md), and [web discover](commands/web_discover/prompt.md)
- Tool commands: [apply patch](crates/tools/src/commands/apply_patch/prompt.md), [bash](crates/tools/src/commands/bash/prompt.md), [planning](crates/tools/src/commands/planning/prompt.md), [shell command](crates/tools/src/commands/shell_command/prompt.md), [task status](crates/tools/src/commands/task_status/prompt.md), and [zsh](crates/tools/src/commands/zsh/prompt.md)
- Runtime manuals: [data research](crates/runtime/src/runtime_prompt/data_research/prompt.md), [debug](crates/runtime/src/runtime_prompt/debug/prompt.md), [DevOps](crates/runtime/src/runtime_prompt/devops/prompt.md), [editorial](crates/runtime/src/runtime_prompt/editorial/prompt.md), [frontend](crates/runtime/src/runtime_prompt/frontend/prompt.md), [interactive and 3D](crates/runtime/src/runtime_prompt/interactive_and_3d/prompt.md), [new build](crates/runtime/src/runtime_prompt/new_build/prompt.md), [refactoring](crates/runtime/src/runtime_prompt/refactoring/prompt.md), [visual](crates/runtime/src/runtime_prompt/visual/prompt.md), and [website](crates/runtime/src/runtime_prompt/website/prompt.md)

## Historical and Project Context

- Release notes: [0.1.34](docs/changelog/0.1.34.md) and [0.1.35](docs/changelog/0.1.35.md)
- Project rationale: [Why I am building Tura](docs/blog/why-i-am-building-tura.md)
- Benchmark and engineering context: [MCP workflow benchmark lessons](docs/blog/what-we-learned-from-the-mcp-workflow-benchmark.md), [benchmark data and test reports](docs/blog/we-need-more-benchmark-data-and-test-reports.md), [token-saving plugin analysis](docs/blog/token-saving-plugins-the-denominator-matters.md), and [GPT-5.6 Sol Max evaluation](docs/blog/is-gpt-5-6-sol-max-worth-it.md)
- Contribution overview: [How to contribute to Tura](docs/blog/how-to-contribute-to-tura.md)
- Funding and incubation tracking: [funding-application-tracker.md](funding-application-tracker.md)

## Working Rules

- Treat the root and component architecture documents as the source of truth for ownership and boundaries; confirm behavior in code when documentation and implementation differ.
- Keep user-facing documentation aligned with behavior changes. Update both language variants when a change affects claims or instructions shared by the English and Chinese root READMEs.
- Use the test class required by [tests/README.md](tests/README.md); do not collapse process-owning, live, performance, release, and local business coverage into one runner.
- Do not treat Markdown under `node_modules`, generated output, logs, sessions, or captured external npm research as repository instructions.
