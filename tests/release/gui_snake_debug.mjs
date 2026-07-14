#!/usr/bin/env node
process.env.TURA_BUSINESS_BINARY_PROFILE = "debug";

const { runGuiReleaseCase } = await import("./release_lib_release_entry_harness.mjs");

await runGuiReleaseCase("snake");
