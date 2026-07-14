#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";

const here = import.meta.dirname;
const appRoot = path.resolve(here, "..", "..", "..");
const scripts = [
  "tui_mock_gateway_stream_flow.mjs",
  "tui_multi_session_playwright.mjs",
  "tui_refresh_replay_playwright.mjs",
  "tui_real_session_db_replay_playwright.mjs",
];

for (const script of scripts) {
  console.log(`=== business ${script} ===`);
  const result = spawnSync(process.execPath, [path.join(here, script)], {
    cwd: appRoot,
    env: process.env,
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
  if (result.signal) {
    console.error(`${script} terminated by ${result.signal}`);
    process.exit(1);
  }
}
