# Scripts

Scripts only build and register the two standard Cargo output directories:

- `target/debug`
- `target/release`

Command entries:

- `tura_exec`: Rust one-shot CLI binary.
- `tura`: compiled terminal entry. Use `tura` for the TUI, `tura run "prompt"` for the TUI gateway client, or `tura exec "prompt"` for the Rust CLI front.
- `tura_gateway`, `tura_router`, `tura_session_db`, `tura_runtime`: backend services.

Important scripts:

- `install.*`: install dependencies only. The root installer checks
  `shell_command`, `bash`, `zsh`, and `git` coverage on every platform, ensures
  user-local `uv`, Python 3.12 through `uv`, and `bun`, calls command-owned
  `commands/*/install.*` scripts, and runs Bun installs inside app/package
  directories. `--skip-uv`/`-SkipUv` requires command installers to be skipped,
  and `--skip-bun`/`-SkipBun` requires app installs to be skipped when Bun
  workspaces are present. It does not build binaries or register PATH launchers.
  Windows adds common Git/MSYS shell paths before checking bash/zsh. macOS
  asserts zsh and bash and reports optional PowerShell (`pwsh`) coverage.
- `build-debug.*`: build Rust debug binaries and the TUI entry into `target/debug`.
- `build-release.*`: build Rust release binaries, the web GUI dist, the TUI entry,
  and the Tauri desktop bundle. CLI/TUI artifacts and copied web assets land in
  `target/release`; Tauri bundle artifacts are produced by the Tauri CLI under
  the release target bundle directory. Release builds preserve local session
  DB/cache state by default; pass `-Clean` on PowerShell or `-clean`/`--clean` on
  POSIX shells when a build must intentionally remove repository-local session
  DB/cache files first. Pass `-BackendOnly` or `--backend-only` when a CI job only
  needs Rust release artifacts.
- `register-cli.*`: add `target/release` to the user PATH. No wrapper directory is created; the registered CLI command is `tura exec`. The POSIX script updates `.profile`, `.bash_profile`, `.bashrc`, `.zprofile`, and `.zshrc` when present, and creates `.zprofile`/`.zshrc` on macOS so new Terminal sessions work.
- `unregister-cli.*`: remove `target/release` from PATH and delete a stale `cli-bin` directory if present.
- `start.*`: convenience runner for `target/debug` by default, or `target/release` with `--release`. The runner repeats the same shell coverage checks before launching; set `TURA_STRICT_SHELL_TOOL_COVERAGE=1` when optional zsh/PowerShell gaps should fail the run.
- `check-backend-quality.*`: CI smell gate. It runs backend Rust test-layout
  policy, Rust formatting, TUI formatting, Rust dependency policy, and spelling.
  It intentionally does not run `cargo test --workspace`; crate tests are owned
  by `xtask/scripts/run-ci-crate-tests.*`.
- `run-ci.*`: local CI orchestrator. It runs `check-backend-quality.*` first,
  then monitors crate tests, backend business tests, and TUI business tests in
  parallel.
- `run-release-dry-run.*`: release dry-run orchestrator. It runs install, the CI
  flow, and release artifact build without publishing.

Xtask test collection scripts:

- `xtask/scripts/run-ci-crate-tests.*`: GitHub-style crate matrix runner. It
  discovers default backend workspace packages, excludes `tura_gui`, and runs
  clippy plus `cargo test -p <crate>` for each crate. Local runs can batch
  crates in parallel.
- Typed Rust test directories are peers: `tests/business`, `tests/os_testing`,
  `tests/performance`, `tests/live`, `tests/release`, and `tests/benchmark`.
  Business and OS testing may use `helpers/` plus target-owned module
  directories beside the top-level entrypoint; other crate-owned typed
  directories stay flat. Do not keep empty typed directories. The workspace root `tests/benchmark` is the manual benchmark
  exception and keeps historical second-level categories such as `bug-fix`,
  `frontend-playwright`, `project-rebuild-refactor`, and `tui`.
- Typed test runners discover cases by scanning the matching directory type.
  Do not add one-off hardcoded script paths when a directory scan can find the
  case.
- `xtask/scripts/run-backend-business-tests.*`: run root Rust business tests plus
  crate-owned Rust tests from `crates/*/tests/business`, `commands/*/tests/business`,
  `agents/*/tests/business`, and `personas/*/tests/business` using one-level
  typed-directory scans. Business targets run in parallel batches and the
  runner reports all failed `package::target` entries after the discovered set
  finishes. Process, daemon, service-owner, lifecycle, and OS policy coverage
  belongs to `xtask/scripts/run-backend-os-tests.*`. These backend runners do
  not execute `.mjs` app, TUI, or GUI scripts; run app suites from `apps/tui` or
  `apps/gui`.
- `xtask/scripts/run-backend-os-tests.*`: run root and crate-owned Rust tests from
  `tests/os_testing` with the `os-tests` feature gate. Every target runs
  serially with `--test-threads=1` to avoid process-global env, local socket,
  owner-lock, daemon, and child-process cleanup conflicts.
- `xtask/scripts/run-backend-live-tests.*`: run opt-in root/backend Rust live tests and
  backend-owned root live scripts using one-level typed-directory scans and the
  `live-tests` feature gate when the package declares it. These backend
  runners do not execute app-owned TUI/GUI scripts; run those from the app
  package commands.
- `xtask/scripts/run-backend-release-tests.*`: run opt-in backend release-binary
  tests discovered from root `tests/release/*.mjs`. TUI/GUI release entrypoints
  also live in `tests/release`, but the backend runner skips `tui_*` and
  `gui_*`; run those directly or through the app package aliases.
- `xtask/scripts/run-backend-performance-tests.*`: runner for crate-owned Rust
  performance tests from `crates/*/tests/performance`; each target is killed if
  it exceeds the configured timeout.

Script tests:

- `tests/scripts/test-install.*`: checks script syntax where available, runs the
  root dependency installer, and verifies command-owned Python environments.
- `tests/scripts/test-build-release.*`: validates a dry-run release probe such as
  `release-v0.0.0-ci`, runs `build-release.*`, checks expected artifacts, and
  verifies command protocol health. Pass `-BackendOnly` or `--backend-only` when
  a CI job only needs Rust release artifacts.

GitHub Actions:

- `.github/workflows/ci.yml` runs the smell gate first. After that, crate matrix
  jobs, backend business tests, and TUI business tests run in parallel with
  Cargo and npm caches. Tags starting with `release` trigger a release dry-run
  job after CI completes; the job builds release artifacts and does not publish
  a GitHub release.

Wrapper/package routes are intentionally not used; release commands resolve directly from `target/release`.
