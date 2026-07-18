#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
JOBS=${TURA_CI_CRATE_JOBS:-4}
TIMEOUT_SECONDS=600
LIST=0
CRATES=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --crate)
      shift
      if [ "$#" -eq 0 ]; then echo "--crate requires a package name" >&2; exit 2; fi
      CRATES="${CRATES}${CRATES:+ }$1"
      ;;
    --jobs|--parallelism)
      shift
      if [ "$#" -eq 0 ]; then echo "--jobs requires a number" >&2; exit 2; fi
      JOBS=$1
      ;;
    --timeout-seconds)
      shift
      if [ "$#" -eq 0 ]; then echo "--timeout-seconds requires a number" >&2; exit 2; fi
      TIMEOUT_SECONDS=$1
      ;;
    --list) LIST=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  xtask/scripts/run-ci-crate-tests.sh [--crate PACKAGE ...] [--jobs N] [--timeout-seconds N] [--list]

Runs the CI crate matrix locally: clippy + cargo test for each backend default
workspace package, excluding tura_gui. Crates run in parallel; each crate test
target uses --test-threads=1.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"
mkdir -p target/test-logs/ci-crates

default_crates() {
  cargo metadata --no-deps --format-version 1 | python3 -c '
import json, sys
metadata = json.load(sys.stdin)
members = set(metadata["workspace_default_members"])
for package in metadata["packages"]:
    if package["id"] in members and package["name"] != "tura_gui":
        print(package["name"])
'
}

if [ -z "$CRATES" ]; then
  CRATES=$(default_crates | tr -d '\r' | tr '\n' ' ')
fi

if [ "$LIST" -eq 1 ]; then
  for crate in $CRATES; do
    echo "$crate"
  done
  exit 0
fi

run_with_timeout() {
  if command -v timeout >/dev/null 2>&1; then
    timeout "${TIMEOUT_SECONDS}s" "$@"
  else
    "$@"
  fi
}

run_crate() {
  crate=$1
  stdout="target/test-logs/ci-crates/${crate}.out.txt"
  stderr="target/test-logs/ci-crates/${crate}.err.txt"
  printf '==> Starting CI crate check: %s\n' "$crate"
  {
    run_with_timeout cargo clippy -p "$crate" --all-targets -- \
      -D warnings \
      -D clippy::redundant_clone \
      -D clippy::clone_on_copy \
      -D clippy::clone_on_ref_ptr \
      -D clippy::unnecessary_to_owned \
      -D clippy::unwrap_used &&
    run_with_timeout cargo test -p "$crate" -- --test-threads=1
  } >"$stdout" 2>"$stderr"
}

failures=$(mktemp)
trap 'rm -f "$failures"' EXIT INT TERM

running=0
for crate in $CRATES; do
  (
    if run_crate "$crate"; then
      printf '==> Passed CI crate check: %s\n' "$crate"
    else
      printf '%s\n' "$crate" >> "$failures"
      printf '==> Failed CI crate check: %s\n' "$crate"
    fi
  ) &
  running=$((running + 1))
  if [ "$running" -ge "$JOBS" ]; then
    wait
    running=0
  fi
done
wait

if [ -s "$failures" ]; then
  printf 'failed crates: '
  paste -sd ', ' "$failures"
  while IFS= read -r crate; do
    printf '%s\n' "--- stdout tail $crate ---"
    tail -n 80 "target/test-logs/ci-crates/${crate}.out.txt" || true
    printf '%s\n' "--- stderr tail $crate ---"
    tail -n 120 "target/test-logs/ci-crates/${crate}.err.txt" || true
  done < "$failures"
  exit 1
fi

printf 'all CI crate checks passed: %s\n' "$CRATES"
