## New Build Operation Manual
Use this prompt when the task is to start a new frontend, backend, full-stack project, or build software without any source code or binary.

### Expectations:
- Use established open-source libraries for conventional frontend, backend, full-stack, data, media, auth, database, queue, and infrastructure work. Do not hand-roll common production components without a strong reason.
- Prefer TypeScript for frontend code and Python for backend code unless the user requests another stack or the project already requires one.
- If the user names a framework, use that framework exactly. Do not substitute, wrap, mix, or quietly replace it with a familiar alternative.
- Before building from scratch, check whether a mature implementation, library, or permissively licensed reference project exists. Inspect the license and reuse proven architecture or components when appropriate.
- Use structured parsers, schema validators, generated clients, DTOs, or typed APIs for structured data. Do not parse structured formats with ad hoc string manipulation when a proper tool is available.
- Define explicit contracts at every module and frontend/backend boundary. Cover those contracts with tests or type checks.
- Use domain-specific typed wrappers or equivalent validation for IDs, units, validated values, and cross-boundary data when they prevent mistakes.
- Keep the project layout clean: source code, shared types, contracts, configuration, tests, components, fixtures, and generated assets must live in clear directories.
- Split new code by real module boundaries. Avoid oversized files, oversized functions, hidden global state, and tightly coupled implementation files.
- Never silence errors. Declare, validate, handle, and propagate errors through the correct boundary.
- Design for production stability when the task is more than a toy: persistence, migrations, concurrency, backpressure, retries, observability, recovery, and horizontal growth must be considered.
- Logs must be useful and structured. Never log tokens, secrets, cookies, keys, passwords, or private credentials.
- Never build SQL with string concatenation. Use parameterized queries, prepared statements, or safe query builders.
- Long-running waits must use bounded timeouts, explicit polling, or heartbeat/trigger checks. Do not wait silently forever.

### Slop guardrails:
- Build real user flows, not a decorative shell. Every visible command, route, API endpoint, setting, and navigation item must either work, be intentionally disabled with clear state, or be removed.
- Do not invent integrations, metrics, permissions, automations, saved state, or "AI-powered" behavior to make the build look more complete. Use mocks only when the user asked for a prototype, and label their boundary in code.
- Avoid placeholder content that hides missing behavior: generic cards, fake dashboards, unused sample data, dead buttons, empty tabs, copied boilerplate, broad TODOs, and success messages that are not backed by state changes.
- Prefer a small maintainable implementation over architecture theater. Do not add queues, caches, event buses, adapters, global stores, plugin systems, or config layers unless a current requirement needs them.
- Before calling the build done, remove unused files, imports, variables, parameters, routes, fixtures, generated assets, and one-off helpers that no longer serve the implemented behavior.

### Validation:
- Use lint, format, and typecheck tooling consistently; when practical, add or enable missing checks rather than relying on manual review.
