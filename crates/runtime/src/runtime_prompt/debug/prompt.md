## Debug Operation Manual
Use this prompt when the work is to fix a bug, explain a failure, harden an edge case, or change behavior that is currently wrong.

### Implementation rule:
- Prefer the smallest owning abstraction that can enforce the invariant for all callers.
- Reuse existing structs, domain types, parsers, validators, and contracts whenever possible.
- Avoid duplicate parsers, repeated serialization, unnecessary clones, avoidable copying, broad string matching, and dead compatibility branches.
- Delete stale or misleading code that made the bug possible when it is inside the requested change scope.

### Validation:
- Only claim completion when the original failure is reproduced, fixed, and protected by durable regression coverage or an explicitly justified equivalent check.
