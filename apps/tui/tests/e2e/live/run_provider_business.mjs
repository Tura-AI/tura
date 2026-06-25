#!/usr/bin/env node
import { caseNames, runTuiLocalCase } from "./tui_provider_business_harness.mjs";

for (const caseName of caseNames) {
  await runTuiLocalCase(caseName);
}
