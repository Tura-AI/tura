#!/usr/bin/env node
import { runGuiReleaseCase } from "../../../../tests/release/release_lib_release_entry_harness.mjs"

await runGuiReleaseCase("password-zip")
