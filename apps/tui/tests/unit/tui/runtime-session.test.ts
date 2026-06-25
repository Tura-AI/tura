import assert from "node:assert/strict";
import test from "node:test";
import { pickInitialSession } from "../../../src/tui/runtime.js";
import type { Session } from "../../../src/types/session.js";

test("pickInitialSession asks gateway for child sessions before choosing latest session", async () => {
  const calls: unknown[] = [];
  const root = session("sess-root", { updated_at: 10 });
  const fork = session("sess-fork", { parent_id: root.id, updated_at: 20 });
  const client = {
    async listSessions(options: unknown): Promise<Session[]> {
      calls.push(options);
      return [root, fork];
    },
    async createSession(): Promise<Session> {
      return session("sess-created");
    },
  };

  const picked = await pickInitialSession(client as never, "C:/repo");

  assert.equal(picked.id, "sess-fork");
  assert.deepEqual(calls, [{ includeChildren: true, limit: 20 }]);
});

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
