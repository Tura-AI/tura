#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { launchEnvironment, missingRuntimeResources } from "./launcher-env.mjs";
import {
  executableName,
  executableNames,
  npmPlatformRuntimeExcludedFiles,
  platformPackageName,
  releaseRuntimeExcludedDirs,
  releaseRuntimeFiles,
} from "./release-artifacts.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

function fail(message) {
  throw new Error(`[tura verify-installed-runtime] ${message}`);
}

function option(name) {
  const prefix = `${name}=`;
  const inline = process.argv.slice(2).find((argument) => argument.startsWith(prefix));
  if (inline) return path.resolve(inline.slice(prefix.length));
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1] ? path.resolve(process.argv[index + 1]) : null;
}

function walkFiles(root) {
  if (!existsSync(root)) return [];
  if (statSync(root).isFile()) return [root];
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (entry.isDirectory() && releaseRuntimeExcludedDirs.includes(entry.name)) continue;
    const candidate = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(candidate));
    if (entry.isFile()) files.push(candidate);
  }
  return files.sort();
}

function runtimeInventory(releaseDir) {
  const files = [];
  for (const [sourceRelative, releaseRelative] of releaseRuntimeFiles) {
    if (npmPlatformRuntimeExcludedFiles.includes(releaseRelative)) continue;
    const source = path.join(repoRoot, sourceRelative);
    if (!existsSync(source)) fail(`release source is missing: ${sourceRelative}`);
    if (statSync(source).isFile()) {
      files.push(path.join(releaseDir, releaseRelative));
      continue;
    }
    for (const sourceFile of walkFiles(source)) {
      files.push(path.join(releaseDir, releaseRelative, path.relative(source, sourceFile)));
    }
  }
  return files;
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    fail(`cannot parse JSON config ${file}: ${error.message}`);
  }
}

function expectedAgents() {
  return readdirSync(path.join(repoRoot, "agents", "src"), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(repoRoot, "agents", "src", entry.name, "agent_config.json"))
    .filter(existsSync)
    .map((file) => readJson(file));
}

function expectedPersonas() {
  return readdirSync(path.join(repoRoot, "personas", "src"), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(repoRoot, "personas", "src", entry.name, "persona_config.json"))
    .filter(existsSync)
    .map((file) => readJson(file));
}

function manifestId(file) {
  const match = readFileSync(file, "utf8").match(/^id\s*=\s*"([^"]+)"/mu);
  if (!match) fail(`tool manifest has no id: ${file}`);
  return match[1];
}

function expectedTools() {
  const roots = [
    path.join(repoRoot, "crates", "tools", "src", "commands"),
    path.join(repoRoot, "commands"),
  ];
  return roots.flatMap((root) =>
    readdirSync(root, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => path.join(root, entry.name, "command.toml"))
      .filter(existsSync)
      .map(manifestId),
  ).sort();
}

function runJson(binPath, gatewayUrl, runtimeEnv, args) {
  const result = spawnSync(binPath, ["--gateway-url", gatewayUrl, ...args], {
    cwd: path.dirname(path.dirname(binPath)),
    env: runtimeEnv,
    shell: process.platform === "win32",
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) fail(`${args.join(" ")} failed: ${result.error.message}`);
  if ((result.status ?? 1) !== 0) {
    fail(`${args.join(" ")} exited ${result.status}: ${(result.stderr || result.stdout).slice(0, 800)}`);
  }
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    fail(`${args.join(" ")} returned invalid JSON: ${result.stdout.slice(0, 800)} (${error.message})`);
  }
}

function requireExactIds(label, actualIds, expectedIds) {
  const actual = [...new Set(actualIds)].sort();
  const expected = [...new Set(expectedIds)].sort();
  const missing = expected.filter((id) => !actual.includes(id));
  const unexpected = actual.filter((id) => !expected.includes(id));
  if (missing.length || unexpected.length) {
    fail(`${label} mismatch; missing: ${missing.join(", ") || "none"}; unexpected: ${unexpected.join(", ") || "none"}`);
  }
}

async function freePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => server.once("error", reject).listen(0, "127.0.0.1", resolve));
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  await new Promise((resolve) => server.close(resolve));
  if (!port) fail("could not reserve a gateway test port");
  return port;
}

async function fetchJson(url, attempts = 120) {
  let lastError;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const response = await fetch(url);
      if (response.ok) return await response.json();
      lastError = new Error(`HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  fail(`gateway did not return ${url}: ${lastError?.message ?? "unknown error"}`);
}

async function stopGateway(child) {
  if (child.exitCode !== null) return;
  child.stdin.end();
  const exited = new Promise((resolve) => child.once("exit", resolve));
  const timeout = new Promise((resolve) => setTimeout(() => resolve("timeout"), 10_000));
  if (await Promise.race([exited, timeout]) === "timeout" && child.exitCode === null) {
    child.kill();
  }
}

export async function verifyInstalledRuntime({ installRoot, binPath }) {
  const mainPackageRoot = path.join(installRoot, "tura-ai");
  const packageJson = readJson(path.join(mainPackageRoot, "package.json"));
  const wrapperPath = path.join(mainPackageRoot, "npm", "tura.mjs");
  const launcherPath = path.join(mainPackageRoot, "scripts", "npm", "launcher-env.mjs");
  if (!existsSync(wrapperPath) || !existsSync(launcherPath)) {
    fail("installed main package is missing the npm runtime launcher");
  }
  const wrapperSource = readFileSync(wrapperPath, "utf8");
  const launcherSource = readFileSync(launcherPath, "utf8");
  if (!/launchEnvironment\(\{\s*providerConfig,\s*releaseBin,\s*releaseDir\s*\}\)/mu.test(wrapperSource)) {
    fail("installed wrapper does not launch with the platform release runtime environment");
  }
  if (!/TURA_PROJECT_ROOT:\s*baseEnv\.TURA_PROJECT_ROOT\s*\|\|\s*releaseDir/mu.test(launcherSource)) {
    fail("installed launcher does not resolve TURA_PROJECT_ROOT to the platform release directory");
  }
  const platformName = platformPackageName();
  const platformRoot = [
    path.join(installRoot, platformName),
    path.join(mainPackageRoot, "node_modules", platformName),
  ].find(existsSync) ?? path.join(installRoot, platformName);
  const releaseDir = path.join(platformRoot, "target", "release");
  const releaseBin = path.join(releaseDir, executableName("tura"));
  const providerConfig = path.join(releaseDir, "config", "provider_config.json");

  const missingResources = missingRuntimeResources(releaseDir);
  const expectedRuntimeFiles = runtimeInventory(releaseDir);
  const requiredExecutables = executableNames.map((name) => path.join(releaseDir, executableName(name)));
  const missing = [...missingResources, ...expectedRuntimeFiles, ...requiredExecutables]
    .filter((file, index, values) => !existsSync(file) && values.indexOf(file) === index);
  if (missing.length) fail(`installed platform package is incomplete:\n${missing.join("\n")}`);

  for (const file of expectedRuntimeFiles.filter((candidate) => path.extname(candidate).toLowerCase() === ".json")) {
    readJson(file);
  }

  const agents = expectedAgents();
  const personas = expectedPersonas();
  const toolIds = expectedTools();
  const providerCatalog = readJson(providerConfig)?.model_catalog?.providers;
  if (!providerCatalog || Object.keys(providerCatalog).length === 0) {
    fail("packaged provider_config.json has no model_catalog.providers entries");
  }

  const runtimeHome = path.join(path.dirname(installRoot), ".tura-runtime-verification");
  const runtimeEnv = launchEnvironment({
    baseEnv: {
      ...process.env,
      TURA_HOME: runtimeHome,
      TURA_GATEWAY_TRAY: "0",
    },
    providerConfig,
    releaseBin,
    releaseDir,
  });
  const port = await freePort();
  const gatewayUrl = `http://127.0.0.1:${port}`;
  const gateway = spawn(path.join(releaseDir, executableName("tura_gateway")), [], {
    cwd: path.dirname(installRoot),
    env: {
      ...runtimeEnv,
      PORT: String(port),
      TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF: "1",
    },
    stdio: ["pipe", "pipe", "pipe"],
    windowsHide: true,
  });
  let gatewayErrors = "";
  gateway.stderr.on("data", (chunk) => { gatewayErrors += chunk.toString(); });

  try {
    await fetchJson(`${gatewayUrl}/global/health`);

    const listedAgents = runJson(binPath, gatewayUrl, runtimeEnv, ["agent", "list", "--json"]);
    requireExactIds("agent list", listedAgents.map((entry) => entry.id ?? entry.name), agents.map((agent) => agent.agent_name));
    for (const expected of agents) {
      const loaded = runJson(binPath, gatewayUrl, runtimeEnv, ["agent", "show", expected.agent_name, "--json"]);
      if (loaded?.config?.agent_name !== expected.agent_name || !loaded?.prompt?.trim()) {
        fail(`agent ${expected.agent_name} did not load its config and prompt`);
      }
    }

    const listedPersonas = runJson(binPath, gatewayUrl, runtimeEnv, ["persona", "list", "--json"]);
    requireExactIds("persona list", listedPersonas.map((entry) => entry?.summary?.id ?? entry?.id), personas.map((persona) => persona.persona_name));
    for (const expected of personas) {
      const loaded = runJson(binPath, gatewayUrl, runtimeEnv, ["persona", "show", expected.persona_name, "--json"]);
      if (loaded?.config?.persona_name !== expected.persona_name || !loaded?.persona?.trim()) {
        fail(`persona ${expected.persona_name} did not load its config and prompt`);
      }
    }

    const providers = runJson(binPath, gatewayUrl, runtimeEnv, ["provider", "list", "--json"]);
    if (!Array.isArray(providers?.all) || providers.all.length === 0) {
      fail("provider list is empty; packaged provider_config.json was not loaded");
    }

    const tools = await fetchJson(`${gatewayUrl}/tool`);
    requireExactIds("tool registry", tools.map((tool) => tool.id), toolIds);
    const unavailable = tools.filter((tool) => !tool.enabled || tool.state === "Unavailable");
    if (unavailable.length) fail(`packaged tools are not exposed: ${unavailable.map((tool) => tool.id).join(", ")}`);

    const exposedNames = new Set(tools.flatMap((tool) => [tool.id, ...(tool.aliases ?? [])]));
    for (const agent of agents) {
      const capabilities = (agent.agent_capabilities ?? [])
        .map((capability) => capability.capability_name)
        .filter(Boolean);
      if (!capabilities.length) fail(`agent ${agent.agent_name} has no configured capabilities`);
      const unresolved = capabilities.filter((capability) => {
        if (capability === "shells") return !["bash", "zsh", "shell_command"].some((name) => exposedNames.has(name));
        return !exposedNames.has(capability);
      });
      if (unresolved.length) fail(`agent ${agent.agent_name} references unexposed tools: ${unresolved.join(", ")}`);
    }
  } catch (error) {
    if (gateway.exitCode !== null && gatewayErrors.trim()) {
      error.message += `\nGateway stderr:\n${gatewayErrors.slice(-2000)}`;
    }
    throw error;
  } finally {
    await stopGateway(gateway);
  }

  console.log(`[tura verify-installed-runtime] ${packageJson.name}@${packageJson.version}: all packaged configs and ${agents.length} agents, ${personas.length} personas, ${toolIds.length} tools verified`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const installRoot = option("--install-root");
  const binPath = option("--bin");
  if (!installRoot || !binPath) fail("usage: verify-installed-runtime.mjs --install-root PATH --bin PATH");
  await verifyInstalledRuntime({ installRoot, binPath });
}
