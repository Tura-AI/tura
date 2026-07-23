#!/usr/bin/env node
import fsp from "node:fs/promises";
import process from "node:process";
import { startBackendStressEnvironment } from "../e2e/full_chain_backend_fixture.mjs";

async function main() {
  const backend = await startBackendStressEnvironment({ runIdPrefix: "backend-full-chain" });
  let completedOk = false;
  try {
    const sessionLog = await backend.verifySessionLog();
    const summary = backend.summaryBase({
      ok: true,
      owner: "backend",
      sessionLog,
      budget: {
        totalTimeoutMs: backend.config.totalTimeoutMs,
        remainingMs: Math.max(0, backend.stressDeadline - Date.now()),
      },
    });
    await fsp.writeFile(backend.summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
    completedOk = true;
  } catch (error) {
    const failureDiagnostics = await backend.collectFailureDiagnostics().catch((diagnosticError) => ({
      error: String(diagnosticError?.stack || diagnosticError?.message || diagnosticError),
    }));
    const summary = backend.summaryBase({
      ok: false,
      owner: "backend",
      budget: {
        totalTimeoutMs: backend.config.totalTimeoutMs,
        remainingMs: Math.max(0, backend.stressDeadline - Date.now()),
      },
      failureDiagnostics,
      error: error instanceof Error ? error.stack || error.message : String(error),
    });
    await fsp.writeFile(backend.summaryPath, JSON.stringify(summary, null, 2));
    console.error(JSON.stringify(summary, null, 2));
    process.exitCode = 1;
  } finally {
    await backend.cleanup();
    if (completedOk) process.exitCode = 0;
  }
}

await main();
