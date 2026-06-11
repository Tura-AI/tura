#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../../.." && pwd)
TARGET_DIR="$REPO_ROOT/target/release"
SKIP_TUI=0
RELEASE_PROBE="${TURA_RELEASE_PROBE:-release-v0.0.0-ci}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-tui) SKIP_TUI=1 ;;
    --release-probe)
      shift
      [ "$#" -gt 0 ] || { echo "--release-probe requires a value" >&2; exit 2; }
      RELEASE_PROBE=$1
      ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/tests/scripts/test-build-release.sh [--skip-tui] [--release-probe release-v0.0.0-ci]
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

step() {
  printf '\n==> %s\n' "$1"
}

require_path() {
  path=$1
  message=$2
  [ -e "$path" ] || { echo "$message" >&2; exit 1; }
}

case "$RELEASE_PROBE" in
  release-v[0-9]*.[0-9]*.[0-9]*)
    ;;
  *)
    echo "Release probe must look like release-v0.0.0 or release-v0.0.0-ci; got '$RELEASE_PROBE'." >&2
    exit 1
    ;;
esac

protocol_health() {
  binary=$1
  output=$(printf '%s\n' '{"kind":"health_check","payload":{}}' | "$binary" --protocol)
  printf '%s' "$output" | grep -q '"ok":true' || {
    echo "Protocol health check failed for $binary: $output" >&2
    exit 1
  }
  printf '%s' "$output" | grep -q '"status":"ok"' || {
    echo "Protocol health check returned unexpected output for $binary: $output" >&2
    exit 1
  }
}

step "Release probe: $RELEASE_PROBE"
step "Running release build script"
if [ "$SKIP_TUI" -eq 1 ]; then
  sh "$REPO_ROOT/scripts/build-release.sh" --skip-tui
else
  sh "$REPO_ROOT/scripts/build-release.sh"
fi

step "Checking release artifacts"
for name in \
  tura_exec \
  tura_gateway \
  tura_router \
  tura_session_db \
  tura_runtime \
  tura-command-read-media \
  tura-command-web-discover
do
  require_path "$TARGET_DIR/$name" "Missing release artifact: $name"
done

if [ "$SKIP_TUI" -eq 0 ]; then
  require_path "$TARGET_DIR/tura" "Missing release TUI executable."
  require_path "$TARGET_DIR/gui/index.html" "Missing release GUI dist."
fi

step "Checking command protocol health"
protocol_health "$TARGET_DIR/tura-command-read-media"
protocol_health "$TARGET_DIR/tura-command-web-discover"

step "Build release script tests completed"
