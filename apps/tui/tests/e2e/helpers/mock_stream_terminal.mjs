#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";

export async function delay(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

export async function waitForUrl(url, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Retry.
    }
    await delay(100);
  }
  throw new Error(`timed out waiting for ${url}`);
}

export async function listen(server) {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return server.address().port;
}

export function startWebTerminal({ repoRoot, workspace, gatewayUrl, port }) {
  const child = spawn(
    process.execPath,
    [path.join(repoRoot, "apps", "tui", "scripts", "web-terminal.mjs")],
    {
      cwd: path.join(repoRoot, "apps", "tui"),
      env: {
        ...process.env,
        PORT: String(port),
        TURA_GATEWAY_URL: gatewayUrl,
        TURA_CWD: workspace,
        FORCE_COLOR: "1",
        TURA_LANG: "en",
      },
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    },
  );
  let logs = "";
  child.stdout.on("data", (chunk) => {
    logs += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    logs += chunk.toString();
  });
  return { child, logs: () => logs };
}

export async function terminalText(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  );
}

export async function terminalBufferText(page) {
  return page.evaluate(() => {
    const buffer = window.__turaTerminal?.buffer.active;
    if (!buffer) return "";
    const lines = [];
    for (let index = 0; index < buffer.length; index += 1) {
      lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
    }
    return lines.join("\n");
  });
}

export async function waitForTerminalBufferText(page, marker, timeoutMs = 5000) {
  await page.waitForFunction(
    (needle) => {
      const buffer = window.__turaTerminal?.buffer.active;
      if (!buffer) return false;
      const lines = [];
      for (let index = 0; index < buffer.length; index += 1) {
        lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
      }
      return lines.join("\n").includes(needle);
    },
    marker,
    { timeout: timeoutMs },
  );
}

export async function scrollTerminalTo(page, target, marker = undefined) {
  await page.evaluate((nextTarget) => {
    const term = window.__turaTerminal;
    if (!term) return;
    if (nextTarget === "top") {
      if (typeof term.scrollToLine === "function") term.scrollToLine(0);
      else term.scrollToTop();
      return;
    }
    if (typeof term.scrollToBottom === "function") term.scrollToBottom();
  }, target);
  if (!marker) {
    await delay(200);
    return;
  }
  await page.waitForFunction(
    (needle) => {
      const rows = [...document.querySelectorAll(".xterm-rows > div")]
        .map((node) => node.textContent ?? "")
        .join("\n");
      return rows.includes(needle);
    },
    marker,
    { timeout: 5_000 },
  );
}

export function markerCount(text, marker) {
  return text.split(marker).length - 1;
}

export function regexCount(text, pattern) {
  return Array.from(text.matchAll(pattern)).length;
}

export function assertNoDuplicatedFrameText(text, label, markers = []) {
  assert.ok(
    regexCount(text, /Enter to send/gu) <= 1,
    `${label} should not retain a duplicated composer/input box`,
  );
  assert.ok(
    markerCount(text, "Mock Stream") <= 1,
    `${label} should not retain duplicated session title chrome`,
  );
  assert.ok(
    regexCount(text, /tokens\s+\d+|tokens\s+-/gu) <= 1,
    `${label} should not retain duplicated token/status chrome`,
  );
  for (const marker of markers) {
    assert.equal(markerCount(text, marker), 1, `${label} should show ${marker} exactly once`);
  }
}

export async function waitForComposer(page, timeoutMs = 5000) {
  await page.waitForFunction(() => /Enter to send/.test(document.body.innerText), null, {
    timeout: timeoutMs,
  });
}

export async function submitTypedPrompt(page, text) {
  await page.evaluate((value) => window.__turaSendInput(value), text);
  await page.evaluate(() => window.__turaSendInput("\r"));
}

export async function seedTerminalScrollback(page, marker) {
  await page.evaluate((staleMarker) => {
    const term = window.__turaTerminal;
    if (!term) return;
    const rows = Number(term.rows) || 24;
    for (let index = 0; index < rows + 12; index += 1) {
      term.write(`\r\n${staleMarker}_${String(index).padStart(2, "0")}`);
    }
    term.scrollToTop?.();
  }, marker);
  await delay(200);
}

export async function waitForSessionPicker(page, timeoutMs = 5000) {
  await page.waitForFunction(() => /New session|New Session/.test(document.body.innerText), null, {
    timeout: timeoutMs,
  });
}

export function assertSessionPickerCleared(text, label, staleMarker) {
  assert.doesNotMatch(
    text,
    new RegExp(staleMarker, "u"),
    `${label} should clear stale terminal scrollback before the session picker renders`,
  );
  assert.doesNotMatch(
    text,
    /Enter to send/u,
    `${label} should not carry the chat composer into the session picker`,
  );
  assert.equal(
    markerCount(text, "TYPED_USER_1"),
    0,
    `${label} should not carry older chat rows into the session picker`,
  );
  assert.ok(
    markerCount(text, "TYPED_REPLY_2") <= 1,
    `${label} may show the active session preview once, but not duplicate it`,
  );
}
