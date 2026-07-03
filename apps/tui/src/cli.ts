import { spawn } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { resolveGatewayUrl, gatewayUrlIsExplicit, resolveCwd } from "./gateway/directory.js";
import { ensureGatewayAvailable } from "./gateway/autostart.js";
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
import { plainCapabilities } from "./tui/capabilities.js";
import {
  runtimeOverridesFromAssignment,
  shellValue,
  type CommandRunShell,
  type RuntimeConfigOverrides,
} from "./commands/config-values.js";
import { formatHelp } from "./output/help.js";
import { parseLanguage, setLanguage, t, type Language } from "./i18n.js";
import { helpPage, type HelpTopic } from "./i18n-help.js";

const DEFAULT_AGENT = "balanced";
const DEFAULT_MODEL_VARIANT = "high";
const DEFAULT_MODEL_ACCELERATION_ENABLED = false;

export async function main(argv: string[]): Promise<void> {
  if (argv[0] === "exec") {
    await runRustCliExec(argv.slice(1));
    return;
  }
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
    if (command === "exec") {
      await runRustCliExec(args);
      return;
    }
    if (command === "run") {
      if (hasHelp(args)) {
        printRunHelp();
        return;
      }
      const parsed = parseRun(args, context.json);
      await runPrompt(await gatewayContext(context), parsed);
      return;
    }
    const commandRunShell = commandRunShellForCommand(command);
    if (commandRunShell) {
      if (hasHelp(args)) {
        printRunHelp();
        return;
      }
      const parsed = parseRun(args, context.json, commandRunShell);
      await runPrompt(await gatewayContext(context), parsed);
      return;
    }
    if (command === "resume") {
      if (hasHelp(args)) {
        printResumeHelp();
        return;
      }
      const parsed = parseResume(args, context.json);
      await resumeCommand(await gatewayContext(context), parsed);
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
    if (command === "session") return sessionCommand(await gatewayContext(context), args);
    if (command === "config") return configCommand(await gatewayContext(context), args);
    if (command === "provider") return providerCommand(await gatewayContext(context), args);
    if (command === "agent") return agentCommand(await gatewayContext(context), args);
    if (command === "persona") return personaCommand(await gatewayContext(context), args);
    if (command === "project") return projectCommand(await gatewayContext(context), args);
    if (command === "file") return fileCommand(await gatewayContext(context), args);
    if (command === "command") return commandRegistryCommand(await gatewayContext(context), args);
    if (command === "inspect") return inspectCommand(await gatewayContext(context), args);
    if (command === "gateway") return gatewayCommand(await gatewayContext(context), args);
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

async function gatewayContext(context: CliContext): Promise<CliContext> {
  if (context.mock) return context;
  const gatewayUrl = await ensureGatewayAvailable(
    context.gatewayUrl,
    plainCapabilities(),
    context.dev,
    context.gatewayUrlExplicit,
  );
  return { ...context, gatewayUrl };
}

function parseGlobal(argv: string[]): { context: CliContext; args: string[] } {
  const args = [...argv];
  let gatewayUrl: string | undefined;
  let cwd: string | undefined;
  let color: ColorMode = "auto";
  let display: DisplayMode = "auto";
  let language: Language | undefined;
  let initialSessionId = process.env.TURA_TUI_INITIAL_SESSION_ID?.trim() || undefined;
  let json = false;
  let verbose = false;
  let mock = process.env.TURA_TUI_MOCK === "1" || process.env.TURA_TUI_MOCK === "true";
  let dev = process.env.TURA_DEV === "1" || process.env.TURA_DEV === "true";
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
    } else if (arg === "--initial-session") {
      initialSessionId = takeValue(args, index--);
    } else if (arg.startsWith("--initial-session=")) {
      initialSessionId = arg.slice("--initial-session=".length);
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
    } else if (arg === "--dev") {
      dev = true;
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
      gatewayUrlExplicit: gatewayUrlIsExplicit(gatewayUrl),
      cwd: resolveCwd(cwd),
      json,
      color,
      display,
      language,
      initialSessionId,
      verbose,
      mock,
      dev,
    },
    args,
  };
}

export function parseRun(
  args: string[],
  rootJson: boolean,
  commandRunShellOverride?: CommandRunShell,
): Parameters<typeof runPrompt>[1] {
  let sessionID: string | undefined;
  let model: string | undefined;
  let agent: string | undefined;
  let sessionType: string | undefined;
  let modelVariant: string | undefined;
  let modelAccelerationEnabled: boolean | undefined;
  let killProcessesOnStart: boolean | undefined;
  let validatorEnabled: boolean | undefined;
  let commandRunShell: CommandRunShell | undefined = commandRunShellOverride;
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
    else if (isCommandRunShellFlag(arg)) commandRunShell = shellValue(arg.slice(2));
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
        commandRunShell,
      } = applyRunOverrides(
        {
          model,
          agent,
          sessionType,
          modelVariant,
          modelAccelerationEnabled,
          killProcessesOnStart,
          validatorEnabled,
          commandRunShell,
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
        commandRunShell,
      } = applyRunOverrides(
        {
          model,
          agent,
          sessionType,
          modelVariant,
          modelAccelerationEnabled,
          killProcessesOnStart,
          validatorEnabled,
          commandRunShell,
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
    modelVariant: modelVariant ?? DEFAULT_MODEL_VARIANT,
    modelAccelerationEnabled: modelAccelerationEnabled ?? DEFAULT_MODEL_ACCELERATION_ENABLED,
    killProcessesOnStart,
    validatorEnabled,
    commandRunShell,
    output,
    stream,
    timeoutSec,
    lastMessageFile,
    source: "cli",
  };
}

export function commandRunShellForCommand(command: string): CommandRunShell | undefined {
  if (command === "bash" || command === "zsh" || command === "shel") return shellValue(command);
  return undefined;
}

function isCommandRunShellFlag(value: string): boolean {
  return value === "--bash" || value === "--zsh" || value === "--shel";
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

async function runRustCliExec(args: string[]): Promise<void> {
  const executable = resolveRustCliBinary();
  if (!executable) {
    throw new CliUsageError(
      "tura_exec binary not found. Run scripts/build-debug or scripts/build-release first.",
    );
  }
  const code = await new Promise<number>((resolveExec, reject) => {
    const child = spawn(executable, ["exec", ...args], {
      stdio: ["inherit", "pipe", "pipe"],
      env: process.env,
      windowsHide: true,
    });
    child.stdout?.pipe(process.stdout);
    child.stderr?.pipe(process.stderr);
    child.on("error", reject);
    child.on("exit", (exitCode, signal) => {
      child.stdout?.destroy();
      child.stderr?.destroy();
      if (signal) resolveExec(1);
      else resolveExec(exitCode ?? 1);
    });
  });
  process.exitCode = code;
}

function resolveRustCliBinary(): string | undefined {
  const executable = process.platform === "win32" ? "tura_exec.exe" : "tura_exec";
  const candidates: string[] = [];
  const execDir = dirname(process.execPath);
  candidates.push(join(execDir, executable));
  const repo = findRepoRootForExec();
  candidates.push(join(repo, "target", "release", executable));
  candidates.push(join(repo, "target", "debug", executable));
  return candidates.find((candidate) => existsSync(candidate));
}

function findRepoRootForExec(): string {
  const starts = [
    process.cwd(),
    dirname(process.execPath),
    dirname(fileURLToPath(import.meta.url)),
  ];
  for (const start of starts) {
    let current = canonicalPath(start);
    for (let depth = 0; depth < 8; depth += 1) {
      if (
        existsSync(join(current, "Cargo.toml")) &&
        existsSync(join(current, "crates", "gateway"))
      ) {
        return current;
      }
      const parent = dirname(current);
      if (parent === current) break;
      current = parent;
    }
  }
  return process.cwd();
}

function canonicalPath(value: string): string {
  try {
    return realpathSync(value);
  } catch {
    return resolve(value);
  }
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
