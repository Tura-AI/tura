#!/usr/bin/env sh
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
XTASK_ROOT="$REPO_ROOT/xtask"
SKIP_AUDIT=0
SKIP_DENY=0
SKIP_TYPOS=0
AUDIT_IGNORES="RUSTSEC-2026-0194 RUSTSEC-2026-0195"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-audit) SKIP_AUDIT=1 ;;
    --skip-deny) SKIP_DENY=1 ;;
    --skip-typos) SKIP_TYPOS=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/check-backend-quality.sh [--skip-audit] [--skip-deny] [--skip-typos]

Runs CI smell checks without building release binaries or running Rust tests.

Checks:
  backend Rust test layout policy check
  cargo fmt --all --check
  npm --prefix apps/tui run format:check
  cargo audit
  cargo deny check
  typos

Crate clippy/test and business suites run in the CI crate/business runners.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

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
require_rust_component rustfmt
require npm "Install Node.js/npm to run TUI formatting checks."
[ "$SKIP_AUDIT" -eq 1 ] || require cargo-audit "Install with: cargo install cargo-audit --locked"
[ "$SKIP_DENY" -eq 1 ] || require cargo-deny "Install with: cargo install cargo-deny --locked"
[ "$SKIP_TYPOS" -eq 1 ] || require typos "Install with: cargo install typos-cli --locked"

step "Checking backend Rust test layout"
run_python_script "$XTASK_ROOT/scripts/check-backend-test-layout.py"

step "Checking Rust formatting"
cargo fmt --all --check -- --config-path "$XTASK_ROOT/rustfmt.toml"

step "Checking TUI formatting"
npm --prefix apps/tui run format:check

if [ "$SKIP_AUDIT" -eq 0 ]; then
  step "Auditing Rust dependencies"
  # quick-xml is pulled through tauri -> plist 1.9.0, and plist has no release
  # on crates.io that permits quick-xml >= 0.41.0 yet. Remove these ignores
  # once plist/tauri exposes a patched dependency path.
  audit_args=""
  for advisory in $AUDIT_IGNORES; do
    audit_args="$audit_args --ignore $advisory"
  done
  # shellcheck disable=SC2086
  cargo audit $audit_args
fi

if [ "$SKIP_DENY" -eq 0 ]; then
  step "Checking Rust dependency policy"
  cargo deny check --config "$XTASK_ROOT/deny.toml" \
    -A license-not-encountered \
    -A advisory-not-detected
fi

if [ "$SKIP_TYPOS" -eq 0 ]; then
  step "Checking repository spelling"
  typos --config "$XTASK_ROOT/typos.toml"
fi
