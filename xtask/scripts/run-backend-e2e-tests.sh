#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
LIST=0
TIMEOUT_SECONDS=300

while [ "$#" -gt 0 ]; do
  case "$1" in
    --list) LIST=1 ;;
    --timeout-seconds)
      shift
      if [ "$#" -eq 0 ]; then echo "--timeout-seconds requires a number" >&2; exit 2; fi
      TIMEOUT_SECONDS=$1
      ;;
    -h|--help)
      cat <<'EOF'
Usage:
  xtask/scripts/run-backend-e2e-tests.sh [--list] [--timeout-seconds N]

Scans tests/e2e/*.mjs and runs every backend full-chain entrypoint against
local mock providers. Files ending in _fixture.mjs are support modules only.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"

run_node() {
  if command -v timeout >/dev/null 2>&1; then
    timeout "${TIMEOUT_SECONDS}s" node "$1"
  else
    node "$1"
  fi
}

if [ -d tests/e2e ]; then
  find tests/e2e -maxdepth 1 -type f -name '*.mjs' | sort | while IFS= read -r test_path; do
    name=${test_path##*/}
    case "$name" in
      *_fixture.mjs) continue ;;
    esac
    target=${name%.mjs}
    if [ "$LIST" -eq 1 ]; then
      echo "node::$target $test_path"
      continue
    fi
    printf '\n==> Running backend E2E node::%s\n' "$target"
    run_node "$test_path"
  done
fi
