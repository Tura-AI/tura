#!/usr/bin/env node

const DEFAULT_GATEWAY_URL = "http://127.0.0.1:4096";
const DEFAULT_ITERATIONS = 20;
const DEFAULT_TIMEOUT_MS = 30_000;

function argValue(name, fallback = undefined) {
  const index = process.argv.indexOf(`--${name}`);
  if (index >= 0 && index + 1 < process.argv.length) {
    return process.argv[index + 1];
  }
  return fallback;
}

function intValue(value, fallback) {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function percentile(sorted, p) {
  if (!sorted.length) return null;
  const index = Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1);
  return sorted[index];
}

function summarize(samples) {
  const durations = samples
    .filter((sample) => Number.isFinite(sample.ms))
    .map((sample) => sample.ms)
    .sort((a, b) => a - b);
  const ok = samples.filter((sample) => sample.ok).length;
  const errors = samples.length - ok;
  const sum = durations.reduce((acc, value) => acc + value, 0);
  return {
    count: samples.length,
    ok,
    errors,
    min_ms: durations.length ? Number(durations[0].toFixed(2)) : null,
    avg_ms: durations.length ? Number((sum / durations.length).toFixed(2)) : null,
    p50_ms: durations.length ? Number(percentile(durations, 50).toFixed(2)) : null,
    p95_ms: durations.length ? Number(percentile(durations, 95).toFixed(2)) : null,
    max_ms: durations.length ? Number(durations[durations.length - 1].toFixed(2)) : null,
  };
}

async function requestJson(url, options = {}, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const start = performance.now();
  try {
    const response = await fetch(url, { ...options, signal: controller.signal });
    const text = await response.text();
    let body = null;
    try {
      body = text ? JSON.parse(text) : null;
    } catch {
      body = text;
    }
    return {
      ok: response.ok && (body?.ok ?? true) !== false,
      http_ok: response.ok,
      status: response.status,
      ms: performance.now() - start,
      body,
    };
  } catch (error) {
    return {
      ok: false,
      http_ok: false,
      status: null,
      ms: performance.now() - start,
      error: error?.message ?? String(error),
    };
  } finally {
    clearTimeout(timer);
  }
}

async function discoverModelTarget(gatewayUrl, timeoutMs) {
  const provider = argValue("provider", process.env.PROVIDER_ID);
  const model = argValue("model", process.env.MODEL_ID);
  if (provider && model) {
    return { provider, model, source: "args" };
  }

  const response = await requestJson(`${gatewayUrl}/model_config`, {}, timeoutMs);
  if (!response.ok || !Array.isArray(response.body?.tiers)) {
    throw new Error(`failed to discover model target from /model_config: ${response.error ?? response.body?.message ?? response.status}`);
  }

  for (const tier of response.body.tiers) {
    const current = tier?.current;
    if (current?.provider && current?.model) {
      return {
        provider: current.provider,
        model: current.model,
        tier: tier.tier,
        source: "model_config.current",
      };
    }
  }

  for (const tier of response.body.tiers) {
    const option = Array.isArray(tier?.options) ? tier.options[0] : null;
    if (option?.provider && option?.model) {
      return {
        provider: option.provider,
        model: option.model,
        tier: tier.tier,
        source: "model_config.options",
      };
    }
  }

  throw new Error("no configured provider/model target found in /model_config");
}

async function runSeries(label, iterations, call) {
  const samples = [];
  for (let index = 0; index < iterations; index += 1) {
    const sample = await call(index);
    samples.push({
      index: index + 1,
      ok: sample.ok,
      status: sample.status,
      ms: Number(sample.ms.toFixed(2)),
      code: sample.body?.code,
      message: sample.body?.message ?? sample.error,
    });
    const marker = sample.ok ? "ok" : "fail";
    console.log(`${label} ${index + 1}/${iterations}: ${marker} ${sample.status ?? "ERR"} ${sample.ms.toFixed(2)}ms`);
  }
  return samples;
}

const gatewayUrl = argValue("gateway", process.env.GATEWAY_URL ?? DEFAULT_GATEWAY_URL).replace(/\/+$/, "");
const iterations = intValue(argValue("iterations", process.env.ITERATIONS), DEFAULT_ITERATIONS);
const timeoutMs = intValue(argValue("timeout-ms", process.env.TIMEOUT_MS), DEFAULT_TIMEOUT_MS);
const target = await discoverModelTarget(gatewayUrl, timeoutMs);

console.log(JSON.stringify({
  gateway_url: gatewayUrl,
  iterations,
  timeout_ms: timeoutMs,
  model_target: target,
}, null, 2));

const gatewaySamples = await runSeries("gateway", iterations, () =>
  requestJson(`${gatewayUrl}/global/health`, {}, timeoutMs)
);

const modelSamples = await runSeries("model", iterations, () =>
  requestJson(
    `${gatewayUrl}/provider/model/validate`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        providerID: target.provider,
        modelID: target.model,
      }),
    },
    timeoutMs
  )
);

const result = {
  gateway_url: gatewayUrl,
  iterations,
  timeout_ms: timeoutMs,
  model_target: target,
  summary: {
    gateway: summarize(gatewaySamples),
    model: summarize(modelSamples),
  },
  samples: {
    gateway: gatewaySamples,
    model: modelSamples,
  },
};

console.log(JSON.stringify(result, null, 2));

if (result.summary.gateway.errors || result.summary.model.errors) {
  process.exitCode = 1;
}
