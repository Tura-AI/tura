import assert from "node:assert/strict";
import http from "node:http";
import test from "node:test";
import { agentCommand } from "../../../src/commands/agent.js";
import { CliUsageError, type CliContext } from "../../../src/types/common.js";

test("agent model rejects priority flags instead of writing agent-level priority", async () => {
  const seen: Array<{ method?: string; url?: string; body?: unknown }> = [];
  await withServer(
    async (req, res) => {
      seen.push({ method: req.method, url: req.url, body: await readBody(req) });
      if (req.method === "GET" && req.url?.startsWith("/project/current")) {
        return sendJson(res, { project: null });
      }
      if (req.method === "GET" && req.url === "/agent/fast") {
        return sendJson(res, {
          summary: { id: "fast", name: "Fast", source: "dynamic", path: "agents/fast.md" },
          config: { agent_name: "fast", provider: { tura_llm_name: "thinking" } },
          prompt: "fast prompt",
        });
      }
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: `unexpected ${req.method} ${req.url}` }));
    },
    async (baseUrl) => {
      await assert.rejects(
        agentCommand(context(baseUrl), ["model", "fast", "codex/gpt-5.5", "--priority"]),
        (error) => {
          assert.ok(error instanceof CliUsageError);
          assert.match(error.message, /--priority/u);
          return true;
        },
      );
    },
  );

  assert.equal(
    seen.some((request) => request.method === "PATCH" && request.url === "/agent/fast"),
    false,
  );
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

function context(baseUrl: string): CliContext {
  return {
    gatewayUrl: baseUrl,
    gatewayUrlExplicit: true,
    cwd: "C:/repo",
    json: false,
    color: "never",
    display: "plain",
    verbose: false,
    mock: false,
    dev: false,
  };
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
