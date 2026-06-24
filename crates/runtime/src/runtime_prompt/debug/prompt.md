## Debug Operation Manual
Use this prompt when the task is to fixing bugs or adding/changing feature.
***When debugging failures, always reproduce the bug with a script or automated test, then work backward from the failure to the earliest invariant boundary before patching. Do not chase only the visible trigger; think through derived/transformed paths so stale references, cached state, and shape mismatches cannot hide.***
- When the concrete failure is in a generated artifact or external runtime error, prioritize the generator/output-shape contract over upstream object-lifetime explanations unless the generator contract is already proven intact.
- Do not mask the failure at the reported call site when the invariant belongs deeper in the system.
- Communication protocols and paths between modules must be explicit, unified, and versionable; do not create ad hoc routes, events, files, or message shapes for each module.
- Reuse existing structs, domain types, and contracts whenever possible; avoid duplicate parsers, repeated serialization, unnecessary clones, and avoidable copying that harm performance and consistency.
- Prefer fewer components, small modules, and clear ownership boundaries. For binary compilation, use the default build cache unless there is a specific reason to disable it.

### Reverse-thinking workflow example:
- If a frontend view refreshes or repeats the same information twice, do not first add frontend fallback, deduping, debounce, or display guards to hide the symptom. Work backward through the information flow to find the earliest source of duplication. For example, if the runtime service receives remote information, writes one update to the database during streaming, then writes another update when finalizing the permanent state, the invariant belongs in the runtime service's streaming/finalization persistence contract. Fix the source so the database and state transition emit the intended single logical update, then verify the frontend receives the correct stream naturally.

## Validation:
- After fixing, validate with test assertions: the reproduction script or test must fail on the original bug, pass after the fix, and cover equivalent callers or nearby paths, not only the exact reproduction.
- Test the full operation sequence, including simulated delays, time-dependent behavior, format conversions, and other transformations when relevant, not only the step that visibly failed.
- As part of verification, actively delete code and files that the user has deemed useless or wrong, and keep directories tidy. If you are unsure whether something should be removed, ask the user before deleting it.
