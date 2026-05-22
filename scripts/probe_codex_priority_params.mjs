import fs from "node:fs";

const AUTH_PATH = "C:/Users/liuliu/.codex/auth.json";
const INSTALLATION_ID_PATH = "C:/Users/liuliu/.codex/installation_id";
const ENDPOINT = "https://chatgpt.com/backend-api/codex/responses";
const CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann";
const DEFAULT_ROUNDS = 2;

const roundsArg = process.argv.find((arg) => arg.startsWith("--rounds="));
const rounds = roundsArg ? Number(roundsArg.split("=")[1]) : DEFAULT_ROUNDS;
const variantsArg = process.argv.find((arg) => arg.startsWith("--variants="));
const variantFilter = variantsArg
  ? new Set(
      variantsArg
        .split("=")[1]
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean),
    )
  : null;

const prompt =
  "Output exactly 1000 characters, all zeros, no spaces, no punctuation, no markdown, no newline.";

const codexUserAgent = "codex_cli_rs/0.0.0 (Windows 10.0; x86_64)";
const turaUserAgent = "tura-os";

function readAuth() {
  return JSON.parse(fs.readFileSync(AUTH_PATH, "utf8"));
}

function writeAuth(auth) {
  fs.writeFileSync(AUTH_PATH, `${JSON.stringify(auth, null, 2)}\n`);
}

async function refreshAuthIfNeeded() {
  const auth = readAuth();
  if (!auth.tokens?.refresh_token) {
    throw new Error("No refresh_token in auth.json");
  }
  const res = await fetch("https://auth.openai.com/oauth/token", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      originator: "codex_cli_rs",
      "User-Agent": codexUserAgent,
    },
    body: JSON.stringify({
      client_id: CLIENT_ID,
      grant_type: "refresh_token",
      refresh_token: auth.tokens.refresh_token,
    }),
  });
  if (!res.ok) {
    throw new Error(`OAuth refresh failed ${res.status}: ${(await res.text()).slice(0, 500)}`);
  }
  const refreshed = await res.json();
  if (refreshed.id_token) auth.tokens.id_token = refreshed.id_token;
  if (refreshed.access_token) auth.tokens.access_token = refreshed.access_token;
  if (refreshed.refresh_token) auth.tokens.refresh_token = refreshed.refresh_token;
  auth.last_refresh = new Date().toISOString();
  writeAuth(auth);
}

function installationId() {
  return fs.existsSync(INSTALLATION_ID_PATH)
    ? fs.readFileSync(INSTALLATION_ID_PATH, "utf8").trim()
    : undefined;
}

function accountId(auth) {
  return auth.tokens?.account_id || process.env.OPENAI_ACCOUNT_ID || undefined;
}

function codexBaseline(round, variantName) {
  const auth = readAuth();
  const sessionId = `priority-param-probe-${round}-${variantName}`;
  const headers = {
    Authorization: `Bearer ${auth.tokens.access_token}`,
    "Content-Type": "application/json",
    Accept: "text/event-stream",
    originator: "codex_cli_rs",
    "User-Agent": codexUserAgent,
    session_id: sessionId,
    prompt_cache_key: "priority-param-probe-fixed-prefix",
  };
  const install = installationId();
  if (install) headers["x-codex-installation-id"] = install;
  const acct = accountId(auth);
  if (acct) headers["ChatGPT-Account-Id"] = acct;

  return {
    headers,
    body: {
      model: "gpt-5.5",
      stream: true,
      store: false,
      instructions: "You are Codex. Follow the user instruction exactly.",
      reasoning: { effort: "low" },
      service_tier: "priority",
      input: [
        {
          role: "user",
          content: [{ type: "input_text", text: prompt }],
        },
      ],
    },
  };
}

const variants = [
  {
    name: "baseline_codex_priority",
    description: "Codex-compatible priority request.",
    mutate: () => {},
  },
  {
    name: "originator_tura_os",
    description: "Only originator becomes tura-os.",
    mutate: (req) => {
      req.headers.originator = "tura-os";
    },
  },
  {
    name: "user_agent_tura_os",
    description: "Only User-Agent becomes tura-os.",
    mutate: (req) => {
      req.headers["User-Agent"] = turaUserAgent;
    },
  },
  {
    name: "headers_tura_os",
    description: "originator and User-Agent both become tura-os.",
    mutate: (req) => {
      req.headers.originator = "tura-os";
      req.headers["User-Agent"] = turaUserAgent;
    },
  },
  {
    name: "no_service_tier",
    description: "Remove service_tier priority.",
    mutate: (req) => {
      delete req.body.service_tier;
    },
  },
  {
    name: "prompt_cache_key_tura_style",
    description: "Only prompt_cache_key becomes a tura-style stable key.",
    mutate: (req) => {
      req.headers.prompt_cache_key = "turaosv2:priority-param-probe-fixed-prefix";
      req.body.prompt_cache_key = "turaosv2:priority-param-probe-fixed-prefix";
    },
  },
  {
    name: "session_headers_tura_style",
    description: "Add Tura's extra session/thread headers.",
    mutate: (req, round, variantName) => {
      const sessionId = `priority-param-probe-${round}-${variantName}`;
      req.headers["session-id"] = sessionId;
      req.headers.thread_id = sessionId;
      req.headers["thread-id"] = sessionId;
      req.headers["x-client-request-id"] = sessionId;
    },
  },
  {
    name: "omit_installation_id",
    description: "Remove x-codex-installation-id.",
    mutate: (req) => {
      delete req.headers["x-codex-installation-id"];
    },
  },
  {
    name: "omit_account_id",
    description: "Remove ChatGPT-Account-Id.",
    mutate: (req) => {
      delete req.headers["ChatGPT-Account-Id"];
    },
  },
  {
    name: "omit_reasoning",
    description: "Remove reasoning effort.",
    mutate: (req) => {
      delete req.body.reasoning;
    },
  },
  {
    name: "parallel_tool_calls_false",
    description: "Add parallel_tool_calls=false.",
    mutate: (req) => {
      req.body.parallel_tool_calls = false;
    },
  },
  {
    name: "instructions_tura_minimal",
    description: "Use a minimal Tura-branded instruction text.",
    mutate: (req) => {
      req.body.instructions = "You are Tura OS. Follow the user instruction exactly.";
    },
  },
  {
    name: "input_content_string",
    description: "Use Responses input content as a plain string instead of input_text array.",
    mutate: (req) => {
      req.body.input = [{ role: "user", content: prompt }];
    },
  },
];
const selectedVariants = variantFilter
  ? variants.filter((variant) => variantFilter.has(variant.name))
  : variants;

if (!selectedVariants.length) {
  throw new Error(`No variants selected by --variants=${variantsArg}`);
}

function eventBlocks(buffer) {
  const blocks = buffer.split(/\r?\n\r?\n/);
  return { complete: blocks.slice(0, -1), rest: blocks.at(-1) || "" };
}

function parseEvent(block) {
  const lines = block.split(/\r?\n/);
  let type = "";
  let data = "";
  for (const line of lines) {
    if (line.startsWith("event:")) type = line.slice(6).trim();
    if (line.startsWith("data:")) data += line.slice(5).trim();
  }
  if (!data || data === "[DONE]") return { type, data: null };
  try {
    return { type, data: JSON.parse(data) };
  } catch {
    return { type, data: null };
  }
}

async function runVariant(round, variant) {
  const req = codexBaseline(round, variant.name);
  variant.mutate(req, round, variant.name);

  const started = performance.now();
  const res = await fetch(ENDPOINT, {
    method: "POST",
    headers: req.headers,
    body: JSON.stringify(req.body),
  });
  const headersAt = performance.now();
  if (!res.ok) {
    return {
      round,
      variant: variant.name,
      ok: false,
      status: res.status,
      error: (await res.text()).slice(0, 500),
    };
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let firstOutputAt = null;
  let completedAt = null;
  let responseTier = null;
  let usage = null;
  let text = "";
  let events = 0;

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const parts = eventBlocks(buffer);
    buffer = parts.rest;
    for (const block of parts.complete) {
      const ev = parseEvent(block);
      events += 1;
      if (!ev.data) continue;
      const data = ev.data;
      if (data.type === "response.output_text.delta" && typeof data.delta === "string") {
        if (firstOutputAt === null) firstOutputAt = performance.now();
        text += data.delta;
      } else {
        const delta = data.delta || data.text || data.output_text_delta;
        if (typeof delta === "string" && delta.length) {
          if (firstOutputAt === null) firstOutputAt = performance.now();
          text += delta;
        }
      }
      if (data.type === "response.completed" || ev.type === "response.completed") {
        completedAt = performance.now();
        const response = data.response || data;
        responseTier = response.service_tier || response.serviceTier || responseTier;
        usage = response.usage || usage;
      }
      if (data.response?.usage) usage = data.response.usage;
      if (data.response?.service_tier) responseTier = data.response.service_tier;
    }
  }

  const end = completedAt || performance.now();
  const first = firstOutputAt || end;
  const genMs = Math.max(1, end - first);
  const outputTokens = usage?.output_tokens ?? usage?.outputTokens ?? null;
  const inputTokens = usage?.input_tokens ?? usage?.inputTokens ?? null;

  return {
    round,
    variant: variant.name,
    ok: true,
    response_tier: responseTier || "unknown",
    headers_ms: Math.round(headersAt - started),
    first_output_ms: Math.round(first - started),
    completed_ms: Math.round(end - started),
    gen_ms: Math.round(genMs),
    input_tokens: inputTokens,
    output_tokens: outputTokens,
    chars: text.length,
    zeros: (text.match(/0/g) || []).length,
    output_tps: outputTokens ? +(outputTokens / (genMs / 1000)).toFixed(2) : null,
    char_tps: +(text.length / (genMs / 1000)).toFixed(2),
    events,
  };
}

function average(items, field) {
  const values = items.map((item) => item[field]).filter((value) => Number.isFinite(value));
  if (!values.length) return null;
  return +(values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(2);
}

function summarize(results) {
  const baseline = results.filter((item) => item.ok && item.variant === "baseline_codex_priority");
  const baselineTps = average(baseline, "output_tps");
  const baselineCompleted = average(baseline, "completed_ms");
  return selectedVariants.map((variant) => {
    const items = results.filter((item) => item.ok && item.variant === variant.name);
    const avgTps = average(items, "output_tps");
    const avgCompleted = average(items, "completed_ms");
    return {
      variant: variant.name,
      n: items.length,
      avg_first_output_ms: average(items, "first_output_ms"),
      avg_completed_ms: avgCompleted,
      avg_output_tps: avgTps,
      response_tiers: [...new Set(items.map((item) => item.response_tier))].join(","),
      tps_vs_baseline_pct:
        avgTps && baselineTps ? +((avgTps / baselineTps - 1) * 100).toFixed(1) : null,
      completed_vs_baseline_pct:
        avgCompleted && baselineCompleted
          ? +((avgCompleted / baselineCompleted - 1) * 100).toFixed(1)
          : null,
      description: variant.description,
    };
  });
}

await refreshAuthIfNeeded();

const results = [];
for (let round = 1; round <= rounds; round += 1) {
  for (const variant of selectedVariants) {
    const result = await runVariant(round, variant);
    results.push(result);
    console.log(JSON.stringify(result));
  }
}

const summary = summarize(results);
console.log("SUMMARY");
console.table(summary);
fs.writeFileSync(
  "priority-param-probe-results.json",
  `${JSON.stringify({ rounds, results, summary }, null, 2)}\n`,
);
