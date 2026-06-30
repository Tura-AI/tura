## Debug Operation Manual
Use this prompt when the work is to fix a bug, explain a failure, harden an edge case, or change behavior that is currently wrong.

The failures agents miss are usually not "find the line and patch it" bugs. They are invariant, parser, state-transition, ordering, lifecycle, or domain-contract bugs where the visible traceback is only the last symptom. Treat every non-trivial bug as a small reverse-engineering task before editing.

### Required workflow:
- Do not start by patching the reported call site.
- First reproduce the bug with a script, failing test, fixture, CLI command, or smallest runnable example.
- Write down the observed failure, expected behavior, input shape, output shape, state before failure, and state after failure.
- Build a failure surface table before patching when the issue touches parsing, queries, migrations, printers, symbolic math, async/lifecycle code, serialization, cache/state, or cross-module data flow.
- The table must include the exact reproduction, neighboring valid cases, neighboring invalid cases, transformed/derived paths, caller variants, format variants, and any round-trip or ordering cases that matter.
- Use authoritative sources to build the table: existing tests, issue repro, failing stack, schema, parser grammar, command/API docs, source dispatchers, migration history, fixtures, logs, and live probes.
- Patch only after you know the earliest invariant boundary where the state becomes wrong.
- Trace the failure all the way back to the authoritative data source. ***Even if you think you have found the answer, confirm the complete chain from source data through every parser, dispatcher, transformer, cache, fallback, renderer, serializer, caller variant, and final externally visible behavior before deciding the root cause.***

### Reproduction rule:
- The reproduction must fail before the fix and pass after the fix.
- Prefer a focused regression test in the owning package. Use a one-off script only to discover the bug, then convert the discovery into a durable test when practical.
- If the bug depends on time, async scheduling, GUI redraw, process lifecycle, cache reuse, migration direction, database backend, locale, encoding, symlink/path shape, or import order, the reproduction must include that dimension.
- If the failure is nondeterministic, run it repeatedly or force the scheduler/state shape until the failure is observable.
- Do not accept a reproduction that only asserts the exact user-provided string. Assert the behavior contract.

### Invariant boundary rule:
- Work backward from the failure to the first point where a valid invariant becomes invalid.
- Do not stop at the first plausible local cause. Enumerate every producer of the wrong value or fallback, every transformation that can replace or override the value, and every caller/documenter/renderer path that can expose it; the root cause is not established until this chain is complete and competing branches are ruled out.
- Do not hide the symptom with fallback, dedupe, debounce, broad try/catch, extra coercion, or display filtering when the invariant belongs deeper.
- For parser/printer failures, fix grammar, tokenization, AST, precedence, escaping, or round-trip ownership rather than special-casing the printed text.
- For ORM/query/migration failures, fix the query state, alias/annotation/order/distinct/index contract, or migration state transition before adding output guards.
- For symbolic/math/algorithm failures, fix the mathematical domain, normalization, fallback algorithm, or exact/approx boundary. Do not patch only the example expression.
- For UI/lifecycle failures, fix ownership of event handlers, redraw, focus, async response, or process lifetime. Do not only suppress duplicate messages or disabled controls.
- For data model failures, preserve representation invariants across private state, caches, derived views, serialization, and round trips.

### Hard-case failure patterns:
- A table-like container is built from primary variables plus coordinate metadata. Rename, copy, merge, or deserialize can leave metadata pointing at a missing variable; the display still looks fine until selection, serialization, or equality uses the stale private state.
- A language parser accepts a numeric literal with a suffix. The bug may be the lexer splitting the suffix as an identifier, the AST losing the literal boundary, or the printer changing meaning around whitespace, delimiters, namespaces, and chained operators.
- A symbolic or statistical expression needs a cumulative result. Direct integration can hang, stay unevaluated, or produce a branch outside the support interval; the useful fix is domain support and fallback behavior, not the exact sample expression.
- A query filter receives an invalid null-test value such as a string, list, or expression object. If validation runs after planning starts, joins, aliases, annotations, or cached clauses may already be mutated before the error is raised.
- A block or tensor expression has uneven partitions. Element access near a partition boundary can return a value from the neighboring block when cumulative row or column offsets are composed in the wrong order.
- An assertion explanation rewrites `all(...)`, `any(...)`, a comprehension, or a chained comparison. The failure is often loss of the original expression, failing element, short-circuit behavior, or generator state during rewrite.
- A widget, streaming command, or async task stops receiving input after redraw, reconnect, cancel, or process restart. The visible callback can be correct while the live handler is still attached to an old owner, stale future, closed process, or unfocused surface.

### Implementation rule:
- Keep probes, failure tables, verifier logs, and issue research outside runtime implementation code.
- New code must implement the real invariant. It must not read verifier artifacts, bug IDs, issue text, or oracle observations to decide runtime behavior.
- Prefer the smallest owning abstraction that can enforce the invariant for all callers.
- Reuse existing structs, domain types, parsers, validators, and contracts whenever possible.
- Avoid duplicate parsers, repeated serialization, unnecessary clones, avoidable copying, broad string matching, and dead compatibility branches.
- Delete stale or misleading code that made the bug possible when it is inside the requested change scope.

### Drift rule:
- Stop immediately if the fix is only a guard at the visible traceback while earlier state is still wrong.
- Stop immediately if tests cover only the original reproduction while the discovered failure surface is larger.
- Stop immediately if the test is prompt-text, message-text, or snapshot-only when a structured behavior assertion is possible.
- Stop immediately if validation becomes help-only, import-only, format-only, or one-backend-only while the bug crosses more surfaces.
- When drift happens, return to reproduction and invariant discovery before patching more code.

### Validation:
- Run the focused regression test first.
- Run nearby owner tests that cover the same parser, query builder, migration operation, renderer, lifecycle path, or data model.
- Run formatting and lint/type checks when the edited surface normally requires them.
- Failed validation means unfinished work.
- Skipped validation means unfinished work unless the environment truly cannot run it; in that case explain the blocker precisely.
- Timed-out validation means unfinished work until the timeout is understood or scoped.
- Only claim completion when the original failure is reproduced, fixed, and protected by durable regression coverage or an explicitly justified equivalent check.
