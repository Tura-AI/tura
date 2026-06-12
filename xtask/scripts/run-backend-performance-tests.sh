#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
XTASK_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$XTASK_ROOT/.." && pwd)
CRATE=""
LIST=0
TIMEOUT_SECONDS=240

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
  scripts/run-backend-performance-tests.sh [--crate PACKAGE] [--list] [--timeout-seconds N]

Scans backend package tests/performance/*.rs and runs each backend performance,
compatibility, concurrency, stress, or stability test target explicitly.
Backend package roots are crates/, commands/, agents/, and personas/.
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

find_backend_performance_tests() {
  for root in crates commands agents personas; do
    if [ -d "$root" ]; then
      find "$root" -path '*/tests/performance/*.rs' -type f
    fi
  done
}

find_backend_performance_tests | sort | while IFS= read -r test_path; do
  crate_root=${test_path%%/tests/performance/*}
  cargo_toml="$crate_root/Cargo.toml"
  if [ ! -f "$cargo_toml" ]; then
    echo "Performance test is not under a crate tests/performance directory: $test_path" >&2
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
  if grep -Eq '^[[:space:]]*performance-tests[[:space:]]*=' "$cargo_toml"; then
    features="performance-tests"
  fi

  if [ "$LIST" -eq 1 ]; then
    echo "$package::$target $test_path"
    continue
  fi

  printf '\n==> Running backend performance test %s::%s\n' "$package" "$target"
  if [ -n "$features" ]; then
    run_cargo test -p "$package" --features "$features" --test "$target" -- --nocapture
  else
    run_cargo test -p "$package" --test "$target" -- --nocapture
  fi
done
