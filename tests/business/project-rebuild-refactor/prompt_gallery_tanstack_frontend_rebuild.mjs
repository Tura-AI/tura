#!/usr/bin/env node
process.env.COMMAND_RUN_MAKEUP_TANSTACK_VERSION ||= "frontend"
await import("./prompt_gallery_tanstack_rebuild.mjs")
