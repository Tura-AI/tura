#!/usr/bin/env node
import { runTuiReleaseCase } from "../release/release_lib_release_entry_harness.mjs"

await runTuiReleaseCase("single-request")
