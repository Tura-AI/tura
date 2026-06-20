#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";
import {
  assertTerminalFits,
  assertTerminalVisualContract,
  forbiddenGatewayPaths,
  startGateway,
  terminalViewportText,
  waitForCondition,
  waitForUrl,
} from "./helpers/gateway_cli_fixture.mjs";

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..");
const nodeBin = process.execPath;
const tuiBin = path.join(repoRoot, "apps", "tui", "dist", "index.js");
const webTerminalBin = path.join(repoRoot, "apps", "tui", "scripts", "web-terminal.mjs");
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "tui-minimal-e2e",
  String(Date.now()),
);
const tuiRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"));
function runCli(args, options = {}) {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now();
    const child = spawn(nodeBin, [tuiBin, ...args], {
      cwd: repoRoot,
      env: { ...process.env, TURA_LANG: "en", ...(options.env ?? {}) },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("error", reject);
    child.on("close", (status) =>
      resolve({ status, stdout, stderr, durationMs: Date.now() - startedAt }),
    );
  });
}

function baseArgs(gateway) {
  return ["--gateway-url", gateway.url, "--cwd", runRoot];
}

async function expectCliOk(args) {
  const result = await runCli(args);
  assert.equal(
    result.status,
    0,
    `expected status=0 for ${args.join(" ")}\nstdout=${result.stdout}\nstderr=${result.stderr}`,
  );
  return result;
}

async function expectCliJson(args) {
  const result = await expectCliOk(args);
  return JSON.parse(result.stdout);
}

async function runWebTerminalE2e(gateway) {
  const { chromium } = tuiRequire("playwright");
  const webPort = 18_000 + Math.floor(Math.random() * 1_000);
  const screenshotsDir = path.join(runRoot, "web-terminal-screenshots");
  await fs.mkdir(screenshotsDir, { recursive: true });
  const draggedImage = path.join(runRoot, "dragged-image.png");
  await fs.writeFile(draggedImage, Buffer.from("89504e470d0a1a0a", "hex"));
  const child = spawn(nodeBin, [webTerminalBin], {
    cwd: repoRoot,
    env: {
      ...process.env,
      PORT: String(webPort),
      TURA_GATEWAY_URL: gateway.url,
      TURA_CWD: runRoot,
      TURA_LANG: "en",
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let logs = "";
  child.stdout.on("data", (chunk) => {
    logs += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    logs += chunk.toString();
  });
  try {
    await waitForUrl(`http://127.0.0.1:${webPort}/`);
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    try {
      for (const profile of ["plain", "ansi", "rich"]) {
        await page.setViewportSize({ width: 1280, height: 720 });
        await page.goto(`http://127.0.0.1:${webPort}/${profile}?instance=${profile}-desktop`, {
          waitUntil: "domcontentloaded",
        });
        await page.waitForFunction(
          () =>
            /Rich Fixture|Rich fixture ph\s*ase 1|Rich fixture phase 1/.test(
              document.body.innerText,
            ),
          null,
          { timeout: 10_000 },
        );
        await page.waitForFunction(
          () => /context\s+90k\/200k\s+██▓░░░/u.test(document.body.innerText),
          null,
          { timeout: 10_000 },
        );
        await page.screenshot({
          path: path.join(screenshotsDir, `${profile}.png`),
          fullPage: false,
        });
        await assertTerminalFits(page, `${profile} desktop`);
        await assertTerminalVisualContract(page, profile);
        assert.equal(
          await page.title(),
          `Tura TUI ${profile === "plain" ? "Plain / Safe" : profile === "ansi" ? "ANSI / Default" : "Rich / Modern"}`,
        );
        const body = await page.locator("body").innerText();
        assert.match(body, /OC \| Tura TUI/);
        assert.doesNotMatch(body, /^workspace$/im);
        assert.match(body, /context\s+90k\/200k\s+██▓░░░/u);
        assert.doesNotMatch(body, /tokens\s+\d+|tokens\s+-/u);
        assert.match(
          body,
          /Rich fixture phase 2|Local path C:\/repo\/apps\/tui|Directory\s+C:\/repo\/apps\/tui/,
        );
        assert.match(
          body,
          /Paragraph before intentional blank line\.[\s\S]*Paragraph after intentional blank line\./,
        );
        assert.match(body, /Cited text or summary/);
        assert.doesNotMatch(body, /│ Cited text or summary/);
        assert.match(
          body,
          /(?:Status\s+Table rendering stays compact and readable|Item: Status\s+Target: Table rendering stays compact and readable)/,
        );
        assert.match(body, /pnpm test --filter @tura\/tui -- --rich-fixture/);
        assert.match(body, /Protocol fixture complete|Search Link|README/);
        assert.doesNotMatch(body, /commands?:[\s\u00a0]*\d+/i);
        assert.doesNotMatch(body, /\[EMOJI:/);
        if (profile === "plain") {
          assert.doesNotMatch(body, /[│▏─┌┐└┘├┤┬┴┼]/u);
          assert.doesNotMatch(body, /^-{8,}$/m);
        }
        if (profile === "rich") {
          assert.match(body, /Rich Fixture/);
          assert.match(body, /Enter to send,.*\/help commands \/settings settings/);
          assert.doesNotMatch(body, /\[MEDIA:/);
          assert.doesNotMatch(body, /Agent:fast/);
          assert.doesNotMatch(body, /persona:direct/);
          const chromeColors = await page.evaluate(() =>
            [...document.querySelectorAll(".dot")].map(
              (node) => getComputedStyle(node).backgroundColor,
            ),
          );
          assert.deepEqual(chromeColors, [
            "rgb(92, 92, 92)",
            "rgb(64, 224, 208)",
            "rgb(92, 92, 92)",
          ]);
        }

        await page.setViewportSize({ width: 820, height: 680 });
        await page.goto(`http://127.0.0.1:${webPort}/${profile}?instance=${profile}-medium`, {
          waitUntil: "domcontentloaded",
        });
        await page.waitForFunction(
          () =>
            /Rich Fixture|Rich fixture ph\s*ase 1|Rich fixture phase 1/.test(
              document.body.innerText,
            ),
          null,
          { timeout: 10_000 },
        );
        await page.screenshot({
          path: path.join(screenshotsDir, `${profile}-medium.png`),
          fullPage: false,
        });
        await assertTerminalFits(page, `${profile} medium`);

        await page.setViewportSize({ width: 390, height: 640 });
        await page.goto(`http://127.0.0.1:${webPort}/${profile}?instance=${profile}-mobile`, {
          waitUntil: "domcontentloaded",
        });
        await page.waitForFunction(() => /Rich Fixture/.test(document.body.innerText), null, {
          timeout: 10_000,
        });
        await page.screenshot({
          path: path.join(screenshotsDir, `${profile}-mobile.png`),
          fullPage: false,
        });
        await assertTerminalFits(page, `${profile} mobile`);
        const mobileViewport = await terminalViewportText(page);
        assert.match(
          mobileViewport,
          /Rich Fixture/,
          `${profile} mobile viewport should keep the session title visible`,
        );
        assert.doesNotMatch(mobileViewport, /(?:\\x1b|\\u001b|8;2;128;128;128m)/);
      }
      await page.setViewportSize({ width: 1280, height: 720 });
      const richCommandInstance = "rich-command";
      const richCommandUrl = `http://127.0.0.1:${webPort}/rich?instance=${richCommandInstance}`;
      const sendRichCommandInput = (data) =>
        page.evaluate((input) => globalThis.__turaSendInput?.(input), data);
      await page.goto(richCommandUrl, { waitUntil: "domcontentloaded" });
      await page.waitForFunction(() => /Rich Fixture/.test(document.body.innerText), null, {
        timeout: 10_000,
      });
      await page.evaluate(() => globalThis.__turaFit?.());
      await sendRichCommandInput("/help\r");
      await page.waitForFunction(
        () =>
          /[─-]{3}\s*Help\s*[─-]{9}/i.test(document.body.innerText) &&
          /(^|\n).*\/chat(?:\s|$)/m.test(document.body.innerText),
        null,
        { timeout: 20_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-help.png"),
        fullPage: false,
      });
      await assertTerminalFits(page, "rich help");
      {
        const body = await page.locator("body").innerText();
        assert.match(body, /[─-]{3}\s*Help\s*[─-]{9}/i);
        assert.doesNotMatch(body, /^\s*[─-]{8,}\s*$/m);
        assert.match(body, /(^|\n).*\/chat(?:\s|$)/m);
        assert.match(body, /(^|\n).*\/commands(?:\s|$)/m);
        assert.doesNotMatch(body, /system|assistant|user/);
        assert.doesNotMatch(body, /Agent:fast/);
      }
      await sendRichCommandInput("/chat\r");
      await page.waitForFunction(
        () => !/[─-]{3}\s*Help\s*[─-]{9}/i.test(document.body.innerText),
        null,
        { timeout: 10_000 },
      );
      await page.waitForFunction(
        () => document.body.innerText.includes("Protocol fixture complete"),
        null,
        { timeout: 10_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-commands-expanded.png"),
        fullPage: false,
      });
      {
        const body = await page.locator("body").innerText();
        assert.match(body, /pnpm test --filter @tura\/tui -- --rich-fixture/);
        assert.match(body, /\[shell: collecting rich terminal screenshots\]/);
        assert.doesNotMatch(body, /◆\s+◇\s+Commands/);
      }
      await sendRichCommandInput("/models\r");
      await page.waitForTimeout(1200);
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-models.png"),
        fullPage: false,
      });
      await sendRichCommandInput("/chat\r");
      await page.waitForFunction(
        () => document.body.innerText.includes("Protocol fixture complete"),
        null,
        { timeout: 10_000 },
      );
      await page.waitForTimeout(300);
      await sendRichCommandInput("\u0015/settings\r");
      await page.waitForFunction(
        () => /[─-]{3}\s*Session Settings\s*[─-]{9}/.test(document.body.innerText),
        null,
        { timeout: 10_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-settings.png"),
        fullPage: false,
      });
      {
        const body = await page.locator("body").innerText();
        assert.match(body, /[─-]{3}\s*Session Settings\s*[─-]{9}/);
        assert.doesNotMatch(body, /^\s*[─-]{8,}\s*$/m);
        assert.doesNotMatch(body, /\/config get|\/config set|\/model provider\/model/);
      }
      await sendRichCommandInput("\u001b");
      await page.waitForFunction(
        () => document.body.innerText.includes("Protocol fixture complete"),
        null,
        { timeout: 10_000 },
      );
      await sendRichCommandInput("/auth\r");
      await page.waitForTimeout(1200);
      await page.screenshot({ path: path.join(screenshotsDir, "rich-auth.png"), fullPage: false });
      await sendRichCommandInput("\u001b");
      await page.waitForFunction(
        () => document.body.innerText.includes("Protocol fixture complete"),
        null,
        { timeout: 10_000 },
      );
      await sendRichCommandInput("/sessions\r");
      await page.waitForFunction(
        () =>
          [...document.querySelectorAll(".xterm-rows > div")]
            .map((node) => node.textContent ?? "")
            .join("\n")
            .includes("Sessions"),
        null,
        { timeout: 10_000 },
      );
      await page.waitForTimeout(300);
      await page.waitForFunction(
        () =>
          [...document.querySelectorAll(".xterm-rows > div")]
            .map((node) => node.textContent ?? "")
            .join("\n")
            .includes("Sessions"),
        null,
        { timeout: 10_000 },
      );
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-sessions.png"),
        fullPage: false,
      });
      {
        const body = await terminalViewportText(page);
        assert.match(body, /Sessions/);
        assert.doesNotMatch(body, /^\s*[─-]{8,}\s*$/m);
      }
      await sendRichCommandInput("\u001b");
      await page.waitForFunction(
        () => document.body.innerText.includes("Protocol fixture complete"),
        null,
        { timeout: 10_000 },
      );
      await sendRichCommandInput("/personas\r");
      await page.waitForFunction(
        () => /Direct persona|Concise|direct/.test(document.body.innerText),
        null,
        { timeout: 10_000 },
      );
      const personaBody = await page.locator("body").innerText();
      assert.match(personaBody, /Direct persona|Concise|direct/);
      await page.screenshot({
        path: path.join(screenshotsDir, "rich-personas.png"),
        fullPage: false,
      });
      const configPatchCount = gateway.records.configPatches.length;
      const agentPatchCount = gateway.records.agentUpserts.length;
      await sendRichCommandInput("/persona direct\r");
      await page.waitForTimeout(1200);
      assert.equal(gateway.records.configPatches.length, configPatchCount);
      assert.equal(gateway.records.agentUpserts.length, agentPatchCount);
      await sendRichCommandInput("/chat\r");
      await page.waitForTimeout(150);
      const promptCountBeforeMedia = gateway.records.prompts.length;
      await sendRichCommandInput(`${draggedImage}\r`);
      await waitForCondition(
        () => gateway.records.prompts.length > promptCountBeforeMedia,
        "timed out waiting for dragged image prompt",
      );
      assert.match(
        gateway.records.prompts.at(-1)?.parts?.[0]?.text ?? "",
        /\[MEDIA:.*dragged-image\.png:MEDIA\]/,
      );
    } finally {
      await browser.close();
    }
    return screenshotsDir;
  } finally {
    child.kill();
    await new Promise((resolve) => child.once("exit", resolve));
    await fs.writeFile(path.join(runRoot, "web-terminal.log"), logs);
  }
}

async function main() {
  await fs.mkdir(runRoot, { recursive: true });
  await fs.access(tuiBin);
  const gateway = await startGateway(runRoot);
  try {
    const help = await expectCliOk([...baseArgs(gateway), "--lang", "en", "help"]);
    for (const command of ["project", "file", "persona", "command", "inspect", "gateway"]) {
      assert.match(help.stdout, new RegExp(`^  ${command}\\s`, "m"));
    }
    assert.match(help.stdout, /agent\s+list, read, create, update, or tier agents/);
    assert.match(help.stdout, /session\s+list or show sessions/);
    const zhHelp = await expectCliOk([...baseArgs(gateway), "--lang", "zh-CN", "help"]);
    assert.match(zhHelp.stdout, /命令:/);
    assert.match(zhHelp.stdout, /agent\s+列出、读取、创建、更新或配置智能体档位/);

    const config = await expectCliJson([...baseArgs(gateway), "--json", "config", "get"]);
    assert.equal(config.active_agent, "fast");
    const patched = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "config",
      "set",
      "agent=fast",
      "model_variant=low",
    ]);
    assert.equal(patched.active_agent, "fast");
    assert.equal(gateway.records.configPatches[0].model_variant, "low");
    const rejectedTheme = await runCli([...baseArgs(gateway), "config", "set", "theme=dark"]);
    assert.notEqual(rejectedTheme.status, 0);
    assert.match(rejectedTheme.stderr + rejectedTheme.stdout, /unsupported session config key/);
    const rejectedPlanning = await runCli([
      ...baseArgs(gateway),
      "--lang",
      "en",
      "config",
      "set",
      "planning=on",
    ]);
    assert.notEqual(rejectedPlanning.status, 0);
    assert.match(
      rejectedPlanning.stderr + rejectedPlanning.stdout,
      /unsupported session config key/,
    );
    const rejectedPlanningZh = await runCli([
      ...baseArgs(gateway),
      "--lang",
      "zh-CN",
      "config",
      "set",
      "planning=on",
    ]);
    assert.notEqual(rejectedPlanningZh.status, 0);
    assert.match(rejectedPlanningZh.stderr + rejectedPlanningZh.stdout, /不支持的会话配置键/);
    const tiers = await expectCliJson([...baseArgs(gateway), "--json", "config", "model-tiers"]);
    assert.equal(tiers.tiers[0].tier, "fast");
    const tierOptions = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "config",
      "model-tier",
      "fast",
    ]);
    assert.equal(tierOptions.tier, "fast");
    const tierUpdated = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "config",
      "model-tier",
      "fast",
      "codex/gpt-5.5",
    ]);
    assert.equal(tierUpdated.tiers[0].current.provider, "codex");
    assert.deepEqual(gateway.records.modelConfigPuts.at(-1), {
      tier: "fast",
      provider: "codex",
      model: "gpt-5.5",
    });

    const sessions = await expectCliJson([...baseArgs(gateway), "--json", "session", "list"]);
    assert.equal(sessions[0].id, "sess-e2e");
    const shown = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "session",
      "show",
      "sess-e2e",
    ]);
    assert.equal(shown.session.id, "sess-e2e");
    assert.match(
      shown.messages
        .map((message) => message.parts.map((part) => part.text ?? "").join("\n"))
        .join("\n"),
      /Rich fixture phase 1[\s\S]*Protocol fixture complete/,
    );
    const updatedSession = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "session",
      "update",
      "sess-e2e",
      "--data",
      '{"agent":"fast"}',
    ]);
    assert.equal(updatedSession.agent, "fast");
    const taskSession = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "session",
      "task-management",
      "sess-e2e",
      "--data",
      '{"status":"doing"}',
    ]);
    assert.equal(taskSession.task_management.status, "doing");

    const providers = await expectCliJson([...baseArgs(gateway), "--json", "provider", "list"]);
    assert.equal(providers.all[0].id, "openai");
    const providerStatus = await expectCliJson([
      ...baseArgs(gateway),
      "provider",
      "status",
      "openai",
    ]);
    assert.equal(providerStatus.authenticated, true);
    const providerLogin = await expectCliOk([
      ...baseArgs(gateway),
      "provider",
      "login",
      "openai",
      "--no-open",
    ]);
    assert.match(providerLogin.stdout, /OAuth started/);
    assert.match(providerLogin.stdout, /authenticated/);
    const logout = await expectCliJson([...baseArgs(gateway), "provider", "logout", "openai"]);
    assert.equal(logout.ok, true);
    const authSet = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "provider",
      "set-auth",
      "openai",
      "--key",
      "sk-test",
    ]);
    assert.equal(authSet.saved, true);
    assert.equal(gateway.records.providerAuthSets.at(-1).key, "sk-test");

    const agents = await expectCliJson([...baseArgs(gateway), "--json", "agent", "list"]);
    assert.equal(agents[0].summary.name, "Fast");
    const agent = await expectCliJson([...baseArgs(gateway), "--json", "agent", "show", "fast"]);
    assert.equal(agent.summary.id, "fast");
    const createdAgent = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "agent",
      "create",
      "dynamic-fast",
      "--config",
      '{"description":"Dynamic fast"}',
      "--prompt",
      "Prompt text",
    ]);
    assert.equal(createdAgent.summary.id, "dynamic-fast");
    const updatedAgent = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "agent",
      "update",
      "dynamic-fast",
      "--config",
      '{"description":"Updated"}',
    ]);
    assert.equal(updatedAgent.config.agent_name, "dynamic-fast");

    const personas = await expectCliJson([...baseArgs(gateway), "--json", "persona", "list"]);
    assert.equal(personas[0].summary.id, "direct");
    const createdPersona = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "persona",
      "create",
      "brief",
      "--persona",
      "Be brief.",
    ]);
    assert.equal(createdPersona.summary.id, "brief");
    const updatedPersona = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "persona",
      "update",
      "brief",
      "--communication-style",
      "Compact.",
    ]);
    assert.equal(updatedPersona.communication_style, "Compact.");

    const localProject = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "project",
      "select-local",
      "--title",
      "Pick workspace",
    ]);
    assert.equal(localProject.worktree, runRoot);

    const resume = await expectCliOk([...baseArgs(gateway), "resume", "sess-e2e"]);
    assert.match(resume.stdout, /Protocol fixture complete/);

    const run = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "run",
      "hello minimal tui",
      "--no-stream",
      "--timeout",
      "5",
    ]);
    assert.equal(run.status, "completed");
    assert.equal(run.finalText, "final: hello minimal tui");
    assert.equal("force_planning" in gateway.records.createSessions.at(-1), false);

    for (const shell of ["bash", "zsh", "fish"]) {
      const completion = await expectCliOk(["completion", shell]);
      assert.match(completion.stdout, /tura|complete|_arguments/);
      assert.match(completion.stdout, /gateway|persona|project/);
    }

    const gatewayClientModule = await import(
      pathToFileURL(path.join(repoRoot, "apps", "tui", "dist", "gateway", "client.js")).href
    );
    const client = new gatewayClientModule.GatewayClient({
      baseUrl: gateway.url,
      directory: runRoot,
      timeoutMs: 5000,
    });
    assert.equal((await client.health()).version, "minimal-e2e");
    await client.syncWorkspace();
    assert.equal((await client.getSessionConfig()).active_agent, "fast");
    assert.equal(
      (await client.patchSessionConfig({ model_variant: "medium" })).model_variant,
      "medium",
    );
    assert.equal(
      (await client.listSessions({ includeChildren: true, limit: 5 }))[0].id,
      "sess-created-1",
    );
    assert.equal((await client.getSession("sess-e2e")).id, "sess-e2e");
    assert.equal((await client.updateSession("sess-e2e", { agent: "fast" })).agent, "fast");
    assert.equal((await client.listMessages("sess-e2e")).at(-1).role, "assistant");
    assert.equal((await client.listProviders()).all[0].id, "openai");
    assert.equal((await client.listProviderAuthMethods()).openai[0].login, "oauth");
    assert.equal((await client.providerAuthStatus("openai")).authenticated, true);
    assert.equal(
      (await client.providerOauthAuthorize("openai", 0)).url,
      "https://auth.example.test/openai",
    );
    assert.equal((await client.providerLogout("openai")).ok, true);
    assert.equal((await client.listAgents())[0].summary.id, "fast");
    assert.equal((await client.getAgent("fast")).summary.id, "fast");
    await client.abort("sess-e2e");
    assert.ok(gateway.records.aborts.includes("sess-e2e"));

    gateway.seedRichFixture();
    const requestCountBeforeWebTerminal = gateway.records.requests.length;
    const screenshotsDir = await runWebTerminalE2e(gateway);
    const forbiddenRequests = gateway.records.requests
      .slice(requestCountBeforeWebTerminal)
      .filter(
        (request) =>
          forbiddenGatewayPaths.some(
            (pathName) => request.path === pathName || request.path.startsWith(`${pathName}/`),
          ) || /\/session\/[^/]+\/task-management/.test(request.path),
      );
    assert.deepEqual(forbiddenRequests, []);
    console.log(`[tui-minimal-e2e] ok=true screenshots=${screenshotsDir}`);
  } finally {
    await gateway.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
