import test from "node:test";
import assert from "node:assert/strict";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
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

function listen(server: Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
}

function close(server: Server): Promise<void> {
  return new Promise((resolve) => server.close(() => resolve()));
}
