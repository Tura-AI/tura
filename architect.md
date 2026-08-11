# Behavior-Test Refactor

## Objective

Remove tests that prove implementation text exists instead of proving Tura
works. Replace them with tests at executable boundaries: parsed schemas,
command routing, state transitions, persistence, process behavior, and rendered
frontend projections.

## Classification

A test is a removal candidate when success depends on reading production source,
prompt Markdown, or documentation and matching implementation words, function
names, branches, or punctuation. A fixed fixture is not a removal candidate by
itself: deterministic input is valid when it crosses a real parser, API, module,
process, or persistence boundary and asserts the observable result.

## Compatibility Boundaries

| Removed assertion style | Required replacement evidence |
| --- | --- |
| Prompt contains a sentence | Schema accepts/rejects the field and the owning handler changes state correctly |
| Source contains a function or branch | Public API or command exercises that branch and asserts output/error/state |
| Test name exists in another source file | Run the owning behavior test directly |
| Configuration file contains serialized words | Parse the file and assert the typed values or reload it through the API |
| Exact user input triggers a canned mock result | Drive a protocol fixture and assert calls, events, persistence, and final output |

## Backward-Compatibility Framework

The frozen runtime/session differential gate under
`tests/equivalence/runtime_session` remains the behavioral compatibility oracle.
This refactor adds a focused behavior-test manifest under
`tests/equivalence/test_quality` that maps every removed source-text assertion to
its executable replacement command. The manifest is evidence, not a keyword
gate: completion requires every command to pass plus the full runtime/session
equivalence gate.

## Invariants

- Provider, runtime, gateway, tool, GUI, and TUI wire behavior must not change.
- Prompt wording is not an API unless a parser consumes it as structured data.
- Error tests assert typed errors or stable status/category data where available,
  not incidental prose.
- Tests may inspect files produced by behavior, but not production source to
  infer that behavior exists.
- No replacement may duplicate a production parser inside the test.

## Validation

Run formatting and syntax checks, focused replacement tests, backend business
tests, frontend unit/type checks for touched apps, the test-quality behavior
manifest, and `tests/equivalence/runtime_session/run.ps1 -Mode gate`.
