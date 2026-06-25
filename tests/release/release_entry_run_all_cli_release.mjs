#!/usr/bin/env node
import { caseNames, runCliReleaseCase } from "./release_lib_release_entry_harness.mjs"

for (const caseName of caseNames) {
  await runCliReleaseCase(caseName)
}
