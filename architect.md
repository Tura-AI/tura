# Tura Refactor Startup Plan

## Purpose

This file is the startup document for broad Tura refactoring work. It exists so
future changes begin from explicit architecture boundaries, compatibility
checks, and user-visible invariants instead of ad hoc module rewrites.

## Current Scope

The active refactor scope is the `apps/gui` conversation experience and its
gateway-facing contracts. The work must preserve these boundaries:

- GUI talks to backend code only through `apps/gui/sdk/gateway`.
- Gateway owns sessions, messages, tools, file metadata, provider defaults,
  auth defaults, and runtime state.
- Shared visual rhythm lives in `apps/gui/app/src/styles/index.css` until a
  component package is introduced deliberately.
- Static GUI assets live under `apps/gui/app/public/assets`.

## Compatibility Invariants

Conversation layout must remain compatible across:

- desktop closed inspector
- desktop open inspector with resized detail panel
- medium width open inspector
- mobile closed conversation
- explorer file list metadata columns

The compatibility boundary is visual and behavioral:

- no horizontal overflow
- title uses page-left alignment
- conversation body and composer stay centered inside available space
- assistant avatar bottom aligns with assistant text bottom
- right inspector never covers conversation text
- inspector resize handle changes panel width after slide-in animation
- build, typecheck, and format checks pass

## Validation Entry Points

For the current GUI refactor, run:

```text
bun run format:check
bun run typecheck
bun run build
```

Then run the Playwright layout validation used by the current task and store
screenshots under `apps/gui/test-results/`.

## Refactor Rule

Do not rewrite the whole repository in one change. Expand from this file by
adding one bounded refactor area at a time, with its compatibility tests and
architecture notes updated before or together with code changes.
