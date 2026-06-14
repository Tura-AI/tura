#!/usr/bin/env node
import { runGuiReleaseCase } from "../release/release_lib_release_entry_harness.mjs"

for (const caseName of ["single-request", "snake", "password-zip"]) {
  await runGuiReleaseCase(caseName)
}
