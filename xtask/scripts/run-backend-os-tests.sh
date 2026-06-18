#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
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
  xtask/scripts/run-backend-os-tests.sh [--crate PACKAGE] [--list] [--timeout-seconds N]

Scans root tests/os_testing/*.rs and backend package tests/os_testing/*.rs.
OS, daemon, process, service-owner, and lifecycle tests run serially.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"

cases=$(mktemp)
trap 'rm -f "$cases"' EXIT INT TERM

run_cargo() {
  if command -v timeout >/dev/null 2>&1; then
    timeout "${TIMEOUT_SECONDS}s" cargo "$@"
  else
    cargo "$@"
  fi
}

find_backend_os_tests() {
  for root in crates commands agents personas; do
    if [ -d "$root" ]; then
      find "$root" -path '*/tests/os_testing/*.rs' -type f
    fi
  done
}

package_name() {
  sed -n 's/^[[:space:]]*name[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | sed -n '1p'
}

os_features() {
  if grep -Eq '^[[:space:]]*os-tests[[:space:]]*=' "$1"; then
    printf '%s' "os-tests"
  fi
}

run_case() {
  record=$1
  package=${record%%|*}
  rest=${record#*|}
  target=${rest%%|*}
  features=${rest#*|}
  printf '\n==> Running backend OS test %s::%s [serial]\n' "$package" "$target"
  if [ -n "$features" ]; then
    run_cargo test -p "$package" --features "$features" --test "$target" -- --nocapture --test-threads=1
  else
    run_cargo test -p "$package" --test "$target" -- --nocapture --test-threads=1
  fi
}

add_case() {
  package=$1
  target=$2
  features=$3
  path=$4
  if [ "$LIST" -eq 1 ]; then
    echo "$package::$target [serial] $path"
  else
    printf '%s|%s|%s\n' "$package" "$target" "$features" >> "$cases"
  fi
}

if [ -d tests/os_testing ]; then
  for test_path in $(find tests/os_testing -maxdepth 1 -type f -name '*.rs' | sort); do
    if [ -n "$CRATE" ] && [ "$CRATE" != "tura_workspace" ] && [ "$CRATE" != "." ]; then
      continue
    fi
    target=${test_path##*/}
    target=${target%.rs}
    features=$(os_features Cargo.toml)
    add_case tura_workspace "$target" "$features" "$test_path"
  done
fi

find_backend_os_tests | sort | while IFS= read -r test_path; do
  crate_root=${test_path%%/tests/os_testing/*}
  cargo_toml="$crate_root/Cargo.toml"
  if [ ! -f "$cargo_toml" ]; then
    echo "OS test is not under a crate tests/os_testing directory: $test_path" >&2
    exit 1
  fi
  package=$(package_name "$cargo_toml")
  crate_dir=${crate_root##*/}
  if [ -n "$CRATE" ] && [ "$CRATE" != "$package" ] && [ "$CRATE" != "$crate_dir" ]; then
    continue
  fi
  target=${test_path##*/}
  target=${target%.rs}
  features=$(os_features "$cargo_toml")
  add_case "$package" "$target" "$features" "$test_path"
done

if [ "$LIST" -eq 0 ]; then
  if [ ! -s "$cases" ]; then
    echo "No backend OS tests matched."
  else
    count=$(wc -l < "$cases" | tr -d ' ')
    printf '\n==> Running %s backend OS tests serially\n' "$count"
    while IFS= read -r record; do
      run_case "$record"
    done < "$cases"
  fi
fi
