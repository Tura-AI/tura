import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "./reducer.js";
import { render } from "./render.js";

test("render includes core TUI panels without throwing", () => {
  const session = { id: "sess-1", title: "Work", directory: "C:/repo", status: "idle" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-1",
        sessionID: "sess-1",
        role: "assistant",
        parts: [
          { id: "part-1", type: "text", text: "Ready" },
          { id: "tool-1", type: "tool", tool: "runtime", state: { status: "completed", output: { text: "checked" } } },
        ],
      },
    ],
    todos: [{ id: "todo-1", content: "Verify", status: "in_progress" }],
    permissions: [{ id: "perm-1", sessionID: "sess-1", permission: "shell" }],
    providers: {
      all: [{ id: "openai", name: "OpenAI", models: { "gpt-5.5": { id: "gpt-5.5", name: "gpt-5.5" } } }],
      default: { openai: "gpt-5.5" },
      connected: ["openai"],
    },
    sessions: [session],
  });
  state = reducer(state, { type: "questions", value: [{ id: "q-1", sessionID: "sess-1", question: "Proceed?" }] });

  const transcript = render(state);
  assert.match(transcript, /Tura/);
  assert.match(transcript, /assistant/);
  assert.match(transcript, /runtime completed/);
  assert.match(transcript, /permission/);
  assert.match(transcript, /question/);

  state = reducer(state, { type: "toggle-models" });
  assert.match(render(state), /openai\/gpt-5\.5/);
});
