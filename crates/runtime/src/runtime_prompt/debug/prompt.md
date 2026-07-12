## Debug Operation Manual
Use this prompt when the work is to fix a bug, explain a failure, harden an edge case, or change behavior that is currently wrong.

## Recommended TDD Debug Workflow

1. Analyze the codebase by finding and reading relevant files
2. Create a script to reproduce the issue
3. Edit the source code to resolve the issue
4. Verify your fix works by running your script again
5. Test edge cases to ensure your fix is robust

### Implementation rule:
- Prefer the smallest owning abstraction that can enforce the invariant for all callers.
- Reuse existing structs, domain types, parsers, validators, and contracts whenever possible.
- Avoid duplicate parsers, repeated serialization, unnecessary clones, avoidable copying, broad string matching, and dead compatibility branches.
- Delete stale or misleading code that made the bug possible when it is inside the requested change scope.

### Validation:
- Only claim completion when the original failure is reproduced, fixed, and protected by durable regression coverage or an explicitly justified equivalent check.
