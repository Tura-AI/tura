import assert from "node:assert/strict";
import test from "node:test";
import { setLanguage } from "../../../src/i18n.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { initialState, reducer, type AppState } from "../../../src/tui/reducer.js";
import { setActiveCapabilities, stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  settingOptions,
  settingsEntries,
  settingsLines,
} from "../../../src/tui/render/settings.js";
import { applyAboutAction } from "../../../src/tui/settings-actions.js";
import type { TuiGatewayClient } from "../../../src/tui/runtime.js";

test("About is the final settings entry and uses the existing selection component", () => {
  setLanguage("en");
  setActiveCapabilities(richCapabilities());
  const state = aboutState();
  const entries = settingsEntries(state);

  assert.equal(entries.at(-1)?.detail, "about");
  assert.equal(entries.at(-1)?.value, "0.1.30");
  assert.deepEqual(
    settingOptions(state).map(([label, _description, value]) => [label, value]),
    [
      ["Add star", "addStar"],
      ["Report bug", "reportBug"],
      ["Contribute", "contribute"],
      ["Update", "update"],
      ["Contact", "contact"],
    ],
  );
  const rendered = stripAnsi(settingsLines(state, 100, 6).join("\n"));
  assert.match(rendered, /Release version\s+0\.1\.30/u);
  assert.match(rendered, /system\s+Windows 11 \(x86_64\)/u);
  assert.match(rendered, />\s+Add star/u);
});

test("About actions call only the shared Gateway client", async () => {
  let state = aboutState();
  const calls: unknown[] = [];
  const client = {
    starTuraRepository: async () => {
      calls.push(["star"]);
      return { outcome: "starred" as const };
    },
    openAboutTarget: async (target: string) => {
      calls.push(["open", target]);
      return { opened: true, target };
    },
    checkTuraUpdate: async () => {
      calls.push(["check"]);
      return { update: { current_version: "0.1.30", latest_version: "0.1.31" } };
    },
    installTuraUpdate: async (version: string, sessionID?: string) => {
      calls.push(["install", version, sessionID]);
      return { scheduled: true, version };
    },
  } as TuiGatewayClient;
  const dispatch = (action: Parameters<typeof reducer>[1]) => {
    state = reducer(state, action);
  };

  await applyAboutAction(client, () => state, dispatch, "addStar");
  await applyAboutAction(client, () => state, dispatch, "reportBug");
  await applyAboutAction(client, () => state, dispatch, "contribute");
  await applyAboutAction(client, () => state, dispatch, "contact");
  await applyAboutAction(client, () => state, dispatch, "update");

  assert.deepEqual(calls, [
    ["star"],
    ["open", "report_bug"],
    ["open", "contribute"],
    ["open", "contact"],
    ["check"],
  ]);
  assert.deepEqual(state.aboutUpdate, {
    current_version: "0.1.30",
    latest_version: "0.1.31",
  });

  let exited = false;
  await applyAboutAction(
    client,
    () => state,
    dispatch,
    "confirmUpdate",
    () => {
      exited = true;
    },
  );
  assert.deepEqual(calls.at(-1), ["install", "0.1.31", "session-1"]);
  assert.equal(state.aboutUpdate, undefined);
  assert.equal(exited, true);
});

test("About update confirmation warns that the session will be interrupted", () => {
  setLanguage("en");
  const state: AppState = {
    ...aboutState(),
    aboutUpdate: { current_version: "0.1.30", latest_version: "0.1.31" },
  };

  assert.deepEqual(
    settingOptions(state).map(([_label, _description, value]) => value),
    ["confirmUpdate", "cancelUpdate"],
  );
  const rendered = stripAnsi(settingsLines(state, 100, 20).join("\n"));
  assert.match(rendered, /0\.1\.30.*0\.1\.31/u);
  assert.match(rendered, /session will be interrupted/iu);
});

function aboutState(): AppState {
  return {
    ...initialState("C:/repo"),
    settingsOpen: true,
    settingDetail: "about",
    session: { id: "session-1" } as AppState["session"],
    sessionConfig: {},
    aboutInfo: {
      release_version: "0.1.30",
      system: {
        operating_system: "Windows",
        os_version: "11",
        architecture: "x86_64",
      },
    },
  };
}
