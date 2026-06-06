import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { TerminalCapabilities } from "../tui/capabilities.js";
import { t } from "../i18n.js";

type StartupStep = "checking" | "building" | "starting" | "waiting";

const gatewayBinaryName = process.platform === "win32" ? "gateway.exe" : "gateway";

export async function ensureGatewayAvailable(
  gatewayUrl: string,
  capabilities: TerminalCapabilities,
): Promise<void> {
  if (await healthOk(gatewayUrl)) return;
  const root = findRepoRoot();
  const binary = join(root, "target", "debug", gatewayBinaryName);
  if (!existsSync(binary)) {
    await runWithSpinner({
      step: "building",
      text: t("gatewayBuilding"),
      capabilities,
      run: () => runProcess("cargo", ["build", "-p", "gateway", "--bin", "gateway"], root),
    });
  }
  await runWithSpinner({
    step: "starting",
    text: t("gatewayStarting"),
    capabilities,
    run: async () => {
      const port = gatewayPort(gatewayUrl);
      const child = spawn(binary, [], {
        cwd: root,
        detached: true,
        stdio: "ignore",
        windowsHide: true,
        env: { ...process.env, ...(port ? { PORT: port } : {}) },
      });
      child.unref();
    },
  });
  await waitForGateway(gatewayUrl, capabilities);
}

async function waitForGateway(
  gatewayUrl: string,
  capabilities: TerminalCapabilities,
): Promise<void> {
  await runWithSpinner({
    step: "waiting",
    text: t("gatewayWaiting"),
    capabilities,
    run: async (tick) => {
      const deadline = Date.now() + 45_000;
      while (Date.now() < deadline) {
        if (await healthOk(gatewayUrl)) return;
        tick();
        await delay(350);
      }
      throw new Error(t("gatewayStartTimeout"));
    },
  });
}

async function healthOk(gatewayUrl: string): Promise<boolean> {
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 1500);
    const response = await fetch(`${gatewayUrl.replace(/\/+$/u, "")}/global/health`, {
      signal: controller.signal,
    });
    clearTimeout(timer);
    return response.ok;
  } catch {
    return false;
  }
}

async function runWithSpinner(options: {
  step: StartupStep;
  text: string;
  capabilities: TerminalCapabilities;
  run: (tick: () => void) => Promise<void>;
}): Promise<void> {
  let frame = 0;
  const frames = options.capabilities.unicode
    ? ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    : ["|", "/", "-", "\\"];
  const draw = () => {
    if (!process.stdout.isTTY) return;
    const prefix = frames[frame % frames.length];
    frame += 1;
    const line = `${prefix} ${options.text}`;
    process.stdout.write(options.capabilities.cursorControl ? `\r\x1b[2K${line}` : `${line}\n`);
  };
  draw();
  const timer = setInterval(draw, 350);
  try {
    await options.run(draw);
    if (process.stdout.isTTY) {
      const done = `✓ ${options.text}`;
      process.stdout.write(
        options.capabilities.cursorControl && options.capabilities.unicode
          ? `\r\x1b[2K${done}\n`
          : "\n",
      );
    }
  } finally {
    clearInterval(timer);
  }
}

function runProcess(command: string, args: string[], cwd: string): Promise<void> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, { cwd, stdio: "ignore", windowsHide: true });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolvePromise();
      else reject(new Error(`${command} ${args.join(" ")} exited with ${code ?? "signal"}`));
    });
  });
}

function findRepoRoot(): string {
  const starts = [process.cwd(), dirname(fileURLToPath(import.meta.url))];
  for (const start of starts) {
    let current = resolve(start);
    for (let depth = 0; depth < 8; depth += 1) {
      if (isRuntimeRoot(current)) return current;
      const parent = dirname(current);
      if (parent === current) break;
      current = parent;
    }
  }
  return process.cwd();
}

function isRuntimeRoot(candidate: string): boolean {
  return (
    (existsSync(join(candidate, "Cargo.toml")) &&
      existsSync(join(candidate, "crates", "gateway"))) ||
    (existsSync(join(candidate, "agents", "src")) &&
      existsSync(join(candidate, "personas", "src"))) ||
    existsSync(join(candidate, "config", "provider_config.json"))
  );
}

function gatewayPort(gatewayUrl: string): string | undefined {
  try {
    return new URL(gatewayUrl).port || undefined;
  } catch {
    return undefined;
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, ms));
}
