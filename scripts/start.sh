#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
MODE=debug
BUILD_ONLY=0
TUI=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --release) MODE=release ;;
    --build-only) BUILD_ONLY=1 ;;
    --tui) TUI=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/start.sh [--release] [PROMPT...]
  scripts/start.sh [--release] --tui [tura args...]
  scripts/start.sh [--release] --build-only
EOF
      exit 0
      ;;
    --) shift; break ;;
    *) break ;;
  esac
  shift
done

TARGET_DIR="$REPO_ROOT/target/$MODE"
BUILD_SCRIPT="$SCRIPT_DIR/build-$MODE.sh"

have() {
  command -v "$1" >/dev/null 2>&1
}

ensure_macos_zsh() {
  os_name=$(uname -s 2>/dev/null || echo unknown)
  [ "$os_name" = "Darwin" ] || return 0
  if [ -n "${TURA_ZSH_PATH:-}" ] && [ -x "$TURA_ZSH_PATH" ]; then
    return 0
  fi
  if [ -x /bin/zsh ] || have zsh; then
    return 0
  fi
  echo "macOS requires zsh for the default Tura shell surface. Install zsh or set TURA_ZSH_PATH to a valid zsh binary." >&2
  exit 1
}

ensure_built() {
  if [ ! -x "$TARGET_DIR/tura_exec" ] || [ ! -x "$TARGET_DIR/tura" ] || [ ! -x "$TARGET_DIR/tura_gateway" ]; then
    sh "$BUILD_SCRIPT"
  fi
}

ensure_macos_zsh
ensure_built
[ "$BUILD_ONLY" -eq 0 ] || exit 0

if [ "$TUI" -eq 1 ]; then
  exec "$TARGET_DIR/tura" "$@"
fi

exec "$TARGET_DIR/tura" exec "$@"
