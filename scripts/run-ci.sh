#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
JOBS=${TURA_CI_JOBS:-4}
CRATE_TIMEOUT_SECONDS=600
BUSINESS_TIMEOUT_SECONDS=300
TUI_TIMEOUT_SECONDS=600
SKIP_QUALITY=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --jobs|--parallelism)
      shift
      if [ "$#" -eq 0 ]; then echo "--jobs requires a number" >&2; exit 2; fi
      JOBS=$1
      ;;
    --crate-timeout-seconds)
      shift
      if [ "$#" -eq 0 ]; then echo "--crate-timeout-seconds requires a number" >&2; exit 2; fi
      CRATE_TIMEOUT_SECONDS=$1
      ;;
    --business-timeout-seconds)
      shift
      if [ "$#" -eq 0 ]; then echo "--business-timeout-seconds requires a number" >&2; exit 2; fi
      BUSINESS_TIMEOUT_SECONDS=$1
      ;;
    --tui-timeout-seconds)
      shift
      if [ "$#" -eq 0 ]; then echo "--tui-timeout-seconds requires a number" >&2; exit 2; fi
      TUI_TIMEOUT_SECONDS=$1
      ;;
    --skip-quality) SKIP_QUALITY=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/run-ci.sh [--jobs N] [--skip-quality]

Runs the local GitHub-style CI flow:
  1. check-backend-quality smell gate
  2. in parallel: crate clippy/tests, backend business tests, TUI unit+e2e tests
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"
mkdir -p target/test-logs/ci

run_with_timeout() {
  timeout_seconds=$1
  shift
  if command -v timeout >/dev/null 2>&1; then
    timeout "${timeout_seconds}s" "$@"
  else
    "$@"
  fi
}

run_job() {
  name=$1
  timeout_seconds=$2
  shift 2
  stdout="target/test-logs/ci/${name}.out.txt"
  stderr="target/test-logs/ci/${name}.err.txt"
  printf '==> Starting CI job: %s\n' "$name"
  if run_with_timeout "$timeout_seconds" "$@" >"$stdout" 2>"$stderr"; then
    printf '==> Passed CI job: %s\n' "$name"
  else
    printf '%s\n' "$name" >> "$failures"
    printf '==> Failed CI job: %s\n' "$name"
  fi
}

if [ "$SKIP_QUALITY" -eq 0 ]; then
  printf '==> Running CI quality smell gate\n'
  sh scripts/check-backend-quality.sh
fi

failures=$(mktemp)
trap 'rm -f "$failures"' EXIT INT TERM

case "$(uname -s 2>/dev/null || true)" in
  MINGW*|MSYS*|CYGWIN*)
    run_job crates "$((CRATE_TIMEOUT_SECONDS * 2))" sh xtask/scripts/run-ci-crate-tests.sh --jobs "$JOBS" --timeout-seconds "$CRATE_TIMEOUT_SECONDS" &
    run_job tui-local "$TUI_TIMEOUT_SECONDS" npm --prefix apps/tui run test:unit &
    wait
    run_job backend-business "$((BUSINESS_TIMEOUT_SECONDS * 2))" sh xtask/scripts/run-backend-business-tests.sh --jobs "$JOBS" --timeout-seconds "$BUSINESS_TIMEOUT_SECONDS"
    ;;
  *)
    run_job crates "$((CRATE_TIMEOUT_SECONDS * 2))" sh xtask/scripts/run-ci-crate-tests.sh --jobs "$JOBS" --timeout-seconds "$CRATE_TIMEOUT_SECONDS" &
    run_job backend-business "$((BUSINESS_TIMEOUT_SECONDS * 2))" sh xtask/scripts/run-backend-business-tests.sh --jobs "$JOBS" --timeout-seconds "$BUSINESS_TIMEOUT_SECONDS" &
    run_job tui-local "$TUI_TIMEOUT_SECONDS" npm --prefix apps/tui run test:unit &
    wait
    ;;
esac

if [ -s "$failures" ]; then
  printf 'failed CI jobs: '
  paste -sd ', ' "$failures"
  while IFS= read -r name; do
    printf '%s\n' "--- stdout tail $name ---"
    tail -n 80 "target/test-logs/ci/${name}.out.txt" || true
    printf '%s\n' "--- stderr tail $name ---"
    tail -n 120 "target/test-logs/ci/${name}.err.txt" || true
  done < "$failures"
  exit 1
fi

printf 'CI flow passed\n'
