#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
CRATE=""
LIST=0
TIMEOUT_SECONDS=300

while [ "$#" -gt 0 ]; do
  case "$1" in
    --crate)
      shift
      if [ "$#" -eq 0 ]; then echo "--crate requires a package name" >&2; exit 2; fi
      CRATE=$1
      ;;
    --list) LIST=1 ;;
    --timeout-seconds)
      shift
      if [ "$#" -eq 0 ]; then echo "--timeout-seconds requires a number" >&2; exit 2; fi
      TIMEOUT_SECONDS=$1
      ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/run-backend-live-tests.sh [--crate PACKAGE] [--list] [--timeout-seconds N]

Scans root tests/live/*.rs, backend package tests/live/*.rs, and backend-owned
root tests/live/*.mjs files and runs opt-in live tests.
Backend package roots are crates/, commands/, agents/, and personas/.
Runnable live Rust entrypoints stay directly under tests/live; target-owned
helper modules may live in sibling subdirectories.
Live tests may require provider credentials, public network access, third-party
services, or provider runtime state such as auth/config/env.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"

run_cargo() {
  if command -v timeout >/dev/null 2>&1; then
    timeout "${TIMEOUT_SECONDS}s" cargo "$@"
  else
    cargo "$@"
  fi
}

run_node() {
  if command -v timeout >/dev/null 2>&1; then
    timeout "${TIMEOUT_SECONDS}s" node "$1"
  else
    node "$1"
  fi
}

find_backend_live_tests() {
  for root in crates commands agents personas; do
    if [ -d "$root" ]; then
      find "$root" -path '*/tests/live/*.rs' -type f
    fi
  done
}

if [ -d tests/live ]; then
  find tests/live -maxdepth 1 -type f -name '*.rs' | sort | while IFS= read -r test_path; do
    case "$test_path" in
      *claude*) continue ;;
    esac
    if [ -n "$CRATE" ] && [ "$CRATE" != "tura_workspace" ] && [ "$CRATE" != "." ]; then
      continue
    fi
    target=${test_path##*/}
    target=${target%.rs}
    if [ "$LIST" -eq 1 ]; then
      echo "tura_workspace::$target $test_path"
      continue
    fi
    printf '\n==> Running root live test tura_workspace::%s\n' "$target"
    run_cargo test -p tura_workspace --test "$target" -- --nocapture --test-threads=1
  done
fi

find_backend_live_tests | sort | while IFS= read -r test_path; do
  case "$test_path" in
    *claude*) continue ;;
  esac
  crate_root=${test_path%%/tests/live/*}
  cargo_toml="$crate_root/Cargo.toml"
  if [ ! -f "$cargo_toml" ]; then
    echo "Live test is not under a crate tests/live directory: $test_path" >&2
    exit 1
  fi
  package=$(sed -n 's/^[[:space:]]*name[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$cargo_toml" | sed -n '1p')
  crate_dir=${crate_root##*/}
  if [ -n "$CRATE" ] && [ "$CRATE" != "$package" ] && [ "$CRATE" != "$crate_dir" ]; then
    continue
  fi
  target=${test_path##*/}
  target=${target%.rs}

  features=""
  if grep -Eq '^[[:space:]]*live-tests[[:space:]]*=' "$cargo_toml"; then
    features="live-tests"
  fi

  if [ "$LIST" -eq 1 ]; then
    echo "$package::$target $test_path"
    continue
  fi

  printf '\n==> Running backend live test %s::%s\n' "$package" "$target"
  if [ -n "$features" ]; then
    run_cargo test -p "$package" --features "$features" --test "$target" -- --nocapture --test-threads=1
  else
    run_cargo test -p "$package" --test "$target" -- --nocapture --test-threads=1
  fi
done

if [ -d tests/live ]; then
  find tests/live -maxdepth 1 -type f -name '*.mjs' | sort | while IFS= read -r test_path; do
    name=${test_path##*/}
    case "$name" in
      *claude*|live_lib_*|*_lib_*|tui_*|gui_*) continue ;;
    esac
    if [ -n "$CRATE" ] && [ "$CRATE" != "node" ] && [ "$CRATE" != "tura_workspace" ] && [ "$CRATE" != "." ]; then
      continue
    fi
    target=${name%.mjs}
    if [ "$LIST" -eq 1 ]; then
      echo "node::$target $test_path"
      continue
    fi
    printf '\n==> Running root live script node::%s\n' "$target"
    run_node "$test_path"
  done
fi
