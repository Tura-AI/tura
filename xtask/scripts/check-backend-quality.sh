#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
XTASK_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$XTASK_ROOT/.." && pwd)
SKIP_AUDIT=0
SKIP_DENY=0
SKIP_TYPOS=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-audit) SKIP_AUDIT=1 ;;
    --skip-deny) SKIP_DENY=1 ;;
    --skip-typos) SKIP_TYPOS=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/check-backend-quality.sh [--skip-audit] [--skip-deny] [--skip-typos]

Runs backend quality checks without building release binaries:
  cargo fmt --all --check
  cargo clippy --workspace --exclude src-tauri --all-targets
  cargo test --workspace --exclude src-tauri
  cargo audit
  cargo deny check
  typos
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

cd "$REPO_ROOT"

require cargo "Install Rust from https://rustup.rs/."
require_rust_component rustfmt
require_rust_component clippy
require_rust_component rust-analyzer
[ "$SKIP_AUDIT" -eq 1 ] || require cargo-audit "Install with: cargo install cargo-audit --locked"
[ "$SKIP_DENY" -eq 1 ] || require cargo-deny "Install with: cargo install cargo-deny --locked"
[ "$SKIP_TYPOS" -eq 1 ] || require typos "Install with: cargo install typos-cli --locked"

step "Checking Rust formatting"
cargo fmt --all --check -- --config-path "$XTASK_ROOT/rustfmt.toml"

step "Running Clippy over the Rust workspace"
cargo clippy --workspace --exclude src-tauri --all-targets -- \
  -W clippy::redundant_clone \
  -W clippy::clone_on_copy \
  -W clippy::clone_on_ref_ptr \
  -W clippy::unnecessary_to_owned \
  -W clippy::unwrap_used

step "Running Rust tests over the workspace"
cargo test --workspace --exclude src-tauri

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
