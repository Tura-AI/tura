# Overview

Tura is a terminal-native developer tool for turning an intent into a verified
code change. It is built for the part between those two points: reading the
repository, choosing a narrow move, preserving an audit trail, and checking the
result before calling the work done.

Tura is framed around four ideas:

| Frame     | Meaning in the runtime                                                                  |
| --------- | --------------------------------------------------------------------------------------- |
| Macro CLI | One `command_run` call can batch known reads, edits, checks, and state updates.         |
| Reasoning | The agent works backward from the desired verified outcome to the next necessary state. |
| Prompt    | Runtime prompt manuals are loaded by task type instead of pasted into every turn.       |
| TDD       | Debug and repair work starts with reproduction and ends with evidence.                  |

The system is built for long-horizon repository tasks. It can inspect the
workspace, make narrow changes, run checks, keep session state, and attach
evidence before claiming success. "Done" is more useful when it has receipts.

## Main components

- [Runtime](../../crates/runtime/ARCHITECTURE.md) owns the agent turn loop and prompt assembly.
- [Command run](../core/command-run.md) is the compact tool surface.
- [Session DB](../../crates/session_log/ARCHITECTURE.md) keeps durable workspace history.
- [Providers](providers.md) resolve model routes and credentials.
- [Agents](../core/agents.md) choose model defaults, prompt resources, and command capabilities.
- [Personas](../core/personas.md) control user-facing communication style.

## Next

Install Tura with [Install](install.md), configure a model provider with
[Provider setup](providers.md#first-run-configure-an-llm-provider), then choose
a front end in [How to start](how-to-start.md).
