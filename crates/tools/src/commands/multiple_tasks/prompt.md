`multiple_tasks` declares the agent task topology, task-management state machine, and delegation plan for complex work: the runtime/gateway may keep dispatching planned tasks to other agents or child sessions, with inherited workspace directory and relevant context, until delegated work is reported back and the plan reaches a terminal state.

Use `multiple_tasks` only for the most complex 10% of requests, and only when the user explicitly asks for planning, production-project creation, major refactor planning, or multiple independent goal tasks with separate deliverables. Call it only right after receiving that request, or after reading code and documentation reveals the task's true complexity, and before execution starts. Do not call it in the middle of execution.

Do not use `multiple_tasks` for a single goal that merely needs several execution steps such as inspect, edit, test, and summarize. Multi-step execution inside one goal is not multiple tasks. If the request is one objective, even a hard one, `multiple_tasks` is forbidden.

For production-project creation or major refactor planning, use `multiple_tasks` only when the user asks for that level of planning. First clarify goal and scope without assuming business requirements, tech stack, or implementation style. Then plan module boundaries, framework choices, state management or state machines, service structure, workflow system, interface contracts, data flow, validation standards, local module requirements, and whole-business requirements. Break the work into small ordered tasks and include for each task: goal, affected files or directories, expected behavior, validation method, dependency on other tasks, risks, and rollback or alternative path. Implementation should proceed by dependency stage: create or update files/directories, connect interfaces/contracts, validate locally, then continue after the relevant checks pass. Independent small modules may share the same `step` so they can be developed and tested in parallel before module integration, but only when their file ownership, contracts, validation methods, and rollback paths are clear. Integration, cross-module contract changes, and e2e validation must be later barrier steps after their dependencies pass. During late debug and test-fix phases, independent failing areas may also share a parallel step for focused fixes and targeted validation, followed by a later whole-project integration or e2e check.

If a planning state machine already exists, do not update it unless the user clearly changed the task. Runtime will reject mid-execution planning updates.

The `command_line` value must be a JSON array. Each item must have:
- `nonce_id`: stable unique task nonce. This is the primary key.

Each item may also include:
- `step`: non-negative integer step number.
- `task_summary`: one short sentence, about 10 words or fewer.
- `delivery`: concrete files or locations to work on, plus the verification standard for that task.

Good use cases:
- User says: "Plan and build a production SaaS from scratch." Use independent tasks for architecture/contracts, data model/server, frontend workflow, validation/e2e.
- User says: "Refactor auth, billing, and workspace isolation as separate deliverables." Use one task per independent subsystem; if the corresponding modules live in different workspaces with clear contracts and validation, they may share the same `step` for parallel development.
- User says: "Break this migration into ordered production tasks with validation and rollback." Use tasks ordered by dependency.
- User says: "Parallelize small independent modules before integration." Use the same step for isolated modules and later steps for contract wiring and e2e.

Do not use cases:
- User says: "Fix this bug and run tests." This is one goal; use normal command_run steps.
- User says: "Inspect, edit, test, summarize." These are execution phases, not independent tasks.
- User says: "Implement one endpoint." This is one goal even if it needs multiple files.

Example `command_line`:
[
  {"nonce_id":"architecture-contracts","step":0,"task_summary":"Read architecture docs","delivery":"Go to docs/architecture and module docs; read ARCHITECTURE.md, REQUIREMENTS.md, ACCEPTANCE.md, and module contract md files. Extract module boundaries, state-machine/service contracts, validation standards, dependencies, risks, and rollback paths before implementation."},
  {"nonce_id":"server-state","step":1,"task_summary":"Implement server module","delivery":"Go to services/server or the documented server workspace; read its README.md, MODULE.md, CONTRACTS.md, and ACCEPTANCE.md. Implement all documented server/state-machine requirements and local tests according to those acceptance standards. Parallel-safe with frontend-flow only if contracts are frozen and files/workspaces do not overlap."},
  {"nonce_id":"frontend-flow","step":1,"task_summary":"Implement frontend module","delivery":"Go to apps/frontend or the documented frontend workspace; read its README.md, MODULE.md, UX_FLOW.md, CONTRACTS.md, and ACCEPTANCE.md. Implement all documented UI flows, state handling, contract usage, and local tests/screenshots according to those acceptance standards. Parallel-safe with server-state only before integration wiring."},
  {"nonce_id":"integration-wiring","step":2,"task_summary":"Wire module contracts","delivery":"Go to docs/integration and the affected module workspaces; read INTEGRATION.md, API_CONTRACTS.md, STATE_MACHINE.md, and ACCEPTANCE.md. Connect server/frontend contracts, resolve interface mismatches, and validate all documented integration requirements."},
  {"nonce_id":"debug-failing-areas","step":3,"task_summary":"Fix focused failures","delivery":"Go to docs/testing and each failing module workspace; read TEST_PLAN.md, DEBUG_GUIDE.md, known-failures md files, and module ACCEPTANCE.md. In parallel where independent, fix isolated failing areas and rerun each documented targeted validation until the module standard passes."},
  {"nonce_id":"integration-e2e","step":4,"task_summary":"Run e2e acceptance","delivery":"Go to docs/acceptance or the documented e2e workspace; read E2E.md, ACCEPTANCE.md, RELEASE_CHECKLIST.md, and the overall architecture file. Implement any missing tests, run the full documented e2e/business-flow acceptance suite, fix regressions, and report remaining gaps only if the documented standard cannot be met."}
]
