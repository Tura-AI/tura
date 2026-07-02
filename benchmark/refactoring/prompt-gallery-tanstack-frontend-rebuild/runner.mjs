#!/usr/bin/env node
process.env.COMMAND_RUN_MAKEUP_TANSTACK_VERSION = "frontend"
await import("../prompt-gallery-tanstack-rebuild/runner.mjs")
