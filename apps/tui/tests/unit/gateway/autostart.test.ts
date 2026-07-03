import test from "node:test";
import assert from "node:assert/strict";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { ensureGatewayAvailable, _gatewayProbeForTest } from "../../../src/gateway/autostart.js";
import { plainCapabilities } from "../../../src/tui/capabilities.js";

test("gateway probe accepts an existing healthy gateway", async () => {
  const server = createServer((req, res) => {
    assert.equal(req.url, "/global/health");
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ healthy: true, root: process.cwd(), version: "test" }));
  });
  await listen(server);
  try {
    const address = server.address() as AddressInfo;
    assert.equal(await _gatewayProbeForTest(`http://127.0.0.1:${address.port}`), true);
  } finally {
    await close(server);
  }
});

test("ensureGatewayAvailable returns a reachable explicit gateway without spawning", async () => {
  const server = createServer((_req, res) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ healthy: true, root: process.cwd() }));
  });
  await listen(server);
  try {
    const address = server.address() as AddressInfo;
    const url = `http://127.0.0.1:${address.port}`;

    await assert.doesNotReject(
      ensureGatewayAvailable(url, plainCapabilities(), false, true),
      "standalone TUI should connect to an existing gateway",
    );
  } finally {
    await close(server);
  }
});

test("ensureGatewayAvailable fails when explicit gateway is absent", async () => {
  const server = createServer();
  await listen(server);
  const address = server.address() as AddressInfo;
  await close(server);

  await assert.rejects(
    ensureGatewayAvailable(`http://127.0.0.1:${address.port}`, plainCapabilities(), false, true),
    /TUI only connects to an existing gateway/u,
  );
});

test("ensureGatewayAvailable rejects non-explicit active gateway from another root", async () => {
  const home = mkdtempSync(join(tmpdir(), "tura-tui-foreign-gateway-home-"));
  const projectRoot = mkdtempSync(join(tmpdir(), "tura-tui-project-root-"));
  const server = createServer((_req, res) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ healthy: true, root: join(tmpdir(), "foreign-root") }));
  });
  await listen(server);
  const previousHome = process.env.TURA_HOME;
  const previousRoot = process.env.TURA_PROJECT_ROOT;
  const previousUrl = process.env.TURA_GATEWAY_URL;
  try {
    const address = server.address() as AddressInfo;
    process.env.TURA_HOME = home;
    process.env.TURA_PROJECT_ROOT = projectRoot;
    delete process.env.TURA_GATEWAY_URL;
    mkdirSync(join(home, ".tura"), { recursive: true });
    writeFileSync(
      join(home, ".tura", "gateway-active.env"),
      `TURA_GATEWAY_URL=http://127.0.0.1:${address.port}\n`,
    );

    await assert.rejects(
      ensureGatewayAvailable("http://127.0.0.1:65530", plainCapabilities(), false, false),
      /No gateway for this Tura home is running/u,
    );
  } finally {
    if (previousHome === undefined) delete process.env.TURA_HOME;
    else process.env.TURA_HOME = previousHome;
    if (previousRoot === undefined) delete process.env.TURA_PROJECT_ROOT;
    else process.env.TURA_PROJECT_ROOT = previousRoot;
    if (previousUrl === undefined) delete process.env.TURA_GATEWAY_URL;
    else process.env.TURA_GATEWAY_URL = previousUrl;
    await close(server);
    rmSync(home, { recursive: true, force: true });
    rmSync(projectRoot, { recursive: true, force: true });
  }
});

function listen(server: Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
}

function close(server: Server): Promise<void> {
  return new Promise((resolve) => server.close(() => resolve()));
}
