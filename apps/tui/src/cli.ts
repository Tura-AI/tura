import { resolveGatewayUrl, resolveCwd } from "./gateway/directory.js";
import { CliUsageError, type CliContext, type ColorMode, type OutputMode } from "./types/common.js";
import { runPrompt } from "./commands/run.js";
import { resumeCommand } from "./commands/resume.js";
import { sessionCommand } from "./commands/session.js";
import { configCommand } from "./commands/config.js";
import { providerCommand } from "./commands/provider.js";
import { permissionCommand } from "./commands/permission.js";
import { commandCommand } from "./commands/command.js";
import { agentCommand } from "./commands/agent.js";
import { statusCommand } from "./commands/status.js";
import { completionCommand } from "./commands/completion.js";
import { runTui } from "./tui/app.js";
import { runtimeOverridesFromAssignment, type RuntimeConfigOverrides } from "./commands/config-values.js";

export async function main(argv: string[]): Promise<void> {
  const { context, args } = parseGlobal(argv);
  const command = args.shift();
  try {
    if (!command) {
      await runTui(context);
      return;
    }
    if (command === "help" || command === "--help" || command === "-h") {
      printHelp();
      return;
    }
    if (command === "run") {
      if (hasHelp(args)) {
        printRunHelp();
        return;
      }
      const parsed = parseRun(args, context.json);
      await runPrompt(context, parsed);
      return;
    }
    if (command === "resume") {
      if (hasHelp(args)) {
        printResumeHelp();
        return;
      }
      const parsed = parseResume(args, context.json);
      await resumeCommand(context, parsed);
      return;
    }
    if (hasHelp(args)) {
      if (command === "session") return printSessionHelp();
      if (command === "config") return printConfigHelp();
      if (command === "provider") return printProviderHelp();
      if (command === "permission") return printPermissionHelp();
      if (command === "command") return printCommandHelp();
      if (command === "agent") return printAgentHelp();
      if (command === "status") return printStatusHelp();
      if (command === "completion") return printCompletionHelp();
    }
    if (command === "session") return sessionCommand(context, args);
    if (command === "config") return configCommand(context, args);
    if (command === "provider") return providerCommand(context, args);
    if (command === "permission") return permissionCommand(context, args);
    if (command === "command") return commandCommand(context, args);
    if (command === "agent") return agentCommand(context, args);
    if (command === "status") return statusCommand(context);
    if (command === "completion") return completionCommand(args);
    await runTui(context, [command, ...args].join(" "));
  } catch (error) {
    const exitCode = typeof error === "object" && error && "exitCode" in error ? Number((error as { exitCode: number }).exitCode) : 1;
    if (exitCode === 2) printHelp();
    throw Object.assign(error instanceof Error ? error : new Error(String(error)), { exitCode });
  }
}

function hasHelp(args: string[]): boolean {
  return args.includes("--help") || args.includes("-h") || args.includes("help");
}

function parseGlobal(argv: string[]): { context: CliContext; args: string[] } {
  const args = [...argv];
  let gatewayUrl: string | undefined;
  let cwd: string | undefined;
  let color: ColorMode = "auto";
  let json = false;
  let verbose = false;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--gateway-url") gatewayUrl = takeValue(args, index--);
    else if (arg.startsWith("--gateway-url=")) {
      gatewayUrl = arg.slice("--gateway-url=".length);
      args.splice(index--, 1);
    } else if (arg === "--cwd") cwd = takeValue(args, index--);
    else if (arg.startsWith("--cwd=")) {
      cwd = arg.slice("--cwd=".length);
      args.splice(index--, 1);
    } else if (arg === "--json") {
      json = true;
      args.splice(index--, 1);
    } else if (arg === "--verbose") {
      verbose = true;
      args.splice(index--, 1);
    } else if (arg === "--color") color = takeValue(args, index--) as ColorMode;
    else if (arg.startsWith("--color=")) {
      color = arg.slice("--color=".length) as ColorMode;
      args.splice(index--, 1);
    }
  }
  return {
    context: {
      gatewayUrl: resolveGatewayUrl(gatewayUrl),
      cwd: resolveCwd(cwd),
      json,
      color,
      verbose,
    },
    args,
  };
}

function parseRun(args: string[], rootJson: boolean): Parameters<typeof runPrompt>[1] {
  let sessionID: string | undefined;
  let model: string | undefined;
  let agent: string | undefined;
  let sessionType: string | undefined;
  let modelVariant: string | undefined;
  let modelAccelerationEnabled: boolean | undefined;
  let forceMultipleTasks: boolean | undefined;
  let killProcessesOnStart: boolean | undefined;
  let validatorEnabled: boolean | undefined;
  let output: OutputMode = rootJson ? "json" : "text";
  let stream = true;
  let timeoutSec = 600;
  let lastMessageFile: string | undefined;
  const prompt: string[] = [];
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--session") sessionID = args[++index];
    else if (arg.startsWith("--session=")) sessionID = arg.slice("--session=".length);
    else if (arg === "--model" || arg === "-m") model = args[++index];
    else if (arg.startsWith("--model=")) model = arg.slice("--model=".length);
    else if (arg === "--agent" || arg === "--agent-id" || arg === "--agent-name" || arg === "-a") agent = args[++index];
    else if (arg.startsWith("--agent=")) agent = arg.slice("--agent=".length);
    else if (arg.startsWith("--agent-id=")) agent = arg.slice("--agent-id=".length);
    else if (arg === "--session-type") sessionType = args[++index];
    else if (arg.startsWith("--session-type=")) sessionType = arg.slice("--session-type=".length);
    else if (arg === "--model-variant" || arg === "--variant" || arg === "--reasoning-effort" || arg === "--model-reasoning-effort") {
      modelVariant = args[++index];
    } else if (arg.startsWith("--model-variant=")) modelVariant = arg.slice("--model-variant=".length);
    else if (arg.startsWith("--model-reasoning-effort=")) modelVariant = arg.slice("--model-reasoning-effort=".length);
    else if (arg.startsWith("--reasoning-effort=")) modelVariant = arg.slice("--reasoning-effort=".length);
    else if (arg === "--model-acceleration" || arg === "--accelerated") modelAccelerationEnabled = true;
    else if (arg === "--no-model-acceleration" || arg === "--no-accelerated") modelAccelerationEnabled = false;
    else if (arg === "-p" || arg === "--priority") modelAccelerationEnabled = true;
    else if (arg === "--force-multiple-tasks") forceMultipleTasks = true;
    else if (arg === "--no-force-multiple-tasks") forceMultipleTasks = false;
    else if (arg === "--output") output = parseOutput(args[++index]);
    else if (arg === "--json") output = "json";
    else if (arg === "--stream") stream = true;
    else if (arg === "--no-stream") stream = false;
    else if (arg === "--timeout") timeoutSec = Number(args[++index]);
    else if (arg === "--last-message-file") lastMessageFile = args[++index];
    else if (arg === "-c" || arg === "--config") {
      const overrides = runtimeOverridesFromAssignment(args[++index]);
      ({ model, agent, sessionType, modelVariant, modelAccelerationEnabled, forceMultipleTasks, killProcessesOnStart, validatorEnabled } =
        applyRunOverrides(
          { model, agent, sessionType, modelVariant, modelAccelerationEnabled, forceMultipleTasks, killProcessesOnStart, validatorEnabled },
          overrides,
        ));
    } else if (arg.startsWith("--config=")) {
      const overrides = runtimeOverridesFromAssignment(arg.slice("--config=".length));
      ({ model, agent, sessionType, modelVariant, modelAccelerationEnabled, forceMultipleTasks, killProcessesOnStart, validatorEnabled } =
        applyRunOverrides(
          { model, agent, sessionType, modelVariant, modelAccelerationEnabled, forceMultipleTasks, killProcessesOnStart, validatorEnabled },
          overrides,
        ));
    }
    else prompt.push(arg);
  }
  if (!prompt.join(" ").trim()) throw new CliUsageError("run requires a prompt");
  return {
    prompt: prompt.join(" "),
    sessionID,
    model,
    agent,
    sessionType,
    modelVariant,
    modelAccelerationEnabled,
    forceMultipleTasks,
    killProcessesOnStart,
    validatorEnabled,
    output,
    stream,
    timeoutSec,
    lastMessageFile,
    source: "cli",
  };
}

function applyRunOverrides(current: RuntimeConfigOverrides, next: RuntimeConfigOverrides): RuntimeConfigOverrides {
  return { ...current, ...next };
}

function parseResume(args: string[], rootJson: boolean): Parameters<typeof resumeCommand>[1] {
  let last = false;
  let output: OutputMode = rootJson ? "json" : "text";
  let sessionID: string | undefined;
  const prompt: string[] = [];
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--last") last = true;
    else if (arg === "--output") output = parseOutput(args[++index]);
    else if (arg === "--json") output = "json";
    else if (!sessionID && !last) sessionID = arg;
    else prompt.push(arg);
  }
  return { sessionID, last, prompt: prompt.join(" "), output };
}

function parseOutput(value: string): OutputMode {
  if (value === "text" || value === "json" || value === "ndjson") return value;
  throw new CliUsageError(`invalid output mode: ${value}`);
}

function takeValue(args: string[], index: number): string {
  const value = args[index + 1];
  if (!value) throw new CliUsageError(`${args[index]} requires a value`);
  args.splice(index, 2);
  return value;
}

function printHelp(): void {
  process.stdout.write(`Tura terminal client

Usage:
  tura [OPTIONS]                         open the interactive TUI
  tura [OPTIONS] run [PROMPT...]         run a non-interactive prompt
  tura [OPTIONS] resume SESSION_ID       show or continue a session
  tura [OPTIONS] <command> --help        show command-specific help

Commands:
  run           send a prompt through the gateway and stream the answer
  resume        show an existing session or append a follow-up prompt
  session       list, show, or delete sessions
  config        read or update workspace session config
  provider      list providers and inspect auth state
  permission    list and answer pending permission requests
  command       list or execute gateway slash commands
  agent         list, read, write, or delete agents under agents/
  status        print gateway, workspace, provider, and service status
  completion    generate shell completion for bash, zsh, or fish

Options:
  --gateway-url URL   Gateway base URL.
                      Defaults to TURA_GATEWAY_URL or http://127.0.0.1:4096
  --cwd PATH          Workspace directory sent to gateway
  --json              JSON output where supported
  --color MODE        auto, always, or never
  --verbose           Print gateway requests to stderr

Examples:
  tura run "Inspect the workspace and summarize the architecture"
  tura run --model openai/gpt-5 --output ndjson "Fix the failing test"
  tura resume --last "Continue from the previous result"
  tura session list --json
`);
}

function printRunHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] run [PROMPT...] [RUN_OPTIONS]

Run options:
  --session ID                  append the prompt to an existing session
  -m, --model PROVIDER/MODEL    request-scoped model override
  -p, --priority                enable priority model routing for this model
  -a, --agent-id ID             request-scoped agent id loaded from agents/ or built-ins
  --session-type TYPE           session type passed to gateway
  --model-variant LEVEL         reasoning/model variant override
  --model-reasoning-effort LEVEL alias for --model-variant
  --reasoning-effort LEVEL      alias for --model-variant
  --model-acceleration          enable priority/accelerated model routing
  --no-model-acceleration       disable priority/accelerated routing
  --force-multiple-tasks        enable the multiple_tasks capability path
  --no-force-multiple-tasks     disable forced multiple_tasks
  --output text|json|ndjson     output format
  --json                        alias for --output json
  --stream, --no-stream         stream gateway events or poll for completion
  --timeout SEC                 timeout before aborting the turn (default 600)
  --last-message-file PATH      write the final assistant message to PATH
  -c, --config KEY=VALUE        runtime override, for example model=gpt-5
`);
}

function printResumeHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] resume SESSION_ID [PROMPT...]
  tura [OPTIONS] resume --last [PROMPT...]

Options:
  --last                    select the most recently updated session
  --output text|json|ndjson output mode when sending a follow-up prompt
  --json                    alias for --output json
`);
}

function printSessionHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] session list [--all] [--json]
  tura [OPTIONS] session plan [--all] [--archived] [--status STATUS] [--json]
  tura [OPTIONS] session show SESSION_ID [--json]
  tura [OPTIONS] session set-status SESSION_ID todo|doing|question|done|archived
  tura [OPTIONS] session update SESSION_ID [--status STATUS] [--plan-summary TEXT] [--task-summary TEXT]
  tura [OPTIONS] session create-ticket SUMMARY [--session SESSION_ID] [--status STATUS]
  tura [OPTIONS] session delete SESSION_ID

Plan options:
  --start-condition session_idle|user_action|scheduled_task|polling_task
  --start-at LOCAL_OR_ISO_TIME
  --poll m=0,d=0,h=1,s=0
  --step N
`);
}

function printConfigHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] config get [KEY]
  tura [OPTIONS] config set KEY=VALUE...

Config is read and written through gateway session config for the selected cwd.
`);
}

function printProviderHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] provider list [--json]
  tura [OPTIONS] provider status [PROVIDER]
  tura [OPTIONS] provider logout PROVIDER
`);
}

function printPermissionHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] permission list [--json]
  tura [OPTIONS] permission reply REQUEST_ID --approve
  tura [OPTIONS] permission reply REQUEST_ID --deny
`);
}

function printCommandHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] command list [--json]
  tura [OPTIONS] command run NAME [ARGS...]
`);
}

function printAgentHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] agent list [--json]
  tura [OPTIONS] agent show AGENT_ID [--json]
  tura [OPTIONS] agent create AGENT_ID [--config JSON_OR_PATH] [--prompt TEXT|--prompt-file PATH]
  tura [OPTIONS] agent update AGENT_ID [--config JSON_OR_PATH] [--prompt TEXT|--prompt-file PATH]
  tura [OPTIONS] agent delete AGENT_ID

Agents are read and written through gateway /agent and dynamic agents live under agents/.
Start a session with: tura run -a AGENT_ID -m PROVIDER/MODEL -p "prompt"
`);
}

function printStatusHelp(): void {
  process.stdout.write(`Usage:
  tura [OPTIONS] status [--json]
`);
}

function printCompletionHelp(): void {
  process.stdout.write(`Usage:
  tura completion bash
  tura completion zsh
  tura completion fish
`);
}
