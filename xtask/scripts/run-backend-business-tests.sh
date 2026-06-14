#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
XTASK_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$XTASK_ROOT/.." && pwd)
CRATE=""
LIST=0
TIMEOUT_SECONDS=180

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
  scripts/run-backend-business-tests.sh [--crate PACKAGE] [--list] [--timeout-seconds N]

Scans root tests/business/*.rs and backend package tests/business/*.rs, then
runs backend business tests explicitly.
Backend package roots are crates/, commands/, agents/, and personas/.
Typed test directories are flat: encode the scenario in the filename instead of
creating tests/business subdirectories.
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

find_backend_business_tests() {
  for root in crates commands agents personas; do
    if [ -d "$root" ]; then
      find "$root" -path '*/tests/business/*.rs' -type f
    fi
  done
}

run_process_sensitive_unit_tests() {
  package=gateway
  filter='session::store::tests::'
  if [ -n "$CRATE" ] && [ "$CRATE" != "$package" ]; then
    return
  fi
  if [ "$LIST" -eq 1 ]; then
    echo "$package::$filter <process-sensitive unit tests>"
    return
  fi
  printf '\n==> Running process-sensitive unit tests %s::%s\n' "$package" "$filter"
  run_cargo test -p "$package" --features business-tests "$filter" -- --nocapture --test-threads=1
}

run_process_sensitive_unit_tests

if [ -d tests/business ]; then
  find tests/business -maxdepth 1 -type f -name '*.rs' | sort | while IFS= read -r test_path; do
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
    printf '\n==> Running root business test tura_workspace::%s\n' "$target"
    run_cargo test -p tura_workspace --test "$target" -- --nocapture --test-threads=1
  done
fi

find_backend_business_tests | sort | while IFS= read -r test_path; do
  case "$test_path" in
    *claude*) continue ;;
  esac
  crate_root=${test_path%%/tests/business/*}
  cargo_toml="$crate_root/Cargo.toml"
  if [ ! -f "$cargo_toml" ]; then
    echo "Business test is not under a crate tests/business directory: $test_path" >&2
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
  if grep -Eq '^[[:space:]]*business-tests[[:space:]]*=' "$cargo_toml"; then
    features="business-tests"
  fi

  if [ "$LIST" -eq 1 ]; then
    echo "$package::$target $test_path"
    continue
  fi

  printf '\n==> Running backend business test %s::%s\n' "$package" "$target"
  if [ -n "$features" ]; then
    run_cargo test -p "$package" --features "$features" --test "$target" -- --nocapture --test-threads=1
  else
    run_cargo test -p "$package" --test "$target" -- --nocapture --test-threads=1
  fi
 done
