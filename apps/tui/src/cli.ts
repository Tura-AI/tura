import { resolveGatewayUrl, resolveCwd } from "./gateway/directory.js";
import {
  CliUsageError,
  type CliContext,
  type ColorMode,
  type DisplayMode,
  type OutputMode,
} from "./types/common.js";
import { runPrompt } from "./commands/run.js";
import { resumeCommand } from "./commands/resume.js";
import { sessionCommand } from "./commands/session.js";
import { configCommand } from "./commands/config.js";
import { providerCommand } from "./commands/provider.js";
import { agentCommand } from "./commands/agent.js";
import { completionCommand } from "./commands/completion.js";
import { projectCommand } from "./commands/project.js";
import { fileCommand } from "./commands/file.js";
import { personaCommand } from "./commands/persona.js";
import { commandRegistryCommand } from "./commands/command-registry.js";
import { gatewayCommand } from "./commands/gateway.js";
import { inspectCommand } from "./commands/inspect.js";
import { runTui } from "./tui/app.js";
import {
  runtimeOverridesFromAssignment,
  type RuntimeConfigOverrides,
} from "./commands/config-values.js";
import { formatHelp } from "./output/help.js";
import { parseLanguage, setLanguage, t, type Language } from "./i18n.js";
import { helpPage, type HelpTopic } from "./i18n-help.js";

const DEFAULT_AGENT = "fast";
const DEFAULT_MODEL_ACCELERATION_ENABLED = true;

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
      if (command === "agent") return printAgentHelp();
      if (command === "persona") return printPersonaHelp();
      if (command === "project") return printProjectHelp();
      if (command === "file") return printFileHelp();
      if (command === "command") return printCommandHelp();
      if (command === "inspect") return printInspectHelp();
      if (command === "gateway") return printGatewayHelp();
      if (command === "completion") return printCompletionHelp();
    }
    if (command === "session") return sessionCommand(context, args);
    if (command === "config") return configCommand(context, args);
    if (command === "provider") return providerCommand(context, args);
    if (command === "agent") return agentCommand(context, args);
    if (command === "persona") return personaCommand(context, args);
    if (command === "project") return projectCommand(context, args);
    if (command === "file") return fileCommand(context, args);
    if (command === "command") return commandRegistryCommand(context, args);
    if (command === "inspect") return inspectCommand(context, args);
    if (command === "gateway") return gatewayCommand(context, args);
    if (command === "completion") return completionCommand(args);
    await runTui(context, [command, ...args].join(" "));
  } catch (error) {
    const exitCode =
      typeof error === "object" && error && "exitCode" in error
        ? Number((error as { exitCode: number }).exitCode)
        : 1;
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
  let display: DisplayMode = "auto";
  let language: Language | undefined;
  let json = false;
  let verbose = false;
  let mock = process.env.TURA_TUI_MOCK === "1" || process.env.TURA_TUI_MOCK === "true";
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
    } else if (arg === "--mock") {
      mock = true;
      args.splice(index--, 1);
    } else if (arg === "--color") color = takeValue(args, index--) as ColorMode;
    else if (arg.startsWith("--color=")) {
      color = arg.slice("--color=".length) as ColorMode;
      args.splice(index--, 1);
    } else if (arg === "--plain") {
      display = "plain";
      color = "never";
      args.splice(index--, 1);
    } else if (arg === "--rich") {
      display = "rich";
      args.splice(index--, 1);
    } else if (arg === "--lang" || arg === "--language") {
      const parsed = parseLanguage(takeValue(args, index--));
      if (!parsed) throw new CliUsageError(t("unsupportedLanguage"));
      language = parsed;
    } else if (arg.startsWith("--lang=")) {
      const parsed = parseLanguage(arg.slice("--lang=".length));
      if (!parsed) throw new CliUsageError(t("unsupportedLanguage"));
      language = parsed;
      args.splice(index--, 1);
    } else if (arg.startsWith("--language=")) {
      const parsed = parseLanguage(arg.slice("--language=".length));
      if (!parsed) throw new CliUsageError(t("unsupportedLanguage"));
      language = parsed;
      args.splice(index--, 1);
    }
  }
  setLanguage(language);
  return {
    context: {
      gatewayUrl: resolveGatewayUrl(gatewayUrl),
      cwd: resolveCwd(cwd),
      json,
      color,
      display,
      language,
      verbose,
      mock,
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
    else if (arg === "--agent" || arg === "--agent-id" || arg === "--agent-name" || arg === "-a")
      agent = args[++index];
    else if (arg.startsWith("--agent=")) agent = arg.slice("--agent=".length);
    else if (arg.startsWith("--agent-id=")) agent = arg.slice("--agent-id=".length);
    else if (arg === "--session-type") sessionType = args[++index];
    else if (arg.startsWith("--session-type=")) sessionType = arg.slice("--session-type=".length);
    else if (
      arg === "--model-variant" ||
      arg === "--variant" ||
      arg === "--reasoning-effort" ||
      arg === "--model-reasoning-effort"
    ) {
      modelVariant = args[++index];
    } else if (arg.startsWith("--model-variant="))
      modelVariant = arg.slice("--model-variant=".length);
    else if (arg.startsWith("--model-reasoning-effort="))
      modelVariant = arg.slice("--model-reasoning-effort=".length);
    else if (arg.startsWith("--reasoning-effort="))
      modelVariant = arg.slice("--reasoning-effort=".length);
    else if (arg === "--model-acceleration" || arg === "--accelerated")
      modelAccelerationEnabled = true;
    else if (arg === "--no-model-acceleration" || arg === "--no-accelerated")
      modelAccelerationEnabled = false;
    else if (arg === "-p" || arg === "--priority") modelAccelerationEnabled = true;
    else if (arg === "--output") output = parseOutput(args[++index]);
    else if (arg === "--json") output = "json";
    else if (arg === "--stream") stream = true;
    else if (arg === "--no-stream") stream = false;
    else if (arg === "--timeout") timeoutSec = Number(args[++index]);
    else if (arg === "--last-message-file") lastMessageFile = args[++index];
    else if (arg === "-c" || arg === "--config") {
      const overrides = runtimeOverridesFromAssignment(args[++index]);
      ({
        model,
        agent,
        sessionType,
        modelVariant,
        modelAccelerationEnabled,
        killProcessesOnStart,
        validatorEnabled,
      } = applyRunOverrides(
        {
          model,
          agent,
          sessionType,
          modelVariant,
          modelAccelerationEnabled,
          killProcessesOnStart,
          validatorEnabled,
        },
        overrides,
      ));
    } else if (arg.startsWith("--config=")) {
      const overrides = runtimeOverridesFromAssignment(arg.slice("--config=".length));
      ({
        model,
        agent,
        sessionType,
        modelVariant,
        modelAccelerationEnabled,
        killProcessesOnStart,
        validatorEnabled,
      } = applyRunOverrides(
        {
          model,
          agent,
          sessionType,
          modelVariant,
          modelAccelerationEnabled,
          killProcessesOnStart,
          validatorEnabled,
        },
        overrides,
      ));
    } else prompt.push(arg);
  }
  if (!prompt.join(" ").trim()) throw new CliUsageError(t("runRequiresPrompt"));
  return {
    prompt: prompt.join(" "),
    sessionID,
    model,
    agent: agent ?? DEFAULT_AGENT,
    sessionType,
    modelVariant,
    modelAccelerationEnabled: modelAccelerationEnabled ?? DEFAULT_MODEL_ACCELERATION_ENABLED,
    killProcessesOnStart,
    validatorEnabled,
    output,
    stream,
    timeoutSec,
    lastMessageFile,
    source: "cli",
  };
}

function applyRunOverrides(
  current: RuntimeConfigOverrides,
  next: RuntimeConfigOverrides,
): RuntimeConfigOverrides {
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
  throw new CliUsageError(t("invalidOutputMode", { value }));
}

function takeValue(args: string[], index: number): string {
  const value = args[index + 1];
  if (!value) throw new CliUsageError(t("valueRequiresValue", { name: args[index] }));
  args.splice(index, 2);
  return value;
}

function printHelp(): void {
  printHelpTopic("main");
}

function printRunHelp(): void {
  printHelpTopic("run");
}

function printResumeHelp(): void {
  printHelpTopic("resume");
}

function printSessionHelp(): void {
  printHelpTopic("session");
}

function printConfigHelp(): void {
  printHelpTopic("config");
}

function printProviderHelp(): void {
  printHelpTopic("provider");
}

function printAgentHelp(): void {
  printHelpTopic("agent");
}

function printCompletionHelp(): void {
  printHelpTopic("completion");
}

function printPersonaHelp(): void {
  printHelpTopic("persona");
}

function printProjectHelp(): void {
  printHelpTopic("project");
}

function printFileHelp(): void {
  printHelpTopic("file");
}

function printCommandHelp(): void {
  printHelpTopic("command");
}

function printInspectHelp(): void {
  printHelpTopic("inspect");
}

function printGatewayHelp(): void {
  printHelpTopic("gateway");
}

function printHelpTopic(topic: HelpTopic): void {
  process.stdout.write(formatHelp(helpPage(topic)));
}
