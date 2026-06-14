import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import fsp from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const harnessDir = path.dirname(fileURLToPath(import.meta.url));
export const repoRoot = path.resolve(harnessDir, "..", "..");
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const defaultModel = process.env.TURA_BUSINESS_MODEL || "codex/gpt-5.5";
const defaultAgent = process.env.TURA_BUSINESS_AGENT || "fast";
const defaultVariant = process.env.TURA_BUSINESS_MODEL_VARIANT || "low";
const binaryProfile = process.env.TURA_BUSINESS_BINARY_PROFILE || "release";
const defaultTimeoutMsByCase = {
  "single-request": 180_000,
  snake: 240_000,
  "password-zip": 240_000,
};

export const caseNames = ["single-request", "snake", "password-zip"];

export function binaryDir() {
  return path.join(repoRoot, "target", binaryProfile);
}

export function releaseBinary(name) {
  return path.join(binaryDir(), `${name}${exeSuffix}`);
}

export function assertReleaseArtifacts() {
  const required = [
    "tura",
    "tura_exec",
    "tura_gateway",
    "tura_router",
    "tura_runtime",
    "tura_session_db",
  ];
  const missing = required
    .map(releaseBinary)
    .filter((candidate) => !fs.existsSync(candidate));
  if (missing.length > 0) {
    throw new Error(
      [
        "Missing release artifacts:",
        ...missing.map((item) => `- ${item}`),
        `Run scripts/build-${binaryProfile}.ps1 or scripts/build-${binaryProfile}.sh first.`,
      ].join("\n"),
    );
  }
}

export function assertReleaseGuiArtifacts() {
  const index = path.join(binaryDir(), "gui", "index.html");
  if (!fs.existsSync(index)) {
    throw new Error(
      [
        `Missing ${binaryProfile} GUI artifact: ${index}`,
        `Run scripts/build-${binaryProfile}.ps1 or scripts/build-${binaryProfile}.sh first.`,
      ].join("\n"),
    );
  }
}

export function runIdFor(surface, caseName) {
  return (
    process.env.TURA_BUSINESS_RUN_ID || `${surface}-${caseName}-${timestamp()}`
  );
}

export function releaseRunPaths(surface, caseName) {
  const runId = runIdFor(surface, caseName);
  const root =
    process.env.TURA_BUSINESS_TARGET_ROOT ||
    process.env.COMMAND_RUN_BUSINESS_TARGET_ROOT ||
    defaultBusinessTargetRoot(surface);
  const runRoot = path.join(root, binaryProfile, surface, caseName, runId);
  return {
    surface,
    caseName,
    runId,
    runRoot,
    workspace: path.join(runRoot, "workspace"),
    logs: path.join(runRoot, "logs"),
    summaryPath: path.join(runRoot, "summary.json"),
    lastMessagePath: path.join(runRoot, "last-message.txt"),
    providerLogRoot: path.join(runRoot, "logs", "provider"),
    turaHome: path.join(runRoot, "tura-home"),
  };
}

function defaultBusinessTargetRoot(surface) {
  if (surface === "tui")
    return path.join(repoRoot, "apps", "tui", "test-results", "release");
  return path.join(repoRoot, "target", "business");
}

export async function runCliReleaseCase(caseName) {
  const timeoutMs = caseTimeoutMs(caseName);
  return runTextEntryCase("cli", caseName, async (ctx, prompt) => {
    const args = [
      "exec",
      "-C",
      ctx.workspace,
      "-m",
      defaultModel,
      "-a",
      defaultAgent,
      "--model-reasoning-effort",
      defaultVariant,
      "-p",
      "--output-last-message",
      ctx.lastMessagePath,
      prompt,
    ];
    return runLoggedProcess("tura", args, ctx, { timeoutMs });
  });
}

export async function runTuiReleaseCase(caseName) {
  const timeoutMs = caseTimeoutMs(caseName);
  return runTextEntryCase("tui", caseName, async (ctx, prompt) => {
    const port = await freePort();
    const gatewayUrl = `http://127.0.0.1:${port}`;
    const args = [
      "--gateway-url",
      gatewayUrl,
      "--cwd",
      ctx.workspace,
      "run",
      "-m",
      defaultModel,
      "-a",
      defaultAgent,
      "--model-reasoning-effort",
      defaultVariant,
      "-p",
      "--timeout",
      String(Math.ceil(timeoutMs / 1000)),
      "--last-message-file",
      ctx.lastMessagePath,
      prompt,
    ];
    return runLoggedProcess("tura", args, ctx, {
      timeoutMs: timeoutMs + 30_000,
      env: {
        TURA_GATEWAY_PORT: String(port),
        TURA_GATEWAY_URL: gatewayUrl,
      },
    });
  });
}

export async function runGuiReleaseCase(caseName) {
  assertReleaseArtifacts();
  assertReleaseGuiArtifacts();
  const timeoutMs = caseTimeoutMs(caseName);
  const ctx = await prepareContext("gui", caseName);
  const definition = caseDefinition(caseName, ctx);
  let gateway;
  let result;
  let cleanup;
  try {
    gateway = await startReleaseGateway(ctx);
    const env = {
      ...caseEnv(ctx),
      TURA_GATEWAY_URL: gateway.url,
      TURA_GUI_URL: gateway.url,
      TURA_GUI_E2E_OUT: path.join(ctx.runRoot, "gui-artifacts"),
      TURA_SMOKE_MODEL: defaultModel,
      TURA_SMOKE_AGENT: defaultAgent,
      TURA_SMOKE_MARKER: definition.sentinel,
      TURA_SMOKE_PROMPT: definition.prompt,
      TURA_SMOKE_TIMEOUT_S: String(Math.ceil(timeoutMs / 1000)),
      PYTHONUTF8: "1",
    };
    result = await runLoggedProcess(
      pythonCommand(),
      [path.join(repoRoot, "apps", "gui", "e2e", "release_web_gui_smoke.py")],
      ctx,
      {
        timeoutMs: timeoutMs + 60_000,
        env,
      },
    );
  } finally {
    cleanup = await shutdownBackendDaemons(ctx);
    await stopProcess(gateway?.child);
  }
  return finishCase(ctx, definition, result, {
    surface: "gui",
    gatewayUrl: gateway?.url,
    cleanup,
    resultText: await readText(
      path.join(ctx.runRoot, "gui-artifacts", "report.json"),
    ),
  });
}

async function runTextEntryCase(surface, caseName, runner) {
  assertReleaseArtifacts();
  const ctx = await prepareContext(surface, caseName);
  const definition = caseDefinition(caseName, ctx);
  let result;
  let cleanup;
  try {
    result = await runner(ctx, definition.prompt);
  } finally {
    cleanup = await shutdownBackendDaemons(ctx);
  }
  return finishCase(ctx, definition, result, { surface, cleanup });
}

export async function prepareContext(surface, caseName) {
  if (!caseNames.includes(caseName)) {
    throw new Error(
      `Unknown business case ${caseName}. Expected one of: ${caseNames.join(", ")}`,
    );
  }
  const ctx = releaseRunPaths(surface, caseName);
  await fsp.rm(ctx.runRoot, { recursive: true, force: true });
  await fsp.mkdir(ctx.workspace, { recursive: true });
  await fsp.mkdir(ctx.logs, { recursive: true });
  if (caseName === "snake") {
    await exposeWorkspaceNodeModules(ctx);
  }
  if (caseName === "password-zip") {
    await writePasswordZipCliRefactorSeed(ctx);
  }
  await writeWorkspaceSeed(ctx, caseName);
  return ctx;
}

async function exposeWorkspaceNodeModules(ctx) {
  const candidates = [
    path.join(repoRoot, "apps", "tui", "node_modules"),
    path.join(repoRoot, "apps", "gui", "node_modules"),
    path.join(repoRoot, "node_modules"),
  ];
  const source = candidates.find((candidate) =>
    fs.existsSync(path.join(candidate, "playwright")),
  );
  if (!source) {
    return;
  }
  const destination = path.join(ctx.workspace, "node_modules");
  await fsp.rm(destination, { recursive: true, force: true });
  try {
    await fsp.symlink(
      source,
      destination,
      process.platform === "win32" ? "junction" : "dir",
    );
  } catch {
    await fsp.cp(source, destination, { recursive: true });
  }
}

async function writeWorkspaceSeed(ctx, caseName) {
  await fsp.writeFile(
    path.join(ctx.workspace, "BUSINESS_TEST_CONTEXT.md"),
    [
      `# ${ctx.surface} ${caseName} release business test`,
      "",
      "This workspace is disposable and belongs to a Tura release-entry live script.",
      "Use the current workspace for all generated files.",
      "Do not ask follow-up questions.",
      "For Playwright verification, the workspace has node_modules linked to the repository test dependencies.",
      "",
    ].join("\n"),
  );
}

async function writePasswordZipCliRefactorSeed(ctx) {
  const salt = "tura-password-zip-20260611";
  const dictionaryPassword = "tura-zip-5519";
  const bruteForcePassword = "cab";
  const dictionaryTarget = createHash("sha256")
    .update(`${salt}:${dictionaryPassword}`, "utf8")
    .digest("hex");
  const bruteForceTarget = createHash("sha256")
    .update(`${salt}:${bruteForcePassword}`, "utf8")
    .digest("hex");
  const legacyDir = path.join(ctx.workspace, "legacy_zip_password_cli");
  const fixtureDir = path.join(ctx.workspace, "fixtures");
  const acceptanceDir = path.join(ctx.workspace, "acceptance");
  await fsp.mkdir(legacyDir, { recursive: true });
  await fsp.mkdir(fixtureDir, { recursive: true });
  await fsp.mkdir(acceptanceDir, { recursive: true });
  await fsp.writeFile(
    path.join(legacyDir, "README.md"),
    [
      "# Legacy zip-password-finder CLI",
      "",
      "This is the source CLI to refactor. It models ZIP password verification with",
      "a deterministic SHA-256 fixture so the business test stays cross-platform and",
      "does not depend on 7z, unzip, or OS archive encryption tools.",
      "",
      "Refactor it into `zip_password_refactor/bin/zip-password-finder.mjs`.",
      "The new CLI must keep the behavior but have clean argument validation,",
      "dictionary search, brute-force search, JSON output, and tests.",
      "",
    ].join("\n"),
  );
  await fsp.writeFile(
    path.join(legacyDir, "legacy_zip_password_finder.mjs"),
    [
      "#!/usr/bin/env node",
      "import crypto from 'node:crypto'",
      "import fs from 'node:fs'",
      "",
      "const args = process.argv.slice(2)",
      "let input = ''",
      "let wordlist = ''",
      "for (let i = 0; i < args.length; i += 1) {",
      "  if (args[i] === '-i' || args[i] === '--input') input = args[++i] || ''",
      "  else if (args[i] === '-w' || args[i] === '--wordlist') wordlist = args[++i] || ''",
      "}",
      "if (!input || !wordlist) {",
      "  console.error('usage: legacy_zip_password_finder --input fixture.json --wordlist candidates.txt')",
      "  process.exit(2)",
      "}",
      "const fixture = JSON.parse(fs.readFileSync(input, 'utf8'))",
      "const candidates = fs.readFileSync(wordlist, 'utf8').split(/\\r?\\n/u).map((line) => line.trim()).filter(Boolean)",
      "for (const candidate of candidates) {",
      "  const digest = crypto.createHash('sha256').update(`${fixture.salt}:${candidate}`, 'utf8').digest('hex')",
      "  if (digest === fixture.target) {",
      "    console.log(`Password found: ${candidate}`)",
      "    process.exit(0)",
      "  }",
      "}",
      "console.error('Password not found')",
      "process.exit(1)",
      "",
    ].join("\n"),
  );
  await fsp.writeFile(
    path.join(fixtureDir, "secret.zip.fixture.json"),
    JSON.stringify(
      {
        kind: "tura.sha256.zip-password-fixture.v1",
        algorithm: "SHA-256",
        salt,
        target: dictionaryTarget,
        expected_hint: "dictionary candidate in fixtures/candidates.txt",
      },
      null,
      2,
    ),
  );
  await fsp.writeFile(
    path.join(fixtureDir, "bruteforce.zip.fixture.json"),
    JSON.stringify(
      {
        kind: "tura.sha256.zip-password-fixture.v1",
        algorithm: "SHA-256",
        salt,
        target: bruteForceTarget,
        expected_hint: "brute force with charset abc and max length 3",
      },
      null,
      2,
    ),
  );
  await fsp.writeFile(
    path.join(fixtureDir, "candidates.txt"),
    [
      "winter-2024",
      "password",
      "letmein",
      "tura-zip-5519",
      "archive-open",
      "zip-secret",
    ].join("\n"),
  );
  await fsp.writeFile(
    path.join(acceptanceDir, "zip_password_cli_acceptance.mjs"),
    [
      "#!/usr/bin/env node",
      "import assert from 'node:assert/strict'",
      "import fs from 'node:fs'",
      "import path from 'node:path'",
      "import { spawnSync } from 'node:child_process'",
      "",
      "const root = process.cwd()",
      "const cli = path.join(root, 'zip_password_refactor', 'bin', 'zip-password-finder.mjs')",
      "const legacyCli = path.join(root, 'legacy_zip_password_cli', 'legacy_zip_password_finder.mjs')",
      "const reportPath = path.join(root, 'zip_password_refactor', 'acceptance-report.json')",
      "const cases = []",
      "",
      "function runCommand(name, command, args, expectedStatus = 0) {",
      "  const result = spawnSync(process.execPath, [command, ...args], { cwd: root, encoding: 'utf8', windowsHide: true, timeout: 30_000 })",
      "  const passed = result.status === expectedStatus",
      "  cases.push({ name, expectedStatus, status: result.status, signal: result.signal, passed, stdout: result.stdout, stderr: result.stderr })",
      "  assert.equal(result.status, expectedStatus, `${name} exited with ${result.status}: ${result.stderr || result.stdout}`)",
      "  return result",
      "}",
      "",
      "function run(name, args, expectedStatus = 0) {",
      "  return runCommand(name, cli, args, expectedStatus)",
      "}",
      "",
      "function runLegacy(name, args, expectedStatus = 0) {",
      "  return runCommand(name, legacyCli, args, expectedStatus)",
      "}",
      "",
      "function passwordFromOutput(text) {",
      "  const match = String(text || '').match(/Password found:\\s*([^\\r\\n]+)/i)",
      "  return match ? match[1].trim() : ''",
      "}",
      "",
      "assert.ok(fs.existsSync(cli), `missing CLI ${cli}`)",
      "assert.ok(fs.existsSync(legacyCli), `missing legacy CLI ${legacyCli}`)",
      "assert.ok(fs.statSync(cli).size > 700, 'CLI implementation is too small to be a real refactor')",
      "",
      "const help = run('help', ['--help'])",
      "assert.match(help.stdout, /zip-password-finder/i)",
      "assert.match(help.stdout, /--input/)",
      "assert.match(help.stdout, /--wordlist/)",
      "assert.match(help.stdout, /--charset/)",
      "assert.match(help.stdout, /--max-len/)",
      "assert.match(help.stdout, /--json/)",
      "",
      "const legacyDictionary = runLegacy('legacy dictionary oracle', [",
      "  '--input', 'fixtures/secret.zip.fixture.json',",
      "  '--wordlist', 'fixtures/candidates.txt',",
      "])",
      "const oraclePassword = passwordFromOutput(legacyDictionary.stdout)",
      "assert.equal(oraclePassword, 'tura-zip-5519')",
      "",
      "const dictionary = run('dictionary search', [",
      "  '--input', 'fixtures/secret.zip.fixture.json',",
      "  '--wordlist', 'fixtures/candidates.txt',",
      "])",
      "assert.equal(passwordFromOutput(dictionary.stdout), oraclePassword)",
      "",
      "const json = run('json dictionary search', [",
      "  '--input', 'fixtures/secret.zip.fixture.json',",
      "  '--wordlist', 'fixtures/candidates.txt',",
      "  '--json',",
      "])",
      "const parsed = JSON.parse(json.stdout)",
      "assert.equal(parsed.password, oraclePassword)",
      "assert.equal(parsed.found, true)",
      "",
      "const brute = run('brute force search', [",
      "  '--input', 'fixtures/bruteforce.zip.fixture.json',",
      "  '--charset', 'abc',",
      "  '--max-len', '3',",
      "])",
      "assert.match(brute.stdout, /cab/)",
      "",
      "const missing = run('missing input validation', ['--wordlist', 'fixtures/candidates.txt'], 2)",
      "assert.match(`${missing.stderr}\\n${missing.stdout}`, /input/i)",
      "",
      "const passed = cases.filter((item) => item.passed).length",
      "const total = cases.length",
      "const score = total ? passed / total : 0",
      "const report = { ok: true, score, passed, total, oracle: { dictionary_password: oraclePassword }, cases }",
      "fs.mkdirSync(path.dirname(reportPath), { recursive: true })",
      "fs.writeFileSync(reportPath, JSON.stringify(report, null, 2))",
      "console.log(`ZIP_PASSWORD_REFACTOR_ACCEPTANCE_OK score=${passed}/${total}`)",
      "",
    ].join("\n"),
  );
}

export function caseDefinition(caseName, ctx) {
  const sentinel = `TURA_${ctx.surface.toUpperCase()}_${caseName.replaceAll("-", "_").toUpperCase()}_${ctx.runId}`;
  if (caseName === "single-request") {
    return {
      sentinel,
      prompt: [
        "Single live release-entry request.",
        "Use command_run to create single_request_result.txt in the current workspace.",
        `The file must contain exactly this marker: ${sentinel}`,
        "Run a shell command to verify the file exists and contains the marker.",
        `Final answer must include this marker: ${sentinel}`,
      ].join("\n"),
      validate: async () => {
        const file = path.join(ctx.workspace, "single_request_result.txt");
        const text = await readText(file);
        return [
          check("single_request_result.txt exists", fs.existsSync(file)),
          check("single request marker written", text.includes(sentinel)),
        ];
      },
    };
  }
  if (caseName === "snake") {
    return {
      sentinel,
      prompt: [
        "Create and verify a minimal browser Snake game in this empty workspace.",
        "Requirements:",
        "- create snake.html with a playable canvas snake game",
        "- create tools/snake_playwright.mjs that opens snake.html with Playwright",
        "- in tools/snake_playwright.mjs resolve snake.html as path.resolve(process.cwd(), 'snake.html'), not relative to the tools directory",
        "- the verifier must press ArrowRight and ArrowDown",
        "- the verifier must check movement, score UI, restart, and no horizontal overflow",
        "- run the verifier with node",
        "- the verifier must save desktop.png and mobile.png with path.resolve(process.cwd(), 'desktop.png') and path.resolve(process.cwd(), 'mobile.png')",
        "- if the verifier fails, fix the game or verifier and rerun it until it passes",
        "- do not include the final marker until snake.html, tools/snake_playwright.mjs, desktop.png, and mobile.png all exist and the verifier exits 0",
        `Final answer must include exactly this marker: ${sentinel}`,
        "Final answer must mention snake.html, tools/snake_playwright.mjs, desktop.png, mobile.png, ArrowRight, ArrowDown, score, restart, and no horizontal overflow.",
      ].join("\n"),
      validate: async () => {
        const files = [
          ["snake.html", 500],
          [path.join("tools", "snake_playwright.mjs"), 500],
          ["desktop.png", 1_000],
          ["mobile.png", 1_000],
        ];
        return files.map(([relative, minSize]) => {
          const file = path.join(ctx.workspace, relative);
          return check(
            `${relative} exists and is non-empty`,
            fileSize(file) >= minSize,
            {
              size: fileSize(file),
            },
          );
        });
      },
    };
  }
  return {
    sentinel,
    prompt: [
      "Long CLI refactor task: rebuild the provided legacy zip-password-finder CLI.",
      "You are given legacy source under legacy_zip_password_cli/ and deterministic ZIP-password fixtures under fixtures/.",
      "Do not rely on 7z, zip, unzip, OS-specific archive encryption tools, or network downloads.",
      "Requirements:",
      "- create a refactored CLI at zip_password_refactor/bin/zip-password-finder.mjs",
      "- implement generic SHA-256 fixture verification using salt + candidate password",
      "- support dictionary mode with --input and --wordlist",
      "- support brute-force mode with --input, --charset, and --max-len",
      "- support --json output, --help output, and clear argument validation",
      "- add useful project files under zip_password_refactor/, including README.md and at least one local test file",
      "- run node acceptance/zip_password_cli_acceptance.mjs from the workspace root; this acceptance must use the legacy CLI as the dictionary oracle and compare the refactored CLI against it",
      "- if acceptance fails, fix the refactor and rerun it until it passes",
      "- do not include the final marker until acceptance prints ZIP_PASSWORD_REFACTOR_ACCEPTANCE_OK",
      "- after acceptance passes, write zip_password_refactor/RELEASE_RESULT.md containing the final marker and a concise acceptance summary",
      "- after acceptance passes, your final visible assistant reply must include the final marker",
      "- if you call task_status with status done, its task_detail must be only a few words describing the current task; it is not the final reply",
      `Final answer must include exactly this marker: ${sentinel}`,
      "Final answer must mention zip_password_refactor/bin/zip-password-finder.mjs, acceptance/zip_password_cli_acceptance.mjs, dictionary mode, brute-force mode, --json, SHA-256, tura-zip-5519, and the acceptance harness score.",
    ].join("\n"),
    validate: async () => {
      const cli = path.join(
        ctx.workspace,
        "zip_password_refactor",
        "bin",
        "zip-password-finder.mjs",
      );
      const readme = path.join(
        ctx.workspace,
        "zip_password_refactor",
        "README.md",
      );
      const report = path.join(
        ctx.workspace,
        "zip_password_refactor",
        "acceptance-report.json",
      );
      const releaseResult = path.join(
        ctx.workspace,
        "zip_password_refactor",
        "RELEASE_RESULT.md",
      );
      const acceptance = path.join(
        ctx.workspace,
        "acceptance",
        "zip_password_cli_acceptance.mjs",
      );
      const acceptanceResult = fs.existsSync(acceptance)
        ? spawnSync(process.execPath, [acceptance], {
            cwd: ctx.workspace,
            encoding: "utf8",
            timeout: 45_000,
            windowsHide: true,
          })
        : {
            status: 1,
            signal: "missing-acceptance",
            stdout: "",
            stderr: "missing acceptance script",
          };
      const reportValue = await readJson(report);
      return [
        check("refactored CLI exists", fileSize(cli) > 700, {
          size: fileSize(cli),
        }),
        check("refactor README exists", fileSize(readme) > 100, {
          size: fileSize(readme),
        }),
        check("acceptance script exists", fileSize(acceptance) > 1_000, {
          size: fileSize(acceptance),
        }),
        check("acceptance rerun exited 0", acceptanceResult.status === 0, {
          status: acceptanceResult.status,
          signal: acceptanceResult.signal,
          stdout: trimForSummary(acceptanceResult.stdout),
          stderr: trimForSummary(acceptanceResult.stderr),
        }),
        check("acceptance report ok", reportValue?.ok === true, { report }),
        check(
          "acceptance harness score complete",
          reportValue?.score === 1 &&
            reportValue?.passed === reportValue?.total &&
            Number(reportValue?.total) >= 6,
          {
            score: reportValue?.score,
            passed: reportValue?.passed,
            total: reportValue?.total,
          },
        ),
        check(
          "release result marker artifact exists",
          fileSize(releaseResult) > 20,
          {
            size: fileSize(releaseResult),
          },
        ),
        check(
          "legacy oracle comparison recorded",
          reportValue?.oracle?.dictionary_password === "tura-zip-5519",
          {
            oracle: reportValue?.oracle,
          },
        ),
      ];
    },
  };
}

export async function finishCase(ctx, definition, result, extras = {}) {
  const lastMessage = await readText(ctx.lastMessagePath);
  const markerArtifactText =
    ctx.caseName === "password-zip"
      ? await readText(
          path.join(
            ctx.workspace,
            "zip_password_refactor",
            "RELEASE_RESULT.md",
          ),
        )
      : "";
  const finalAnswerText = [
    result.stdout,
    lastMessage,
    extras.resultText || "",
    markerArtifactText,
  ].join("\n");
  const validation = await definition.validate();
  if (Array.isArray(extras.validation)) validation.push(...extras.validation);
  validation.push(
    check("process exited 0", result.status === 0, {
      status: result.status,
      signal: result.signal,
    }),
  );
  validation.push(
    check(
      "final marker observed",
      finalAnswerText.includes(definition.sentinel),
    ),
  );
  if (extras.cleanup) {
    validation.push(
      check("backend daemons cleaned up", extras.cleanup.ok, extras.cleanup),
    );
  }
  const summary = {
    schema: "tura.business.release-entry.v1",
    ok: validation.every((item) => item.ok),
    surface: extras.surface || ctx.surface,
    case_name: ctx.caseName,
    run_id: ctx.runId,
    run_root: ctx.runRoot,
    workspace: ctx.workspace,
    summary_path: ctx.summaryPath,
    binary_profile: binaryProfile,
    binary_dir: binaryDir(),
    model: defaultModel,
    agent: defaultAgent,
    model_variant: defaultVariant,
    timeout_ms: caseTimeoutMs(ctx.caseName),
    sentinel: definition.sentinel,
    command: result.command,
    args: result.args,
    status: result.status,
    signal: result.signal,
    duration_ms: result.durationMs,
    stdout_path: result.stdoutPath,
    stderr_path: result.stderrPath,
    last_message_path: ctx.lastMessagePath,
    validation,
    ...extras,
  };
  await fsp.writeFile(ctx.summaryPath, JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  if (!summary.ok) process.exitCode = 1;
  return summary;
}

export async function runLoggedProcess(command, args, ctx, options = {}) {
  const stdoutPath = path.join(
    ctx.logs,
    `${path.basename(command).replace(/[^\w.-]/g, "_")}.stdout.log`,
  );
  const stderrPath = path.join(
    ctx.logs,
    `${path.basename(command).replace(/[^\w.-]/g, "_")}.stderr.log`,
  );
  const env = { ...caseEnv(ctx), ...(options.env || {}) };
  const started = Date.now();
  const child = spawn(command, args, {
    cwd: repoRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    shell: false,
  });
  let stdout = "";
  let stderr = "";
  child.stdout?.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr?.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const timeoutMs = options.timeoutMs || caseTimeoutMs(ctx.caseName);
  const result = await new Promise((resolve) => {
    const timer = setTimeout(async () => {
      await stopProcess(child);
      resolve({ status: null, signal: "timeout" });
    }, timeoutMs);
    child.on("error", (error) => {
      clearTimeout(timer);
      stderr += `\n${error.stack || error.message || error}`;
      resolve({ status: 1, signal: "error" });
    });
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      resolve({ status, signal });
    });
  });
  child.stdout?.destroy();
  child.stderr?.destroy();
  await fsp.writeFile(stdoutPath, stdout);
  await fsp.writeFile(stderrPath, stderr);
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    durationMs: Date.now() - started,
    stdout,
    stderr,
    stdoutPath,
    stderrPath,
  };
}

export function caseEnv(ctx) {
  return {
    ...process.env,
    PATH: `${binaryDir()}${path.delimiter}${process.env.PATH || ""}`,
    TURA_HOME: ctx.turaHome,
    TURA_PROJECT_ROOT: repoRoot,
    TURA_PROVIDER_CONFIG:
      process.env.TURA_PROVIDER_CONFIG ||
      path.join(
        repoRoot,
        "crates",
        "provider",
        "config",
        "provider_config.json",
      ),
    LOG_PATH: ctx.providerLogRoot,
    TURA_CWD: ctx.workspace,
    TURA_DEBUG_RUNTIME: process.env.TURA_DEBUG_RUNTIME || "1",
    FORCE_COLOR: process.env.FORCE_COLOR || "0",
  };
}

export async function startReleaseGateway(ctx) {
  const port = await freePort();
  const child = spawn(releaseBinary("tura_gateway"), [], {
    cwd: ctx.workspace,
    env: {
      ...caseEnv(ctx),
      PORT: String(port),
      TURA_GATEWAY_PORT: String(port),
      TURA_GATEWAY_URL: `http://127.0.0.1:${port}`,
      TURA_GUI_DIST: path.join(binaryDir(), "gui"),
      TURA_ROUTER_STDERR_LOG: path.join(ctx.logs, "router.stderr.log"),
      TURA_RUNTIME_WORKER_STDERR_LOG: path.join(
        ctx.logs,
        "runtime-worker.stderr.log",
      ),
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout?.pipe(
    fs.createWriteStream(path.join(ctx.logs, "gateway.stdout.log")),
  );
  child.stderr?.pipe(
    fs.createWriteStream(path.join(ctx.logs, "gateway.stderr.log")),
  );
  const url = `http://127.0.0.1:${port}`;
  await waitForUrl(`${url}/global/health`, child, 60_000);
  return { child, url };
}

export async function waitForUrl(url, child, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    if (child?.exitCode !== null) {
      throw new Error(`${url} exited before readiness with ${child.exitCode}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) return response;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw lastError || new Error(`Timed out waiting for ${url}`);
}

export async function stopProcess(child) {
  if (!child || child.killed || child.exitCode !== null) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      windowsHide: true,
    });
    return;
  }
  try {
    child.kill("SIGTERM");
  } catch {}
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    delay(2_000),
  ]);
  if (child.exitCode === null) {
    try {
      child.kill("SIGKILL");
    } catch {}
  }
}

export async function shutdownBackendDaemons(ctx) {
  const routerAddrPath = path.join(
    ctx.turaHome,
    "db",
    "session_log",
    "router.addr",
  );
  const serviceAddrPath = path.join(
    ctx.turaHome,
    "db",
    "session_log",
    "service.addr",
  );
  const result = {
    router_addr_path: routerAddrPath,
    service_addr_path: serviceAddrPath,
    requested: false,
    ok: true,
  };
  const endpoint = await readJson(routerAddrPath);
  const addr = endpoint?.addr;
  if (!addr) {
    result.router_addr_missing = true;
    result.service_addr_removed = !fs.existsSync(serviceAddrPath);
    return result;
  }

  result.requested = true;
  try {
    result.response = await callRouter(addr, {
      request_id: `business-shutdown-${Date.now()}`,
      kind: "call",
      method: "execution.shutdown",
      payload: {},
    });
    await waitForFileMissing(routerAddrPath, 8_000);
    await waitForFileMissing(serviceAddrPath, 8_000);
    result.router_addr_removed = !fs.existsSync(routerAddrPath);
    result.service_addr_removed = !fs.existsSync(serviceAddrPath);
    result.ok = result.router_addr_removed && result.service_addr_removed;
  } catch (error) {
    result.ok = false;
    result.error = error.stack || error.message || String(error);
  }
  return result;
}

async function callRouter(addr, request) {
  const [host, portText] = addr.split(":");
  const port = Number(portText);
  const timeoutMs = 15_000;
  if (!host || !Number.isFinite(port)) {
    throw new Error(`invalid router address ${addr}`);
  }
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host, port });
    let buffer = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(
        new Error(`router shutdown timed out after ${timeoutMs}ms at ${addr}`),
      );
    }, timeoutMs);
    socket.on("connect", () => {
      socket.write(`${JSON.stringify(request)}\n`);
    });
    socket.on("data", (chunk) => {
      buffer += chunk.toString();
      const newline = buffer.indexOf("\n");
      if (newline >= 0) {
        clearTimeout(timer);
        socket.end();
        try {
          resolve(JSON.parse(buffer.slice(0, newline)));
        } catch (error) {
          reject(error);
        }
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    socket.on("close", () => {
      clearTimeout(timer);
      if (!buffer.trim()) {
        reject(new Error(`router closed without response at ${addr}`));
      }
    });
  });
}

async function waitForFileMissing(file, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!fs.existsSync(file)) return;
    await delay(100);
  }
}

export async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close(() => resolve(port));
    });
  });
}

export function pythonCommand() {
  return (
    process.env.PYTHON || (process.platform === "win32" ? "python" : "python3")
  );
}

export function timestamp() {
  const now = new Date();
  return now
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\..+$/u, "")
    .replace("T", "-");
}

export function caseTimeoutMs(caseName) {
  const specificName = `TURA_BUSINESS_${caseName.replaceAll("-", "_").toUpperCase()}_TIMEOUT_MS`;
  const specific = positiveNumberFromEnv(specificName);
  if (specific) return specific;
  const global = positiveNumberFromEnv("TURA_BUSINESS_TIMEOUT_MS");
  if (global) return global;
  return defaultTimeoutMsByCase[caseName] || 180_000;
}

function positiveNumberFromEnv(name) {
  const value = Number(process.env[name] || 0);
  return Number.isFinite(value) && value > 0 ? value : 0;
}

function fileSize(file) {
  try {
    return fs.statSync(file).size;
  } catch {
    return 0;
  }
}

export async function readText(file) {
  try {
    return await fsp.readFile(file, "utf8");
  } catch {
    return "";
  }
}

export async function readJson(file) {
  const text = await readText(file);
  if (!text.trim()) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

export function check(name, ok, details = {}) {
  return { name, ok: Boolean(ok), ...details };
}

export function trimForSummary(text) {
  const value = String(text || "");
  return value.length > 2_000 ? `${value.slice(0, 2_000)}...` : value;
}

export function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
