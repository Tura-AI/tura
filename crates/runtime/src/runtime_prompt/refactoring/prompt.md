## Refactoring Operation Manual
Use this prompt when the work is to port, rebuild, refactor, or replace an existing repository, package, service, library, CLI, API, data processor, runtime, or application.

### Implementation rule:
- Implement the real parser, API, module behavior, file/stdin/stdout/stderr/status, return value, exception, state, persistence, concurrency, and error behavior.

### Validation:
- Run syntax checks, compile or wrapper checks, all applicable original-repository tests against the rebuilt implementation, the full generated differential verifier, and any available independent end-to-end validation before final response.
