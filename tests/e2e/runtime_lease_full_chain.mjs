#!/usr/bin/env node
import assert from "node:assert/strict";
import fsp from "node:fs/promises";
import process from "node:process";

import { startBackendStressEnvironment } from "./full_chain_backend_fixture.mjs";

async function main() {
  const backend = await startBackendStressEnvironment({
    runIdPrefix: "runtime-lease-e2e",
    config: {
      workspaces: 1,
      tasksPerWorkspace: 1,
      turnsPerSession: 1,
      liveSessionTarget: 1,
      turnTimeoutMs: 30_000,
      totalTimeoutMs: 240_000,
      createSessionConcurrency: 1,
      gatewayVerifyConcurrency: 1,
      ensureBuilds: true,
      forceKillTrackedChildren: true,
    },
  });

  try {
    const sessionLog = await backend.verifySessionLog();
    assert.equal(
      backend.providerRequests.length,
      1,
      "the local mock provider must receive exactly one real runtime turn",
    );
    assert.equal(
      backend.requestErrors.length,
      0,
      `the full chain must not return gateway/router/runtime errors: ${JSON.stringify(backend.requestErrors)}`,
    );

    const summary = backend.summaryBase({
      ok: true,
      suite: "backend-e2e",
      provider: "local-mock",
      chain: ["gateway", "router", "runtime", "session_db"],
      sessionLog,
    });
    await fsp.writeFile(backend.summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
  } catch (error) {
    const failureDiagnostics = await backend.collectFailureDiagnostics().catch((diagnosticError) => ({
      error: String(diagnosticError?.stack || diagnosticError?.message || diagnosticError),
    }));
    const summary = backend.summaryBase({
      ok: false,
      suite: "backend-e2e",
      provider: "local-mock",
      failureDiagnostics,
      error: error instanceof Error ? error.stack || error.message : String(error),
    });
    await fsp.writeFile(backend.summaryPath, JSON.stringify(summary, null, 2));
    console.error(JSON.stringify(summary, null, 2));
    process.exitCode = 1;
  } finally {
    await backend.cleanup();
  }
}

await main();
