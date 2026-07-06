# Overview

Tura is a terminal-native developer tool for turning intent into verified code
changes with disciplined motion, audit trails, and repo-aware control.

The landing text in [`i18n.js`](../../i18n.js) frames Tura around four ideas:

| Frame | Meaning in the runtime |
| --- | --- |
| Macro CLI | One `command_run` call can batch known reads, edits, checks, and state updates. |
| Reasoning | The agent works backward from the desired verified outcome to the next necessary state. |
| Prompt | Runtime prompt manuals are loaded by task type instead of pasted into every turn. |
| TDD | Debug and repair work starts with reproduction and ends with evidence. |

The system is built for long-horizon repository tasks: inspect the workspace,
make narrow changes, run checks, keep session state, and attach evidence before
claiming success.

## Main components

- [Runtime](../architecture/runtime.md) owns the agent turn loop and prompt assembly.
- [Command run](../core/command-run.md) is the compact tool surface.
- [Session DB](../architecture/session-db.md) keeps durable workspace history.
- [Providers](providers.md) resolve model routes and credentials.
- [Agents](../core/agents.md) choose model defaults, prompt resources, and command capabilities.
- [Personas](../core/personas.md) control user-facing communication style.

## Next

Install Tura with [Install](install.md), then choose a front end in
[How to start](how-to-start.md).
