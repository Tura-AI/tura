import assert from "node:assert/strict";
import test from "node:test";
import { handleTuiKeypress } from "../../../src/tui/app.js";
import { initialState, reducer, type AppAction, type AppState } from "../../../src/tui/reducer.js";
import { providerEnums } from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

function stateHarness(initial: AppState = initialState("C:/repo")) {
  let state = initial;
  return {
    getState: () => state,
    dispatch: (action: AppAction) => {
      state = reducer(state, action);
    },
  };
}

async function press(
  harness: ReturnType<typeof stateHarness>,
  text: string,
  key: Parameters<typeof handleTuiKeypress>[4],
): Promise<void> {
  await handleTuiKeypress({} as never, harness.getState, harness.dispatch, text, key);
}

test("Tab completes the selected slash command and arrows navigate suggestions", async () => {
  const harness = stateHarness();
  await press(harness, "/mo", { sequence: "/mo" });
  assert.equal(harness.getState().composer, "/mo");

  await press(harness, "", { name: "down", sequence: "\x1b[B" });
  assert.equal(harness.getState().selectedCompletionIndex, 1);

  await press(harness, "", { name: "tab", sequence: "\t" });
  assert.equal(harness.getState().composer, "/models ");
  assert.equal(harness.getState().composerCursor, "/models ".length);
});

test("Enter accepts a partial command before executing it", async () => {
  const harness = stateHarness();
  await press(harness, "/set", { sequence: "/set" });
  await press(harness, "", { name: "return" });
  assert.equal(harness.getState().composer, "/settings ");
});

test("slash commands remain available while a non-settings panel is open", async () => {
  const harness = stateHarness({ ...initialState("C:/repo"), help: true });
  await press(harness, "/chat", { sequence: "/chat" });
  assert.equal(harness.getState().composer, "/chat");

  await press(harness, "", { name: "return" });
  assert.equal(harness.getState().help, false);
  assert.equal(harness.getState().composer, "");
});

test("composer editing moves and changes text at the cursor", async () => {
  const harness = stateHarness();
  await press(harness, "hello", { sequence: "hello" });
  await press(harness, "", { name: "left", sequence: "\x1b[D" });
  await press(harness, "", { name: "left", sequence: "\x1b[D" });
  await press(harness, "X", { sequence: "X" });
  assert.equal(harness.getState().composer, "helXlo");
  assert.equal(harness.getState().composerCursor, 4);

  await press(harness, "", { name: "backspace" });
  assert.equal(harness.getState().composer, "hello");
  assert.equal(harness.getState().composerCursor, 3);

  await press(harness, "", { name: "a", ctrl: true });
  assert.equal(harness.getState().composerCursor, 0);
  await press(harness, "", { name: "e", ctrl: true });
  assert.equal(harness.getState().composerCursor, 5);
});

test("Home, End, and PageDown navigate long menus", async () => {
  const models = Object.fromEntries(
    Array.from({ length: 70 }, (_item, index) => [
      `model-${index}`,
      { id: `model-${index}`, name: `Model ${index}` },
    ]),
  );
  const harness = stateHarness({
    ...initialState("C:/repo"),
    modelsOpen: true,
    providers: {
      all: [{ id: "provider", name: "Provider", source: "test", models }],
      default: {},
      connected: [],
      enums: providerEnums,
    },
  });

  await press(harness, "", { name: "end" });
  assert.equal(harness.getState().selectedModelIndex, 69);
  await press(harness, "", { name: "home" });
  assert.equal(harness.getState().selectedModelIndex, 0);
  await press(harness, "", { name: "pagedown", sequence: "\x1b[6~" });
  assert.ok(harness.getState().selectedModelIndex > 0);
});
