#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
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
  const logDir = path.join(repoRoot, "apps", "tui", "test-results", "detached-web-terminal");
  fs.mkdirSync(logDir, { recursive: true });
  const stdout = fs.openSync(path.join(logDir, `${port}.stdout.log`), "a");
  const stderr = fs.openSync(path.join(logDir, `${port}.stderr.log`), "a");
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
      detached: true,
      stdio: ["ignore", stdout, stderr],
      windowsHide: true,
    },
  );
  child.unref();
  return { child, logs: () => "" };
}

export async function terminalText(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  );
}

export async function startFramePresenceMonitor(page, markers) {
  await page.evaluate((nextMarkers) => {
    const readVisibleText = () =>
      [...document.querySelectorAll(".xterm-rows > div")]
        .map((node) => node.textContent ?? "")
        .join("\n");
    const state = {
      markers: nextMarkers,
      running: true,
      samples: [],
      frame: 0,
      request: undefined,
    };
    const step = () => {
      const visibleText = readVisibleText();
      state.samples.push({
        frame: state.frame,
        time: performance.now(),
        visible: Object.fromEntries(
          state.markers.map((marker) => [marker, visibleText.includes(marker)]),
        ),
      });
      state.frame += 1;
      if (state.running) state.request = requestAnimationFrame(step);
    };
    if (window.__turaFramePresenceMonitor?.request) {
      cancelAnimationFrame(window.__turaFramePresenceMonitor.request);
    }
    window.__turaFramePresenceMonitor = state;
    state.request = requestAnimationFrame(step);
  }, markers);
}

export async function stopFramePresenceMonitor(page) {
  return page.evaluate(() => {
    const state = window.__turaFramePresenceMonitor;
    if (!state) return [];
    state.running = false;
    if (state.request) cancelAnimationFrame(state.request);
    const samples = state.samples;
    delete window.__turaFramePresenceMonitor;
    return samples;
  });
}

export function assertNoMarkerBlink(samples, markers, label) {
  assert.ok(samples.length > 0, `${label} should capture terminal frames`);
  for (const marker of markers) {
    const states = samples.map((sample) => Boolean(sample.visible?.[marker]));
    const firstSeen = states.indexOf(true);
    assert.ok(firstSeen >= 0, `${label} should see ${marker} before checking blink`);
    const missingAfterSeen = states.findIndex((present, index) => index > firstSeen && !present);
    assert.equal(
      missingAfterSeen,
      -1,
      `${label} ${marker} disappeared after first visibility at frame ${missingAfterSeen}`,
    );
  }
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
    if (nextTarget === "middle") {
      const baseY = Number(term.buffer?.active?.baseY) || 0;
      if (typeof term.scrollToLine === "function") term.scrollToLine(Math.floor(baseY / 2));
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

export async function terminalViewportPosition(page) {
  return page.evaluate(() => {
    const buffer = window.__turaTerminal?.buffer?.active;
    return {
      baseY: Number(buffer?.baseY) || 0,
      viewportY: Number(buffer?.viewportY) || 0,
    };
  });
}

export function markerCount(text, marker) {
  return text.split(marker).length - 1;
}

export function regexCount(text, pattern) {
  return Array.from(text.matchAll(pattern)).length;
}

export const composerHintPattern = /Enter(?::| to)? send/u;
export const composerHintGlobalPattern = /Enter(?::| to)? send/gu;

export async function waitForComposer(page, timeoutMs = 5000) {
  await page.waitForFunction(() => /Enter(?::| to)? send/.test(document.body.innerText), null, {
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

export function assertSessionPickerCleared(text, label) {
  assert.doesNotMatch(
    text,
    composerHintPattern,
    `${label} should not carry the chat composer into the session picker`,
  );
}
