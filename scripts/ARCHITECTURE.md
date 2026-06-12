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
  `shell_command`, `bash`, and `zsh` coverage on every platform, ensures
  user-local `uv` and `bun`, calls command-owned `commands/*/install.*`
  scripts, and runs Bun installs inside app/package directories. It does not
  build binaries or register PATH launchers. Windows adds common Git/MSYS shell
  paths before checking bash/zsh. macOS asserts zsh and bash and reports
  optional PowerShell (`pwsh`) coverage.
- `build-debug.*`: build Rust debug binaries and the TUI entry into `target/debug`.
- `build-release.*`: build Rust release binaries and the TUI entry into `target/release`.
- `register-cli.*`: add `target/release` to the user PATH. No wrapper directory is created; the registered CLI command is `tura exec`. The POSIX script updates `.profile`, `.bash_profile`, `.bashrc`, `.zprofile`, and `.zshrc` when present, and creates `.zprofile`/`.zshrc` on macOS so new Terminal sessions work.
- `unregister-cli.*`: remove `target/release` from PATH and delete a stale `cli-bin` directory if present.
- `start.*`: convenience runner for `target/debug` by default, or `target/release` with `--release`. The runner repeats the same shell coverage checks before launching; set `TURA_STRICT_SHELL_TOOL_COVERAGE=1` when optional zsh/PowerShell gaps should fail the run.
- Typed Rust test directories are peers: `tests/business`, `tests/performance`,
  `tests/live`, `tests/release`, and `tests/benchmark`. Keep files directly under the typed
  directory, encode categories in filenames, and do not keep empty typed
  directories. The workspace root `tests/benchmark` is the manual benchmark
  exception and keeps historical second-level categories such as `bug-fix`,
  `frontend-playwright`, `project-rebuild-refactor`, and `tui`.
- Typed test runners discover cases by scanning the matching directory type.
  Do not add one-off hardcoded script paths when a directory scan can find the
  case.
- `run-backend-business-tests.*`: run root Rust business tests plus
  crate-owned Rust tests from `crates/*/tests/business`, `commands/*/tests/business`,
  `agents/*/tests/business`, and `personas/*/tests/business` using one-level
  typed-directory scans. These backend runners do not execute `.mjs` app,
  TUI, or GUI scripts; run app suites from `apps/tui` or `apps/gui`.
- `run-backend-live-tests.*`: run opt-in root/backend Rust live tests and
  backend-owned root live scripts using one-level typed-directory scans and the
  `live-tests` feature gate when the package declares it. These backend
  runners do not execute app-owned TUI/GUI scripts; run those from the app
  package commands.
- `run-backend-release-tests.*`: run opt-in release-binary tests discovered only
  from root `tests/release/*.mjs`, separated from business/live runners so
  ordinary test runs do not touch release daemons.
- `run-backend-performance-tests.*`: runner for crate-owned Rust performance
  tests from `crates/*/tests/performance`; each target is killed if it exceeds
  the configured timeout.

Script tests:

- `tests/scripts/test-install.*`: checks script syntax where available, runs the
  root dependency installer, and verifies command-owned Python environments.
- `tests/scripts/test-build-release.*`: validates a dry-run release probe such as
  `release-v0.0.0-ci`, runs `build-release.*`, checks expected artifacts, and
  verifies command protocol health. Pass `-SkipTui` or `--skip-tui` when a CI job
  only needs Rust release artifacts.

GitHub Actions:

- `.github/workflows/scripts-install-release.yml` runs the script tests on
  Windows, macOS, and Linux hosted runners.
- `.github/workflows/release.yml` only treats tags matching `release-v*` as real
  release tags. Manual dry runs use the same script tests and a `release-v...`
  probe without publishing a GitHub release.

Wrapper/package routes are intentionally not used; release commands resolve directly from `target/release`.
