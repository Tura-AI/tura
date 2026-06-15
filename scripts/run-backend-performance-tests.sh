#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
CRATE=""
LIST=0
TIMEOUT_SECONDS=240
JOBS=${TURA_BACKEND_TEST_JOBS:-3}

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
  scripts/run-backend-performance-tests.sh [--crate PACKAGE] [--list] [--timeout-seconds N] [--jobs N]

Runs backend tests/performance/*.rs in parallel batches. Process, lifecycle,
router, and session_db stress tests belong in tests/os_testing.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"

parallel_cases=$(mktemp)
serial_cases=$(mktemp)
failures=$(mktemp)
trap 'rm -f "$parallel_cases" "$serial_cases" "$failures"' EXIT INT TERM

run_cargo() {
  if command -v timeout >/dev/null 2>&1; then
    timeout "${TIMEOUT_SECONDS}s" cargo "$@"
  else
    cargo "$@"
  fi
}

package_name() {
  sed -n 's/^[[:space:]]*name[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | sed -n '1p'
}

performance_features() {
  if grep -Eq '^[[:space:]]*performance-tests[[:space:]]*=' "$1"; then
    printf '%s' "performance-tests"
  fi
}

is_process_sensitive() {
  name=$(printf '%s::%s %s' "$1" "$2" "$3" | tr '[:upper:]' '[:lower:]')
  case "$name" in
    *process*|*lifecycle*|*router*|*session_db*|*service*) return 0 ;;
    *) return 1 ;;
  esac
}

add_case() {
  package=$1
  target=$2
  features=$3
  path=$4
  if is_process_sensitive "$package" "$target" "$path"; then
    group=serial
    file=$serial_cases
  else
    group=parallel
    file=$parallel_cases
  fi
  if [ "$LIST" -eq 1 ]; then
    echo "$package::$target [$group] $path"
  else
    printf '%s|%s|%s|%s\n' "$package" "$target" "$features" "$group" >> "$file"
  fi
}

run_case_record() {
  record=$1
  package=${record%%|*}
  rest=${record#*|}
  target=${rest%%|*}
  rest=${rest#*|}
  features=${rest%%|*}
  rest=${rest#*|}
  group=$rest
  printf '\n==> Running backend performance test %s::%s [%s]\n' "$package" "$target" "$group"
  if [ -n "$features" ]; then
    run_cargo test -p "$package" --features "$features" --test "$target" -- --nocapture --test-threads=1
  else
    run_cargo test -p "$package" --test "$target" -- --nocapture --test-threads=1
  fi
}

run_parallel_cases() {
  if [ ! -s "$parallel_cases" ]; then return; fi
  count=$(wc -l < "$parallel_cases" | tr -d ' ')
  printf '\n==> Running %s non-process backend performance tests with parallelism %s\n' "$count" "$JOBS"
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
  done < "$parallel_cases"
  wait
  if [ -s "$failures" ]; then cat "$failures" >&2; exit 1; fi
}

run_serial_cases() {
  if [ ! -s "$serial_cases" ]; then return; fi
  while IFS= read -r record; do
    run_case_record "$record"
  done < "$serial_cases"
}

scan_roots=""
for root in crates commands agents personas; do
  if [ -d "$root" ]; then
    scan_roots="$scan_roots $root"
  fi
done

for root in $scan_roots; do
  find "$root" -path '*/tests/performance/*.rs' -type f
done | sort | while IFS= read -r test_path; do
  crate_root=${test_path%%/tests/performance/*}
  cargo_toml="$crate_root/Cargo.toml"
  if [ ! -f "$cargo_toml" ]; then
    echo "Performance test is not under a crate tests/performance directory: $test_path" >&2
    exit 1
  fi
  package=$(package_name "$cargo_toml")
  crate_dir=${crate_root##*/}
  if [ -n "$CRATE" ] && [ "$CRATE" != "$package" ] && [ "$CRATE" != "$crate_dir" ]; then
    continue
  fi
  target=${test_path##*/}
  target=${target%.rs}
  features=$(performance_features "$cargo_toml")
  add_case "$package" "$target" "$features" "$test_path"
done

if [ "$LIST" -eq 0 ]; then
  run_parallel_cases
  run_serial_cases
fi
