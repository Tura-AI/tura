#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
CRATE=""
LIST=0
TIMEOUT_SECONDS=180
JOBS=${TURA_BACKEND_TEST_JOBS:-4}

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
    --jobs|--parallelism)
      shift
      if [ "$#" -eq 0 ]; then echo "--jobs requires a number" >&2; exit 2; fi
      JOBS=$1
      ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/run-backend-business-tests.sh [--crate PACKAGE] [--list] [--timeout-seconds N] [--jobs N]

Scans root tests/business/*.rs and backend package tests/business/*.rs.
Business tests run in parallel batches. Process, daemon, lifecycle, and OS
policy coverage lives in tests/os_testing and runs through run-backend-os-tests.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"

cases=$(mktemp)
failures=$(mktemp)
trap 'rm -f "$cases" "$failures"' EXIT INT TERM

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

package_name() {
  sed -n 's/^[[:space:]]*name[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | sed -n '1p'
}

business_features() {
  if grep -Eq '^[[:space:]]*business-tests[[:space:]]*=' "$1"; then
    printf '%s' "business-tests"
  fi
}

add_case() {
  package=$1
  target=$2
  features=$3
  path=$4
  if [ "$LIST" -eq 1 ]; then
    echo "$package::$target [parallel] $path"
  else
    printf '%s|%s|%s|%s\n' "$package" "$target" "$features" "$path" >> "$cases"
  fi
}

run_case_record() {
  record=$1
  package=${record%%|*}
  rest=${record#*|}
  target=${rest%%|*}
  rest=${rest#*|}
  features=${rest%%|*}
  printf '\n==> Running backend business test %s::%s [parallel]\n' "$package" "$target"
  if [ -n "$features" ]; then
    run_cargo test -p "$package" --features "$features" --test "$target" -- --nocapture --test-threads=1
  else
    run_cargo test -p "$package" --test "$target" -- --nocapture --test-threads=1
  fi
}

run_parallel_cases() {
  if [ ! -s "$cases" ]; then
    echo "No backend business tests matched."
    return
  fi
  count=$(wc -l < "$cases" | tr -d ' ')
  printf '\n==> Running %s backend business tests with parallelism %s\n' "$count" "$JOBS"
  batch=0
  while IFS= read -r record; do
    (
      run_case_record "$record" || echo "$record" >> "$failures"
    ) &
    batch=$((batch + 1))
    if [ "$batch" -ge "$JOBS" ]; then
      wait
      if [ -s "$failures" ]; then cat "$failures" >&2; exit 1; fi
      batch=0
    fi
  done < "$cases"
  wait
  if [ -s "$failures" ]; then cat "$failures" >&2; exit 1; fi
}

if [ -d tests/business ]; then
  find tests/business -maxdepth 1 -type f -name '*.rs' | sort | while IFS= read -r test_path; do
    case "$test_path" in *claude*) continue ;; esac
    if [ -n "$CRATE" ] && [ "$CRATE" != "tura_workspace" ] && [ "$CRATE" != "." ]; then
      continue
    fi
    target=${test_path##*/}
    target=${target%.rs}
    add_case tura_workspace "$target" "" "$test_path"
  done
fi

find_backend_business_tests | sort | while IFS= read -r test_path; do
  case "$test_path" in *claude*) continue ;; esac
  crate_root=${test_path%%/tests/business/*}
  cargo_toml="$crate_root/Cargo.toml"
  if [ ! -f "$cargo_toml" ]; then
    echo "Business test is not under a crate tests/business directory: $test_path" >&2
    exit 1
  fi
  package=$(package_name "$cargo_toml")
  crate_dir=${crate_root##*/}
  if [ -n "$CRATE" ] && [ "$CRATE" != "$package" ] && [ "$CRATE" != "$crate_dir" ]; then
    continue
  fi
  target=${test_path##*/}
  target=${target%.rs}
  features=$(business_features "$cargo_toml")
  add_case "$package" "$target" "$features" "$test_path"
done

if [ "$LIST" -eq 0 ]; then
  run_parallel_cases
fi
