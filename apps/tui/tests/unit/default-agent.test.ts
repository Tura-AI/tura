import assert from "node:assert/strict";
import test from "node:test";
import { parseRun } from "../../src/cli.js";
import { MockGatewayClient } from "../../src/gateway/mock-client.js";

test("TUI defaults new CLI runs and mock session config to balanced", async () => {
  const parsed = parseRun(["hello"], false);
  const client = new MockGatewayClient({ directory: process.cwd() });
  const config = await client.getSessionConfig();
  const agents = await client.listAgents();

  assert.equal(parsed.agent, "balanced");
  assert.equal(config.active_agent, "balanced");
  assert.ok(agents.some((agent) => agent.summary.id === "balanced"));
});
