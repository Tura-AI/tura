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
  DB/cache files first. The build scripts only stop repo-owned backend/service
  binaries before rebuilding; they do not stop the interactive `tura` TUI or
  `tura_gui` desktop process. If a frontend executable is locked, close it
  explicitly and rerun. Pass `-BackendOnly` or `--backend-only` when a CI job only
  needs Rust release artifacts.
- `register-cli.*`: add `target/release` to the user PATH. No wrapper directory is created; the registered CLI command is `tura exec`. The POSIX script ensures `.profile` exists, updates shell profiles when present, and creates `.zprofile`/`.zshrc` on macOS so new Terminal sessions work.
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
- `scripts/npm/install-release.mjs`: npm postinstall release installer for the
  public `tura-ai` package. It first uses an installed platform package such as
  `tura-win32-x64`, then a local archive from `release/`, and finally a GitHub
  Release archive. The installed runtime layout is `target/release` with
  `config/provider_config.json`, backend binaries, TUI, GUI dist, and Tauri
  bundle artifacts. After verifying the release files it calls
  `scripts/npm/cli-path.mjs` so npm installs register the `tura` command on the
  current OS; set `TURA_NPM_SKIP_CLI_REGISTRATION=1` to suppress this in
  automation. Current npm releases do not run uninstall lifecycle scripts, so
  the package exposes `tura unregister-cli` for PATH/profile cleanup before
  `npm uninstall tura-ai` instead of publishing fake `uninstall` scripts. The
  npm release workflow builds CLI/backend/TUI and web GUI artifacts with Tauri
  packaging skipped, so desktop installer failures do not block publishing the
  platform npm packages used by `npm install tura-ai`. Its local install
  verifier stages the freshly packed platform tarball outside the main install
  tree and points `TURA_NPM_PLATFORM_PACKAGE_DIR` at it, avoiding npm registry
  lookups for optional platform packages before those packages are published.
  The verifier checks the installed release files, verifies PATH registration,
  runs `tura unregister-cli`, and asserts the PATH entry was removed. The wrapper
  passes `TURA_RELEASE_BIN_DIR` so the compiled TUI resolves sibling Rust release
  binaries from the npm installation layout. Postinstall also restores executable
  bits on copied release binaries because npm package tarballs do not preserve
  native executable modes for ordinary package files.
- `scripts/npm/stage-main-package.mjs` and
  `scripts/npm/restore-main-package.mjs`: temporarily replace the repository
  `package.json` during `npm pack`/`npm publish` so the published main package
  contains only runtime files and the real `postinstall` lifecycle script. The
  repository package metadata is restored in `postpack`.
- `scripts/npm/package-platform.mjs`: stages the current OS release into a
  platform npm package: `tura-linux-x64`, `tura-darwin-x64`,
  `tura-darwin-arm64`, or `tura-win32-x64`.
- `scripts/npm/package-release.mjs`: creates the matching GitHub Release archive
  under `release/`, for example `tura-v0.1.0-windows-x64.zip` or
  `tura-v0.1.0-macos-arm64.tar.gz`.

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
- `.github/workflows/os-worker-tests.yml` runs the four current OS runners
  (`ubuntu-latest`, `macos-latest`, `windows-2022`, and `windows-2025`) through
  install-script checks, backend release-script checks, and serial backend OS
  tests.
- `.github/workflows/npm-release.yml` builds the four npm platform releases
  (`tura-linux-x64`, `tura-darwin-x64`, `tura-darwin-arm64`, and
  `tura-win32-x64`), verifies a local `npm install` of the main `tura-ai` package
  against the platform package, verifies the slim main npm package contents,
  verifies postinstall CLI registration plus `tura unregister-cli`, uploads
  release archives, and publishes npm packages when `NPM_TOKEN` is configured.

Local source builds still resolve directly from `target/release`. Published npm
installs resolve through the main `tura-ai` package plus the matching platform
package, with GitHub Release archives as the fallback install route.
