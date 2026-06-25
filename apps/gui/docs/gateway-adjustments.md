# Gateway Adjustment Notes

Date: 2026-06-18

No gateway source changes were required for the GUI/Tauri test and type-layout cleanup.

The GUI SDK types now mirror the existing Rust contract domains under
`crates/gateway/src/contracts`, but they stay in `apps/gui/sdk/gateway/src/types`
and are exported through the same `@tura/gateway-sdk` entrypoint.

If gateway contracts change later, update the matching GUI SDK domain file first,
then add or adjust a focused unit test in `apps/gui/tests/unit/sdk/gateway`.
