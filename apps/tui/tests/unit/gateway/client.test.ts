import assert from "node:assert/strict";
import http from "node:http";
import test from "node:test";
import { GatewayClient } from "../../../src/gateway/client.js";
import { GatewayHttpError } from "../../../src/gateway/errors.js";
import type { Message, Session } from "../../../src/types/session.js";

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

      sendJson(res, session("sess-1", { directory: "C:/repo with spaces" }));
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

test("GatewayClient maps About operations to fixed Gateway endpoints", async () => {
  const seen: Array<{ method?: string; url?: string; body?: unknown }> = [];
  await withServer(
    async (req, res) => {
      seen.push({ method: req.method, url: req.url, body: await readBody(req) });
      if (req.url === "/about") {
        return sendJson(res, {
          release_version: "0.1.30",
          system: { operating_system: "Windows", os_version: "11", architecture: "x86_64" },
        });
      }
      if (req.url === "/about/star") return sendJson(res, { outcome: "starred" });
      if (req.url === "/about/open") {
        return sendJson(res, { opened: true, target: "report_bug" });
      }
      if (req.url === "/about/update/check") return sendJson(res, {});
      return sendJson(res, { scheduled: true, version: "0.1.31" });
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      assert.equal((await client.aboutInfo()).release_version, "0.1.30");
      assert.equal((await client.starTuraRepository()).outcome, "starred");
      assert.equal((await client.openAboutTarget("report_bug")).target, "report_bug");
      assert.equal((await client.checkTuraUpdate()).update, undefined);
      assert.equal((await client.installTuraUpdate("0.1.31", "session-1")).scheduled, true);
    },
  );

  assert.deepEqual(seen, [
    { method: "GET", url: "/about", body: undefined },
    { method: "POST", url: "/about/star", body: {} },
    { method: "POST", url: "/about/open", body: { target: "report_bug" } },
    { method: "GET", url: "/about/update/check", body: undefined },
    {
      method: "POST",
      url: "/about/update/install",
      body: { version: "0.1.31", session_id: "session-1" },
    },
  ]);
});

test("GatewayClient reads strict gateway message arrays", async () => {
  await withServer(
    async (_req, res) => {
      sendJson(res, [message("sess-1", "msg-1", "assistant", "hello")]);
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      const messages = await client.listMessages("sess-1");
      assert.equal(messages[0].parts[0].text, "hello");
    },
  );
});

test("GatewayClient deletes and forks sessions through session endpoints", async () => {
  const seen: Array<{ method?: string; url?: string; body?: unknown }> = [];
  await withServer(
    async (req, res) => {
      seen.push({ method: req.method, url: req.url, body: await readBody(req) });
      if (req.method === "DELETE") return sendJson(res, true);
      sendJson(res, session("sess-copy", { parent_id: "sess-1", directory: "C:/repo" }));
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      assert.equal(await client.deleteSession("sess-1"), true);
      const session = await client.forkSession("sess-1", { copy_context: true });
      assert.equal(session.id, "sess-copy");
    },
  );

  assert.equal(seen[0].method, "DELETE");
  assert.equal(seen[0].url, "/session/sess-1");
  assert.equal(seen[1].method, "POST");
  assert.equal(seen[1].url, "/session/sess-1/fork");
  assert.deepEqual(seen[1].body, { directory: "C:/repo", copy_context: true });
});

test("GatewayClient fork/delete flow observes gateway session state", async () => {
  const sessions = new Map<string, Session>([
    ["sess-1", session("sess-1", { name: "Root", message_count: 2 })],
  ]);
  const messages = new Map<string, Message[]>([
    [
      "sess-1",
      [
        message("sess-1", "msg-user", "user", "build it"),
        message("sess-1", "msg-assistant", "assistant", "done"),
      ],
    ],
  ]);

  await withServer(
    async (req, res) => {
      const path = new URL(req.url ?? "/", "http://127.0.0.1");
      if (req.method === "GET" && path.pathname === "/session") {
        return sendJson(res, [...sessions.values()]);
      }
      if (req.method === "GET" && path.pathname === "/session/sess-copy") {
        return sendJson(res, sessions.get("sess-copy"));
      }
      if (req.method === "GET" && path.pathname === "/session/sess-copy/message") {
        return sendJson(res, messages.get("sess-copy") ?? []);
      }
      if (req.method === "POST" && path.pathname === "/session/sess-1/fork") {
        const body = await readBody(req);
        assert.deepEqual(body, { directory: "C:/repo", copy_context: true });
        const fork = session("sess-copy", {
          parent_id: "sess-1",
          name: "Root",
          message_count: messages.get("sess-1")?.length ?? 0,
        });
        sessions.set(fork.id, fork);
        messages.set(
          fork.id,
          (messages.get("sess-1") ?? []).map((item, index) =>
            message(fork.id, `fork-${index}`, item.role, item.parts[0]?.text ?? ""),
          ),
        );
        return sendJson(res, fork);
      }
      if (req.method === "DELETE" && path.pathname === "/session/sess-copy") {
        sessions.delete("sess-copy");
        messages.delete("sess-copy");
        return sendJson(res, true);
      }
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: `unexpected ${req.method} ${path.pathname}` }));
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      const fork = await client.forkSession("sess-1", { copy_context: true });
      assert.equal(fork.id, "sess-copy");
      assert.equal(fork.parent_id, "sess-1");

      const listedAfterFork = await client.listSessions({ includeChildren: true });
      assert.deepEqual(listedAfterFork.map((item) => item.id).sort(), ["sess-1", "sess-copy"]);
      assert.equal((await client.getSession("sess-copy")).parent_id, "sess-1");
      assert.deepEqual(
        (await client.listMessages("sess-copy")).map((item) => item.parts[0]?.text),
        ["build it", "done"],
      );

      assert.equal(await client.deleteSession("sess-copy"), true);
      assert.deepEqual(
        (await client.listSessions({ includeChildren: true })).map((item) => item.id),
        ["sess-1"],
      );
      assert.deepEqual(await client.listMessages("sess-copy"), []);
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

test("GatewayClient streams session-scoped events from the session events endpoint", async () => {
  let seenUrl = "";
  await withServer(
    async (req, res) => {
      seenUrl = req.url ?? "";
      const event = {
        directory: "C:/repo",
        payload: {
          type: "message.updated",
          properties: { sessionID: "sess 1", info: { id: "runtime.message" } },
        },
      };
      res.writeHead(200, { "content-type": "text/event-stream" });
      res.end(`data: ${JSON.stringify(event)}\n\n`);
    },
    async (baseUrl) => {
      const client = new GatewayClient({ baseUrl, directory: "C:/repo" });
      const stream = client.streamSessionEvents("sess 1");
      const next = await stream.next();
      assert.equal(next.done, false);
      assert.equal(next.value.payload?.type, "message.updated");
    },
  );

  assert.equal(seenUrl, "/session/sess%201/events");
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

function session(id: string, overrides: Partial<Session> = {}): Session {
  return {
    id,
    name: null,
    parent_id: null,
    created_at: 1,
    updated_at: 1,
    directory: "C:/repo",
    model: "openai",
    agent: "thinking-planning",
    session_type: "coding",
    auto_session_name: true,
    kill_processes_on_start: false,
    validator_enabled: false,
    force_planning: false,
    model_variant: null,
    model_acceleration_enabled: false,
    disable_permission_restrictions: false,
    status: "idle",
    message_count: 0,
    task_management: {},
    plan_summary: null,
    session_display_name: null,
    ...overrides,
  };
}

function message(sessionID: string, id: string, role: Message["role"], text: string): Message {
  return {
    id,
    sessionID,
    parentID: null,
    role,
    parts: [
      {
        id: `${id}.part`,
        sessionID,
        messageID: id,
        type: "text",
        text,
        content: text,
        metadata: null,
        callID: null,
        tool: null,
        state: null,
      },
    ],
    created_at: 1,
    updated_at: 1,
    time: { created: 1, updated: 1 },
  };
}
