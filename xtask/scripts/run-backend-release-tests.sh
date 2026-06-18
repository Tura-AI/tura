#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
LIST=0
TIMEOUT_SECONDS=600

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
  xtask/scripts/run-backend-release-tests.sh [--list] [--timeout-seconds N]

Scans backend-owned tests/release/*.mjs release binary tests only. App-owned
TUI/GUI release scripts live under their app e2e directories and are run by
the app package commands.
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

if [ -d tests/release ]; then
  find tests/release -maxdepth 1 -type f -name '*.mjs' | sort | while IFS= read -r test_path; do
    name=${test_path##*/}
    case "$name" in
      release_lib_*|*_lib_*) continue ;;
      tui_*|gui_*) continue ;;
    esac
    target=${name%.mjs}
    if [ "$LIST" -eq 1 ]; then
      echo "node::$target $test_path"
      continue
    fi
    printf '\n==> Running release binary script node::%s\n' "$target"
    run_node "$test_path"
  done
fi
