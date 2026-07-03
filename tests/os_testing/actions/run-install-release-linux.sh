#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../../.." && pwd)

run_logged() {
  label=$1
  shift
  log=$(mktemp)
  set +e
  "$@" > "$log" 2>&1
  status=$?
  set -e
  cat "$log"
  if [ "$status" -ne 0 ]; then
    tail_text=$(tail -n 80 "$log" | sed ':a;N;$!ba;s/%/%25/g;s/\r/%0D/g;s/\n/%0A/g')
    printf '::error title=Install release failed::%s exited with %s%%0A%s\n' "$label" "$status" "$tail_text"
    rm -f "$log"
    exit "$status"
  fi
  rm -f "$log"
}

run_logged "test-install" sh "$REPO_ROOT/scripts/tests/scripts/test-install.sh" --full --skip-apps
run_logged "test-build-release" sh "$REPO_ROOT/scripts/tests/scripts/test-build-release.sh" --backend-only
