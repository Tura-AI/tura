# Scripts

Scripts only build and register the two standard Cargo output directories:

- `target/debug`
- `target/release`

Command entries:

- `tura_exec`: Rust one-shot CLI binary.
- `tura`: compiled terminal entry. Use `tura` for the TUI, `tura run "prompt"` for the TUI gateway client, or `tura exec "prompt"` for the Rust CLI front.
- `tura_gateway`, `tura_router`, `tura_session_db`, `tura_runtime`: backend services.

Important scripts:

- `install.*`: install dependencies only. The root installer ensures user-local
  `uv` and `bun`, calls command-owned `commands/*/install.*` scripts, and runs
  Bun installs inside app/package directories. It does not build binaries or
  register PATH launchers. On macOS it also asserts that zsh is available for
  the default command-run shell surface.
- `build-debug.*`: build Rust debug binaries and the TUI entry into `target/debug`.
- `build-release.*`: build Rust release binaries and the TUI entry into `target/release`.
- `register-cli.*`: add `target/release` to the user PATH. No wrapper directory is created; the registered CLI command is `tura exec`. The POSIX script updates `.profile`, `.bash_profile`, `.bashrc`, `.zprofile`, and `.zshrc` when present, and creates `.zprofile`/`.zshrc` on macOS so new Terminal sessions work.
- `unregister-cli.*`: remove `target/release` from PATH and delete the legacy `cli-bin` directory if present.
- `start.*`: convenience runner for `target/debug` by default, or `target/release` with `--release`. On macOS the POSIX runner asserts zsh availability before launching.
- `run-backend-business-tests.*`: run crate-owned Rust tests from `crates/*/tests/business`; `flow`, `live`, and `long-e2e` are selected by directory suite.
- `run-backend-performance-tests.*`: run crate-owned Rust tests from `crates/*/tests/performance`.

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

Legacy wrapper/package routes are intentionally not used.
