import assert from "node:assert/strict";
import http from "node:http";
import test from "node:test";
import { agentCommand } from "../../../src/commands/agent.js";
import {
  agentRuntimeConfig,
  agentRuntimeRequest,
  applyAgentRuntimeConfig,
  formatAgentRuntimeModelText,
  modelForRuntimeTier,
} from "../../../src/agent-runtime-config.js";
import type { CliContext } from "../../../src/types/common.js";

test("agent runtime resolves default tiers to models without displaying tier names", () => {
  const modelConfig = {
    tiers: [
      {
        tier: "thinking",
        current: { provider: "codex", model: "gpt-5.5" },
      },
    ],
  };
  const agent = {
    options: {
      provider: {
        default_model_tier: "thinking",
        model_reasoning_effort: "medium",
        model_acceleration_enabled: true,
      },
    },
  };

  const runtime = agentRuntimeConfig(agent);
  const request = agentRuntimeRequest(agent, { modelConfig, model: "openai/fallback" });
  const displayModel = modelForRuntimeTier(modelConfig, runtime.defaultModelTier);

  assert.equal(request.model, "codex/gpt-5.5");
  assert.equal(request.variant, "medium");
  assert.equal(request.model_acceleration_enabled, true);
  assert.equal(
    formatAgentRuntimeModelText(displayModel ?? "", runtime, "p"),
    "codex/gpt-5.5 - medium - p",
  );
});

test("agent runtime omits priority suffix when priority is disabled", () => {
  const runtime = agentRuntimeConfig({
    options: {
      provider: {
        current_model: "codex/gpt-5.5",
        model_reasoning_effort: "high",
        model_acceleration_enabled: false,
      },
    },
  });

  assert.equal(formatAgentRuntimeModelText("codex/gpt-5.5", runtime, "p"), "codex/gpt-5.5 - high");
});

test("agent model writes priority flags through shared runtime config", async () => {
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
      if (req.method === "PATCH" && req.url === "/agent/fast") {
        return sendJson(res, {
          summary: { id: "fast", name: "Fast", source: "dynamic", path: "agents/fast.md" },
          ...(seen.at(-1)?.body as Record<string, unknown>),
        });
      }
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: `unexpected ${req.method} ${req.url}` }));
    },
    async (baseUrl) => {
      await agentCommand(context(baseUrl), ["model", "fast", "codex/gpt-5.5", "--priority"]);
    },
  );

  const patch = seen.find((request) => request.method === "PATCH" && request.url === "/agent/fast");
  assert.ok(patch);
  assert.deepEqual(patch.body, {
    config: {
      agent_name: "fast",
      provider: {
        current_model: "codex/gpt-5.5",
        default_model_tier: "thinking",
        model_acceleration_enabled: true,
        model_reasoning_effort: "high",
        service_tier: "priority",
        tura_llm_name: "thinking",
      },
    },
    prompt: "fast prompt",
  });
});

test("shared agent runtime config round-trips the same GUI and TUI provider shape", () => {
  const config = applyAgentRuntimeConfig(
    {
      agent_name: "fast",
      provider: {
        current_model: "legacy/stale",
        default_model_tier: "direct",
        tura_llm_name: "direct",
        model_reasoning_effort: "low",
        model_acceleration_enabled: true,
        service_tier: "priority",
      },
    },
    {
      defaultModelTier: "thinking",
      currentModel: "codex/gpt-5.5",
      reasoningLevel: "medium",
      priorityEnabled: false,
    },
  );

  assert.deepEqual(config.provider, {
    current_model: "codex/gpt-5.5",
    default_model_tier: "thinking",
    tura_llm_name: "thinking",
    model_reasoning_effort: "medium",
  });
  assert.deepEqual(agentRuntimeConfig(undefined, { config }), {
    defaultModelTier: "thinking",
    currentModel: { provider: "codex", model: "gpt-5.5" },
    reasoningLevel: "medium",
    priorityEnabled: false,
  });
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
