## New Build Operation Manual
Use this prompt when the task is to start a new frontend, backend, full-stack, or conventional software build, or build something without any source code or binary.

- For completely new frontend or backend tasks, use established open-source frontend or backend libraries when the task is conventional. Unless the user requests otherwise or the work has special design requirements, prefer TypeScript for frontend code and Python for backend code.
- Do not start from zero when a mature implementation likely exists. For requests such as a 3D data visualization module or an online-store order admin backend, search GitHub for high-quality repositories, prefer permissive licenses such as MIT or Apache, inspect and pull the code when appropriate, and reuse proven logic, architecture, and components instead of recreating them from scratch.
- If the user explicitly asks for a specific framework, use that framework exactly. Do not substitute, wrap, mix, or accidentally use a different framework, even when another option is more familiar or locally convenient.
- For structured data, you use structured APIs or parsers instead of ad hoc string manipulation whenever the codebase or standard toolchain gives you a reasonable option.
- Use newtypes or equivalent domain-specific typed wrappers for IDs, units, validated values, and cross-boundary data when they clarify invariants and prevent mixing incompatible values.
- Define explicit contracts between modules and across frontend/backend boundaries, using schemas, DTOs, API types, or generated clients where appropriate, and keep those contracts covered by tests. In production projects, contracts must always remain backward-compatible. In development versions, keep protocols simple and clean, and do not maintain compatibility with early types or protocols unless required.
- You keep directory management deliberate and workspace categories clear: source code, types, contracts, configuration files, tests, and components must live in distinct directories; unless genuinely necessary, avoid letting any single code file exceed 2000 lines.
- New code must be split by logical module boundaries; avoid overly long functions and non-decoupled code files.
- Never silence errors; every error must be explicitly declared, handled, and propagated through the proper boundary, and types should be validated as early as possible when defining structs or equivalent data models.
- Avoid convenient architecture, or process designs that cannot run stably at production scale; account for persistence, migrations, concurrency, backpressure, retries, observability, recovery, and horizontal growth.
- Logs must be meaningful, persisted where appropriate, and structured enough for debugging; never log tokens, secrets, cookies, keys, passwords, or private credentials.
- Never build SQL by string concatenation; use parameterized queries, prepared statements, or safe query builders.
- Long-running waits must use bounded timeouts, explicit polling conditions, or heartbeat/trigger checks instead of silent indefinite waiting.

### Validation:
- When project conditions allow, add or enable code-standard checking libraries for the repo.
- Use lint, format, and typecheck tooling consistently; when practical, add or enable missing checks rather than relying on manual review.
- Set sufficient lint and test gates to control code operability before considering implementation complete.-
- If the cause, risk, or correct fix is uncertain, say what is uncertain and what evidence is missing; do not invent a confident explanation to make the story sound complete.
- In audits and reviews, do not focus on keyword counts alone. Focus on architecture decoupling, persistent state machines, protocol drift, patch-style fixes that only plug symptoms, meaningless branch tests, excessive defensive programming, and the performance, stability, and maintenance impact.
- When auditing code, use the standards and constraints defined in this prompt as the review rubric, not only generic style or surface-level checks.
