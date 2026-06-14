import assert from "node:assert/strict";
import http from "node:http";
import test from "node:test";
import { GatewayClient } from "./client.js";
import { GatewayHttpError } from "./errors.js";

test("GatewayClient sends workspace directory through query and header", async () => {
  const seen: Array<{ url?: string; directoryHeader?: string; body?: unknown }> = [];
  await withServer(
    async (req, res) => {
      const body = await readBody(req);
      seen.push({
        url: req.url,
        directoryHeader: req.headers["x-opencode-directory"] as string,
        body,
      });
      sendJson(res, { id: "sess-1", directory: "C:/repo with spaces", status: "idle" });
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo with spaces" });
      const session = await client.createSession({ agent: "fast" });
      assert.equal(session.id, "sess-1");
    },
  );

  assert.equal(seen[0].url, "/session?directory=C%3A%2Frepo+with+spaces");
  assert.equal(seen[0].directoryHeader, "C%3A%2Frepo%20with%20spaces");
  assert.deepEqual(seen[0].body, { directory: "C:/repo with spaces", agent: "fast" });
});

test("GatewayClient normalizes message envelopes", async () => {
  await withServer(
    async (_req, res) => {
      sendJson(res, [
        {
          info: { id: "msg-1", sessionID: "sess-1", role: "assistant", parts: [] },
          parts: [{ id: "part-1", type: "text", text: "hello" }],
        },
      ]);
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      const messages = await client.listMessages("sess-1");
      assert.equal(messages[0].parts[0].text, "hello");
    },
  );
});

test("GatewayClient surfaces HTTP error status and body", async () => {
  await withServer(
    async (_req, res) => {
      res.writeHead(418, { "content-type": "application/json" });
      res.end(JSON.stringify({ code: "teapot", message: "short and stout" }));
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      await assert.rejects(client.health(), (error) => {
        assert.ok(error instanceof GatewayHttpError);
        assert.equal(error.status, 418);
        assert.match(error.body ?? "", /teapot/);
        return true;
      });
    },
  );
});

test("GatewayClient converts network failures and timeouts to GatewayHttpError", async () => {
  await withServer(
    async (_req, _res) => {
      await new Promise((resolve) => setTimeout(resolve, 200));
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo", timeoutMs: 20 });
      await assert.rejects(client.health(), (error) => {
        assert.ok(error instanceof GatewayHttpError);
        assert.equal(error.status, 0);
        assert.match(error.message, /aborted|abort|signal/i);
        return true;
      });
    },
  );
});

test("GatewayClient handles concurrent requests without sharing response state", async () => {
  const paths: string[] = [];
  await withServer(
    async (req, res) => {
      paths.push(req.url ?? "");
      if (req.url?.startsWith("/session/")) {
        await new Promise((resolve) => setTimeout(resolve, 25));
        return sendJson(res, [{ id: req.url, sessionID: "s", role: "assistant", parts: [] }]);
      }
      sendJson(res, { healthy: true, version: "test" });
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      const [health, one, two] = await Promise.all([
        client.health(),
        client.listMessages("one"),
        client.listMessages("two"),
      ]);
      assert.equal(health.healthy, true);
      assert.equal(one[0].id, "/session/one/message");
      assert.equal(two[0].id, "/session/two/message");
    },
  );
  assert.equal(paths.length, 3);
});

async function withServer(
  handler: (req: http.IncomingMessage, res: http.ServerResponse) => void | Promise<void>,
  callback: (baseUrl: string) => Promise<void>,
) {
  const server = http.createServer((req, res) => void handler(req, res));
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.ok(address && typeof address === "object");
  try {
    await callback(`http://127.0.0.1:${address.port}`);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
}

function sendJson(res: http.ServerResponse, value: unknown) {
  const body = JSON.stringify(value);
  res.writeHead(200, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(body),
  });
  res.end(body);
}

function readBody(req: http.IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    let body = "";
    req.setEncoding("utf8");
    req.on("data", (chunk) => {
      body += chunk;
    });
    req.on("error", reject);
    req.on("end", () => resolve(body ? JSON.parse(body) : undefined));
  });
}
