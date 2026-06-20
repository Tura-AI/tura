#!/usr/bin/env sh
set -eu

COMMAND_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
VENV_DIR="$COMMAND_DIR/.venv"
CHECK_ONLY=0
OFFLINE=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --check-only) CHECK_ONLY=1 ;;
    --offline) OFFLINE=1 ;;
    -h|--help)
      echo "Usage: commands/web_discover/install.sh [--check-only] [--offline]"
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

have() {
  command -v "$1" >/dev/null 2>&1
}

venv_python() {
  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*) printf '%s\n' "$VENV_DIR/Scripts/python.exe" ;;
    *) printf '%s\n' "$VENV_DIR/bin/python" ;;
  esac
}

verify_web_discover() {
  python=$(venv_python)
  [ -x "$python" ] || { echo "web_discover virtual environment was not found at $VENV_DIR." >&2; exit 1; }
  "$python" -c 'import ddgs, duckduckgo_search, yt_dlp; print("web_discover python deps ok")'
}

have uv || { echo "uv was not found. Run the root scripts/install.sh first or install uv from https://docs.astral.sh/uv/." >&2; exit 1; }

if [ "$CHECK_ONLY" -eq 1 ]; then
  verify_web_discover
  echo "web_discover dependencies: ok"
  exit 0
fi

cd "$COMMAND_DIR"
if [ ! -x "$(venv_python)" ]; then
  if [ "$OFFLINE" -eq 1 ]; then
    uv venv --python 3.12 .venv --offline
  else
    uv venv --python 3.12 .venv
  fi
else
  echo "Reusing web_discover virtual environment at $VENV_DIR"
fi

if [ "$OFFLINE" -eq 1 ]; then
  uv pip install --python "$(venv_python)" -r "$COMMAND_DIR/requirements.txt" --offline
else
  uv pip install --python "$(venv_python)" -r "$COMMAND_DIR/requirements.txt"
fi

verify_web_discover
echo "web_discover dependencies installed in $VENV_DIR"
