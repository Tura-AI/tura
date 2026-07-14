import assert from "node:assert/strict";
import test from "node:test";
import { pickInitialSession } from "../../../src/tui/runtime.js";
import type { Session } from "../../../src/types/session.js";
import { sessionSortAt } from "../../../src/types/session.js";

test("pickInitialSession asks gateway for child sessions before choosing latest sidebar timestamp session", async () => {
  const calls: unknown[] = [];
  const root = session("sess-root", { created_at: 10, task_start_at: 10, updated_at: 999 });
  const fork = session("sess-fork", {
    parent_id: root.id,
    created_at: 20,
    task_start_at: 20,
    updated_at: 20,
  });
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

  assert.equal(picked.id, "sess-root");
  assert.deepEqual(calls, [{ includeChildren: true, limit: 20 }]);
});

test("pickInitialSession uses an explicit initial session id before default sorting", async () => {
  const calls: unknown[] = [];
  const explicit = session("sess-explicit", { created_at: 1 });
  const latest = session("sess-latest", { created_at: 999 });
  const client = {
    async getSession(sessionID: string): Promise<Session> {
      calls.push(["getSession", sessionID]);
      return explicit;
    },
    async listSessions(options: unknown): Promise<Session[]> {
      calls.push(["listSessions", options]);
      return [latest, explicit];
    },
    async createSession(): Promise<Session> {
      calls.push(["createSession"]);
      return session("sess-created");
    },
  };

  const picked = await pickInitialSession(client as never, "C:/repo", explicit.id);

  assert.equal(picked.id, explicit.id);
  assert.deepEqual(calls, [["getSession", explicit.id]]);
});

test("sessionSortAt uses only the latest user message timestamp", () => {
  assert.equal(sessionSortAt(session("user", { last_user_message_at: 20, updated_at: 999 })), 20);
  assert.equal(sessionSortAt(session("updated", { updated_at: 999 })), 0);
  assert.equal(sessionSortAt(session("created", { created_at: 10, updated_at: undefined })), 0);
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
