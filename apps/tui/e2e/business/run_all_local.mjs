#!/usr/bin/env node
import { runTuiLocalCase } from "./tui_local_business_harness.mjs";

for (const caseName of ["single-request", "snake", "password-zip"]) {
  await runTuiLocalCase(caseName);
}
