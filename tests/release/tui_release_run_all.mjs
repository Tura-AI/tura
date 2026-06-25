#!/usr/bin/env node

for (const script of [
  "./tui_release_single_request.mjs",
  "./tui_release_snake_acceptance.mjs",
  "./tui_release_password_zip_playwright.mjs",
]) {
  await import(script)
}
