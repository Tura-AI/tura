#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
XTASK_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$XTASK_ROOT/.." && pwd)
SKIP_AUDIT=0
SKIP_DENY=0
SKIP_TYPOS=0
CRATE=""
LINT_ONLY=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-audit) SKIP_AUDIT=1 ;;
    --skip-deny) SKIP_DENY=1 ;;
    --skip-typos) SKIP_TYPOS=1 ;;
    --crate)
      shift
      if [ "$#" -eq 0 ]; then echo "--crate requires a package name" >&2; exit 2; fi
      CRATE=$1
      ;;
    --lint-only) LINT_ONLY=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/check-backend-quality.sh [--skip-audit] [--skip-deny] [--skip-typos]
  scripts/check-backend-quality.sh --crate PACKAGE
  scripts/check-backend-quality.sh --lint-only

Runs backend quality checks without building release binaries.

Default (whole workspace):
  backend Rust test layout policy check
  cargo fmt --all --check
  cargo clippy --workspace --exclude src-tauri --all-targets
  cargo test --workspace --exclude src-tauri -- --test-threads=1
  cargo audit
  cargo deny check
  typos

Default checks exclude crate-owned typed suites under tests/business,
tests/performance, tests/live, and tests/benchmark. Run those explicitly with
their typed runners; live tests are opt-in because they require third-party
services or secrets.

Split modes (used by CI to run concurrently and avoid timeouts):
  --crate PACKAGE   clippy + test for a single package only
  --lint-only       fmt + audit + deny + typos only (no clippy/test)
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

# Decide which phases run. A single-crate run does clippy+test for that crate
# only; --lint-only does the workspace-wide formatting/policy/spelling checks.
RUN_CRATE_CHECKS=1
RUN_LINT_CHECKS=1
if [ -n "$CRATE" ]; then RUN_LINT_CHECKS=0; fi
if [ "$LINT_ONLY" -eq 1 ]; then RUN_CRATE_CHECKS=0; fi
if [ -n "$CRATE" ] && [ "$LINT_ONLY" -eq 1 ]; then
  echo "--crate and --lint-only are mutually exclusive" >&2
  exit 2
fi

step() {
  printf '\n==> %s\n' "$1"
}

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "$1 was not found on PATH. $2" >&2
    exit 1
  fi
}

require_rust_component() {
  component=$1
  require rustup "Install Rust with rustup from https://rustup.rs/."
  if ! rustup component list --installed 2>/dev/null | grep -Eq "^${component}($|[-[:space:]])"; then
    echo "Rust component $component was not found. Install with: rustup component add $component" >&2
    exit 1
  fi
}

run_python_script() {
  if command -v python3 >/dev/null 2>&1; then
    python3 "$1"
  elif command -v python >/dev/null 2>&1; then
    python "$1"
  else
    echo "python was not found on PATH. Install Python 3 to run backend policy checks." >&2
    exit 1
  fi
}

cd "$REPO_ROOT"

require cargo "Install Rust from https://rustup.rs/."
if [ "$RUN_LINT_CHECKS" -eq 1 ]; then
  require_rust_component rustfmt
fi
if [ "$RUN_CRATE_CHECKS" -eq 1 ]; then
  require_rust_component clippy
fi
if [ "$RUN_LINT_CHECKS" -eq 1 ]; then
  [ "$SKIP_AUDIT" -eq 1 ] || require cargo-audit "Install with: cargo install cargo-audit --locked"
  [ "$SKIP_DENY" -eq 1 ] || require cargo-deny "Install with: cargo install cargo-deny --locked"
  [ "$SKIP_TYPOS" -eq 1 ] || require typos "Install with: cargo install typos-cli --locked"
fi

run_clippy() {
  cargo clippy "$@" --all-targets -- \
    -D warnings \
    -D clippy::redundant_clone \
    -D clippy::clone_on_copy \
    -D clippy::clone_on_ref_ptr \
    -D clippy::unnecessary_to_owned \
    -D clippy::unwrap_used
}

if [ "$RUN_CRATE_CHECKS" -eq 1 ]; then
  if [ -n "$CRATE" ]; then
    step "Running Clippy for $CRATE"
    run_clippy -p "$CRATE"
    step "Running Rust tests for $CRATE"
    cargo test -p "$CRATE" -- --test-threads=1
  else
    step "Running Clippy over the Rust workspace"
    run_clippy --workspace --exclude src-tauri
    step "Running Rust tests over the workspace"
    cargo test --workspace --exclude src-tauri -- --test-threads=1
  fi
fi

if [ "$RUN_LINT_CHECKS" -eq 1 ]; then
  step "Checking backend Rust test layout"
  run_python_script "$SCRIPT_DIR/check-backend-test-layout.py"

  step "Checking Rust formatting"
  cargo fmt --all --check -- --config-path "$XTASK_ROOT/rustfmt.toml"

  if [ "$SKIP_AUDIT" -eq 0 ]; then
    step "Auditing Rust dependencies"
    cargo audit
  fi

  if [ "$SKIP_DENY" -eq 0 ]; then
    step "Checking Rust dependency policy"
    cargo deny check --config "$XTASK_ROOT/deny.toml"
  fi

  if [ "$SKIP_TYPOS" -eq 0 ]; then
    step "Checking repository spelling"
    typos --config "$XTASK_ROOT/typos.toml"
  fi
fi
